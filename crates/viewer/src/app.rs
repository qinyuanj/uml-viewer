//! eframe application: window, hot reload, evidence publishing, driver
//! replay, input mapping, and the undecorated-window drag/resize regions.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use edn::Value;
use eframe::egui;
use egui::{Pos2, ViewportCommand};
use scene::document::{self, ViewState};
use scene::events::{self, Dims};
use scene::geom::{get, num};

use crate::bench;
use crate::driver::{self, Input, Runner};
use crate::evidence::Evidence;
use crate::paint;
use crate::{cli::Options, fonts, identity, metrics, strings, watch};

pub const WINDOW_TITLE: &str = "UML viewer — Rust";
pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Height of the undecorated window's drag strip, in points.
pub const DRAG_STRIP: f32 = 28.0;
/// Resize grip thickness, in points.
pub const RESIZE_GRIP: f32 = 6.0;

/// Window decoration decision (ADR 0008 / #134): decorated X11 when the
/// X11 preflight passes, otherwise an undecorated Wayland window with custom
/// drag and resize regions (winit's client decorations disconnect Weston).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    X11Decorated,
    WaylandUndecorated,
}

impl WindowMode {
    pub fn decorated(self) -> bool {
        self == WindowMode::X11Decorated
    }
}

pub fn choose(x11_available: bool) -> WindowMode {
    if x11_available {
        WindowMode::X11Decorated
    } else {
        WindowMode::WaylandUndecorated
    }
}

/// X11 needs libxkbcommon-x11 at runtime; probe it without elevating.
pub fn x11_available() -> bool {
    if std::env::var("DISPLAY").ok().is_none_or(|d| d.is_empty()) {
        return false;
    }
    lib_present("libxkbcommon-x11.so.0")
}

fn lib_present(name: &str) -> bool {
    let paths = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib",
    ];
    if paths.iter().any(|dir| Path::new(dir).join(name).exists()) {
        return true;
    }
    std::process::Command::new("ldconfig")
        .arg("-p")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(name))
        .unwrap_or(false)
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Default diagram (port of `session_file.default_diagram`): the generated
/// `diagram.edn`, the project-named example, then any example, else
/// `diagram.edn`.
pub fn default_diagram(root: &Path) -> PathBuf {
    let candidates = [
        root.join("diagram.edn"),
        root.join("examples")
            .join(
                root.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
            .with_extension("edn"),
        root.join("examples/uml-viewer.edn"),
    ];
    for candidate in &candidates {
        if candidate.is_file() {
            return candidate.clone();
        }
    }
    if let Ok(entries) = std::fs::read_dir(root.join("examples"))
        && let Some(found) = entries
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "edn"))
    {
        return found;
    }
    candidates[0].clone()
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn session_path(root: &Path) -> PathBuf {
    root.join(".uml-viewer/session.edn")
}

fn read_session(root: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(session_path(root)).ok()?;
    match edn::parse(&text) {
        Ok(v @ Value::Map(_)) => Some(v),
        _ => None,
    }
}

/// Reapply a saved view (port of `application.session.restore`).
fn restore_session(state: &ViewState, snapshot: &Value) -> ViewState {
    let mut extra = Value::Map(vec![]);
    for key in ["proposal-id", "proposal", "open-layer", "declutter"] {
        if let Some(value) = get(snapshot, key) {
            extra.assoc(Value::keyword(key), value.clone());
        }
    }
    let mut shaped = ViewState {
        focus: match get(snapshot, "focus") {
            Some(Value::Vector(v)) => v.clone(),
            _ => Vec::new(),
        },
        extra,
        cam_x: get(snapshot, "cam-x").and_then(num).unwrap_or(0.0),
        cam_y: get(snapshot, "cam-y").and_then(num).unwrap_or(0.0),
        zoom: get(snapshot, "zoom").and_then(num).unwrap_or(1.0),
        selected: get(snapshot, "selected").cloned(),
        detail_id: get(snapshot, "detail-id").cloned(),
        discussion_context: get(snapshot, "discussion-context").cloned(),
        ..ViewState::default()
    };
    shaped.mail_seen = get(snapshot, "mail-seen").and_then(num).unwrap_or(0.0) as i64;
    document::restore_session(state, &shaped)
}

struct Setup {
    project_root: PathBuf,
    diagram: PathBuf,
    evidence: Option<Evidence>,
    test_seconds: Option<f64>,
    script: Option<driver::Script>,
    restart: bool,
    backend: bool,
    /// Protocol-1 output directory (#143); the bench owns the camera.
    bench: Option<PathBuf>,
    bench_only: Option<String>,
}

impl Setup {
    fn from_options(options: Options) -> Result<Setup, String> {
        let root = env_nonempty("UML_VIEWER_PROJECT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let project_root = canonical(&root);
        // The saved path is consulted only on --restart (Lua `session.path`).
        let snapshot = if options.restart {
            read_session(&project_root)
        } else {
            None
        };
        let saved = snapshot
            .as_ref()
            .and_then(|s| get(s, "path"))
            .and_then(|p| match p {
                Value::String(s) => Some(PathBuf::from(s)),
                _ => None,
            });
        let diagram = match options.path.clone().or(saved) {
            Some(path) => canonical(&path),
            None => canonical(&default_diagram(&project_root)),
        };
        let evidence = env_nonempty("UML_VIEWER_EVIDENCE").map(|dir| {
            let dir = PathBuf::from(dir);
            let _ = std::fs::create_dir_all(&dir);
            Evidence::new(dir)
        });
        let test_seconds = env_nonempty("UML_VIEWER_TEST_SECONDS").and_then(|v| v.parse().ok());
        let script = match env_nonempty("UML_VIEWER_TEST_DRIVER") {
            Some(path) => Some(driver::Script::load(Path::new(&path))?),
            None => None,
        };
        Ok(Setup {
            project_root,
            diagram,
            evidence,
            test_seconds,
            script,
            restart: options.restart,
            backend: options.backend,
            bench: env_nonempty("UML_VIEWER_BENCH").map(PathBuf::from),
            bench_only: env_nonempty("UML_VIEWER_BENCH_ONLY"),
        })
    }
}

/// eframe entry point.
pub fn run(options: Options) -> ExitCode {
    // WSLg: d3d12 through Mesa and the GL wgpu backend; never override a user
    // value. Also set by the launcher; this keeps direct launches on the GPU.
    for (key, value) in [("GALLIUM_DRIVER", "d3d12"), ("WGPU_BACKEND", "gl")] {
        if std::env::var_os(key).is_none() {
            // SAFETY: called before any thread is spawned (eframe starts the
            // event loop after this point).
            unsafe { std::env::set_var(key, value) };
        }
    }
    let setup = match Setup::from_options(options) {
        Ok(setup) => setup,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let mode = choose(x11_available());
    let exit = Arc::new(AtomicU8::new(0));
    // Protocol 1 fixes the window at 1500 × 920 view px.
    let inner = if setup.bench.is_some() {
        [1500.0, 920.0]
    } else {
        [1200.0, 800.0]
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size(inner)
        .with_min_inner_size([420.0, 300.0])
        .with_decorations(mode.decorated());
    if !mode.decorated() {
        viewport = viewport.with_title(format!("{WINDOW_TITLE} (无装饰)"));
    }
    let mut native = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if mode == WindowMode::X11Decorated {
        native.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11();
        }));
    }
    let window_mode = mode;
    let exit_handle = Arc::clone(&exit);
    let result = eframe::run_native(
        WINDOW_TITLE,
        native,
        Box::new(move |cc| {
            let app = Viewer::new(cc, setup, window_mode, exit_handle)?;
            Ok(Box::new(app))
        }),
    );
    if let Err(error) = result {
        eprintln!("UML viewer: window failed: {error}");
        return ExitCode::from(1);
    }
    ExitCode::from(exit.load(Ordering::SeqCst))
}

struct Viewer {
    state: ViewState,
    project_root: PathBuf,
    diagram: PathBuf,
    evidence: Option<Evidence>,
    test_seconds: Option<f64>,
    driver: Option<Runner>,
    frames: u64,
    started: Instant,
    last_poll: Instant,
    last_frame: Instant,
    frame_ms: f64,
    pending: Vec<Input>,
    hint: Option<String>,
    status: Option<String>,
    show_performance: bool,
    force_refresh: bool,
    exit: Arc<AtomicU8>,
    published: u64,
    render_shot: bool,
    shots_requested: Vec<String>,
    decorated: bool,
    driver_failed: bool,
    quit: bool,
    ctx: egui::Context,
    window_w: f64,
    window_h: f64,
    shots_sent: Vec<String>,
    bench: Option<bench::Bench>,
    bench_geo: Option<bench::Geo>,
    /// Adapter name, backend, driver, driver info (protocol-1 environment).
    adapter: [String; 4],
}

impl Viewer {
    fn new(
        cc: &eframe::CreationContext<'_>,
        setup: Setup,
        mode: WindowMode,
        exit: Arc<AtomicU8>,
    ) -> Result<Viewer, String> {
        fonts::install(&cc.egui_ctx, &setup.project_root);
        let adapter = cc
            .wgpu_render_state
            .as_ref()
            .map(|state| {
                let info = state.adapter.get_info();
                [
                    info.name,
                    format!("{:?}", info.backend),
                    info.driver,
                    info.driver_info,
                ]
            })
            .unwrap_or_default();
        let renderer = cc.wgpu_render_state.as_ref().map(|state| {
            let info = state.adapter.get_info();
            format!(
                "renderer: {}\nbackend: {:?}\napi: {:?}\ndriver: {}\n",
                info.name, info.backend, info.device_type, info.driver
            )
        });
        let mut app = Viewer {
            state: ViewState::default(),
            project_root: setup.project_root.clone(),
            diagram: setup.diagram.clone(),
            evidence: setup.evidence.clone(),
            test_seconds: setup.test_seconds,
            driver: setup.script.clone().map(Runner::new),
            frames: 0,
            started: Instant::now(),
            last_poll: Instant::now() - POLL_INTERVAL,
            last_frame: Instant::now(),
            frame_ms: 0.0,
            pending: Vec::new(),
            hint: None,
            status: None,
            show_performance: false,
            force_refresh: false,
            exit,
            published: 0,
            render_shot: false,
            shots_requested: Vec::new(),
            decorated: mode.decorated(),
            driver_failed: false,
            quit: false,
            ctx: cc.egui_ctx.clone(),
            window_w: 1200.0,
            window_h: 800.0,
            shots_sent: Vec::new(),
            bench: None,
            bench_geo: None,
            adapter,
        };
        if let Some(evidence) = &setup.evidence {
            if let Some(info) = &renderer {
                let _ = evidence.write_text("renderer.txt", info);
            }
            // The backend identity is the D3D12-versus-llvmpipe evidence.
            eprintln!(
                "UML viewer: {}",
                info_lines(renderer.as_deref().unwrap_or("renderer: unknown\n"))
            );
        }
        app.load_initial(setup.restart, setup.backend)?;
        if let Some(out) = setup.bench.clone() {
            let (classes, _, _) = bench::scene_counts(&app.state.scene);
            let runner = bench::Bench::new(
                out,
                setup.bench_only.clone(),
                app.adapter[0].clone(),
                classes,
            )?;
            app.bench = Some(runner);
            // The bench sets the unclamped protocol camera; no viewer fit.
            app.state.fit_pending = false;
        }
        // A refused or invalid startup never overwrites an existing identity.
        if !setup.backend
            && let Err(error) = identity::record(&setup.project_root)
        {
            eprintln!("uml viewer: could not record the process identity: {error}");
        }
        Ok(app)
    }

    fn load_initial(&mut self, restart: bool, backend: bool) -> Result<(), String> {
        let (snapshots, stamp) = metrics::snapshot(&self.project_root);
        let path = self.diagram.to_string_lossy().into_owned();
        if backend && !restart {
            let mut state = document::waiting_state(Some(path), 0);
            state.project_root = Some(self.project_root.to_string_lossy().into_owned());
            state.revision = 1;
            self.state = state;
            self.publish();
            return Ok(());
        }
        let text = match std::fs::read_to_string(&self.diagram) {
            Ok(text) => text,
            Err(_) => {
                let state = ViewState {
                    path: Some(path.clone()),
                    project_root: Some(self.project_root.to_string_lossy().into_owned()),
                    revision: 1,
                    ..ViewState::default()
                };
                self.state = state;
                self.hint = Some(strings::missing_file(&path));
                self.publish();
                return Ok(());
            }
        };
        let doc = scene::ir::read_document(&text).map_err(|e| format!("{path}: {e}"))?;
        let mtime = watch::mtime(&self.diagram).ok();
        let mut state = document::new_state(
            doc,
            mtime.map(Value::String),
            None,
            None,
            &snapshots,
            Some(stamp),
        );
        state.path = Some(path);
        state.project_root = Some(self.project_root.to_string_lossy().into_owned());
        if restart {
            if let Some(snapshot) = read_session(&self.project_root) {
                state = restore_session(&state, &snapshot);
            }
        } else {
            state.fit_pending = true;
        }
        self.state = state;
        self.publish();
        Ok(())
    }

    fn publish(&mut self) {
        if let Some(evidence) = &self.evidence
            && self.state.revision != self.published
        {
            if let Err(error) = evidence.publish(&self.state.scene, self.state.revision) {
                eprintln!("uml viewer: could not publish evidence: {error}");
            }
            self.published = self.state.revision;
        }
    }

    fn dims(&self, ctx: &egui::Context) -> Dims {
        let (control, shift) = ctx.input(|i| (i.modifiers.ctrl, i.modifiers.shift));
        let screen = ctx.content_rect();
        Dims {
            window_w: screen.width() as f64,
            window_h: screen.height() as f64,
            view_w: Some(screen.width() as f64),
            control,
            shift,
            ..Dims::default()
        }
    }

    fn apply_input(&mut self, input: Input, ctx: &egui::Context) {
        let dims = self.dims(ctx);
        let transform = paint::Transform::new(ctx.content_rect(), &self.state);
        let screen = |x: f64, y: f64| transform.world(Pos2::new(x as f32, y as f32));
        let mut state = self.state.clone();
        match input {
            Input::Click { x, y } => {
                let (wx, wy) = screen(x, y);
                state = events::on_left_press(&state, wx, wy);
            }
            Input::DoubleClick { x, y } => {
                let (wx, wy) = screen(x, y);
                state = events::on_double_press(&state, wx, wy);
            }
            Input::Drag { from, to } => {
                let (ax, ay) = screen(from[0], from[1]);
                let (bx, by) = screen(to[0], to[1]);
                state = events::on_left_press(&state, ax, ay);
                state = events::on_drag(&state, bx, by);
                state = events::on_release(&state, bx, by);
            }
            Input::Wheel { dx, dy } => state = events::on_wheel(&state, dx, dy, &dims),
            Input::Key { key, ctrl, shift } => {
                let mut dims = dims.clone();
                dims.control = ctrl || dims.control;
                dims.shift = shift || dims.shift;
                dims.raw_key = Some(key.clone());
                let value = Value::String(key.clone());
                state = events::on_key(&state, Some(&value), &dims);
                if key == "r" {
                    self.force_refresh = true;
                }
            }
            Input::Text(_) => {}
        }
        self.state = state;
    }

    /// Poll the IR and metrics stamps; reload when either changed.
    fn poll(&mut self) {
        // A companion launch shows the empty canvas until :display (#142).
        if self.state.waiting {
            return;
        }
        let path_text = self.diagram.to_string_lossy().into_owned();
        let Ok(mtime) = watch::mtime(&self.diagram) else {
            self.hint = Some(strings::missing_file(&path_text));
            return;
        };
        let (snapshots, stamp) = metrics::snapshot(&self.project_root);
        let current_mtime = Some(Value::String(mtime));
        if current_mtime == self.state.mtime && Some(stamp.clone()) == self.state.metrics_stamp {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&self.diagram) else {
            self.hint = Some(strings::missing_file(&path_text));
            return;
        };
        let (next, changed, rejected) =
            document::reload(&self.state, &text, current_mtime, &snapshots, Some(stamp));
        if let Some(reason) = rejected {
            eprintln!("Document reload rejected: {reason}");
        }
        if next.document.is_some() {
            self.hint = None;
        }
        self.state = next;
        if changed {
            self.publish();
        }
    }

    fn request_screenshot(&self, ctx: &egui::Context, name: &str) {
        let Some(evidence) = &self.evidence else {
            return;
        };
        let target = evidence.path(name);
        let ctx = ctx.clone();
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
            move |image| {
                if let Err(error) = write_png(&target, &image) {
                    eprintln!("uml viewer: screenshot failed: {error}");
                }
            },
        )));
    }

    /// Undecorated fallback: the top strip drags, the edges resize. Returns
    /// true when the press was consumed by a window region.
    fn window_chrome_input(&mut self, ctx: &egui::Context) -> bool {
        if self.decorated {
            return false;
        }
        let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
            return false;
        };
        let pressed = ctx.input(|i| i.pointer.primary_pressed());
        let screen = ctx.content_rect();
        let direction = if pointer.y <= screen.min.y + RESIZE_GRIP {
            if pointer.x <= screen.min.x + RESIZE_GRIP * 3.0 {
                Some(egui::viewport::ResizeDirection::NorthWest)
            } else if pointer.x >= screen.max.x - RESIZE_GRIP * 3.0 {
                Some(egui::viewport::ResizeDirection::NorthEast)
            } else {
                Some(egui::viewport::ResizeDirection::North)
            }
        } else if pointer.y >= screen.max.y - RESIZE_GRIP {
            if pointer.x <= screen.min.x + RESIZE_GRIP * 3.0 {
                Some(egui::viewport::ResizeDirection::SouthWest)
            } else if pointer.x >= screen.max.x - RESIZE_GRIP * 3.0 {
                Some(egui::viewport::ResizeDirection::SouthEast)
            } else {
                Some(egui::viewport::ResizeDirection::South)
            }
        } else if pointer.x <= screen.min.x + RESIZE_GRIP {
            Some(egui::viewport::ResizeDirection::West)
        } else if pointer.x >= screen.max.x - RESIZE_GRIP {
            Some(egui::viewport::ResizeDirection::East)
        } else if pointer.y <= screen.min.y + DRAG_STRIP {
            None
        } else {
            return false;
        };
        if pressed {
            match direction {
                Some(direction) => ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction)),
                None => ctx.send_viewport_cmd(ViewportCommand::StartDrag),
            }
        }
        true
    }

    fn collect_real_input(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        self.window_w = screen.width() as f64;
        self.window_h = screen.height() as f64;
        if self.driver.is_some() || self.bench.is_some() {
            // A driver or the bench owns the event stream; real input would race it.
            return;
        }
        let transform = paint::Transform::new(screen, &self.state);
        let (pos, pressed, released, double, down) = ctx.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary),
                i.pointer.primary_down(),
            )
        });
        let inside = pos.is_some_and(|p| screen.contains(p));
        if let Some(p) = pos.filter(|_| inside) {
            let (wx, wy) = transform.world(p);
            if double {
                self.state = events::on_double_press(&self.state, wx, wy);
            } else if pressed {
                self.state = events::on_left_press(&self.state, wx, wy);
            } else if down && self.state.drag.is_some() {
                self.state = events::on_drag(&self.state, wx, wy);
            } else if !down {
                self.state = events::on_move(&self.state, wx, wy);
            }
            if released {
                self.state = events::on_release(&self.state, wx, wy);
            }
        } else if !down {
            self.state = events::on_leave(&self.state);
        }
        let wheel = ctx.input(|i| i.smooth_scroll_delta);
        if wheel != egui::Vec2::ZERO {
            self.pending.push(Input::Wheel {
                dx: wheel.x as f64,
                // egui reports points; LOVE wheelmoved reports notches.
                dy: (wheel.y / 50.0) as f64,
            });
        }
        let keys: Vec<egui::Key> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(*key),
                    _ => None,
                })
                .collect()
        });
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.ctrl, i.modifiers.shift));
        for key in keys {
            if key == egui::Key::F3 {
                self.show_performance = !self.show_performance;
                continue;
            }
            let name = key_name(key);
            if name.is_empty() {
                continue;
            }
            self.pending.push(Input::Key {
                key: name.into(),
                ctrl,
                shift,
            });
        }
    }
}

impl Viewer {
    /// One protocol-1 frame: apply the trajectory camera (unclamped, world
    /// centre → top-left cam), keep repainting in interactive phases, and
    /// close with the verdict as the exit code once every phase is done.
    fn bench_frame(
        &mut self,
        ctx: &egui::Context,
        rect: egui::Rect,
        now: Instant,
        cpu_ms: Option<f64>,
    ) {
        let (w, h) = (rect.width() as f64, rect.height() as f64);
        if self.bench_geo.is_none() {
            let geo = bench::Geo::from_scene(&self.state.scene, w, h);
            let [name, backend, driver, info] = &self.adapter;
            let meta = bench::environment_meta(
                name,
                backend,
                driver,
                info,
                ctx.pixels_per_point() as f64,
                (w, h),
                &geo,
                bench::scene_counts(&self.state.scene),
            );
            eprintln!("UML viewer bench: {meta}");
            if let Some(runner) = &mut self.bench {
                runner.set_meta(meta);
            }
            self.bench_geo = Some(geo);
        }
        let geo = self.bench_geo.expect("set above");
        let Some(runner) = &mut self.bench else {
            return;
        };
        let step = runner.frame(&geo, now, cpu_ms);
        let passed = runner.passed();
        if let Some((cx, cy, zoom)) = step.camera {
            self.state.zoom = zoom;
            self.state.cam_x = cx - w / (2.0 * zoom);
            self.state.cam_y = cy - h / (2.0 * zoom);
            self.state.fit_pending = false;
        }
        if step.continuous {
            ctx.request_repaint();
        }
        if step.finished {
            let code = if passed == Some(true) { 0 } else { 1 };
            self.exit.store(code, Ordering::SeqCst);
            self.quit = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

fn key_name(key: egui::Key) -> &'static str {
    match key {
        egui::Key::R => "r",
        egui::Key::P => "p",
        egui::Key::Escape => "escape",
        egui::Key::ArrowLeft => "left",
        egui::Key::ArrowRight => "right",
        egui::Key::ArrowUp => "up",
        egui::Key::ArrowDown => "down",
        egui::Key::Plus | egui::Key::Equals => "+",
        egui::Key::Minus => "-",
        egui::Key::Num0 => "0",
        _ => "",
    }
}

fn info_lines(info: &str) -> String {
    info.trim().replace('\n', "; ")
}

fn write_png(target: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let [w, h] = image.size;
    let mut pixels = Vec::with_capacity(w * h * 4);
    for pixel in &image.pixels {
        pixels.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
    }
    let buffer = image::RgbaImage::from_raw(w as u32, h as u32, pixels)
        .ok_or_else(|| "screenshot size mismatch".to_string())?;
    buffer.save(target).map_err(|e| e.to_string())
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.frames += 1;
        let now = Instant::now();
        self.frame_ms = now.duration_since(self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;

        // Protocol-1 bench (#143): it owns the camera and the frame cadence.
        if self.bench.is_some() {
            let cpu_ms = frame.info().cpu_usage.map(|s| s as f64 * 1000.0);
            self.bench_frame(ctx, ui.max_rect(), now, cpu_ms);
        }

        // Driver replay: take the runner out so it can borrow the app as host.
        if self.bench.is_none() && self.driver.is_some() && !self.driver_failed {
            let mut runner = self.driver.take().expect("checked");
            let evidence = self.evidence.as_ref().map(|e| e.dir.clone());
            let project = self.project_root.clone();
            let outcome = runner.run(self.frames, evidence.as_deref(), &project, self);
            if let Err(message) = &outcome {
                eprintln!("Navigation acceptance failed: {message}");
                self.exit.store(1, Ordering::SeqCst);
                self.driver_failed = true;
                self.quit = true;
            }
            self.driver = Some(runner);
            if self.quit {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        // Real input (unless a driver owns the stream).
        self.collect_real_input(ctx);
        let chrome_consumed = self.window_chrome_input(ctx);
        if !chrome_consumed {
            let pending = std::mem::take(&mut self.pending);
            for input in pending {
                self.apply_input(input, ctx);
            }
        } else {
            self.pending.clear();
        }

        // Hot reload: IR or metrics stamp change, or R.
        if self.force_refresh || self.last_poll.elapsed() >= POLL_INTERVAL {
            self.force_refresh = false;
            self.last_poll = Instant::now();
            self.poll();
        }
        if self.state.fit_pending {
            let dims = self.dims(ctx);
            self.state = events::settle(&self.state, &dims);
        }

        let rect = ui.max_rect();
        let hint = self.hint.clone();
        let status = self.status.clone();
        let performance = self
            .show_performance
            .then(|| strings::performance(fps(ctx), self.frame_ms));
        paint::paint(
            ctx,
            ui.painter(),
            &self.state,
            rect,
            hint.as_deref(),
            status.as_deref(),
            performance,
            self.show_performance,
        );

        // Evidence: a PNG at frame 4 plus driver-requested screenshots.
        if self.frames == 4 && self.evidence.is_some() && !self.render_shot {
            self.render_shot = true;
            self.request_screenshot(ctx, "render.png");
        }
        for name in self.shots_requested.clone() {
            let missing = self
                .evidence
                .as_ref()
                .is_some_and(|e| !e.path(&name).exists());
            if missing && !self.shots_sent.iter().any(|n| n == &name) {
                self.shots_sent.push(name.clone());
                self.request_screenshot(ctx, &name);
            }
        }

        if let Some(seconds) = self.test_seconds
            && self.started.elapsed().as_secs_f64() >= seconds
        {
            self.quit = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        ctx.request_repaint_after(POLL_INTERVAL);
    }
}

fn fps(ctx: &egui::Context) -> u32 {
    let dt = ctx.input(|i| i.stable_dt).max(1e-6);
    (1.0 / dt).round() as u32
}

impl driver::Host for Viewer {
    fn inject(&mut self, input: Input) {
        self.pending.push(input);
    }

    fn assert_italic_glyphs(&mut self) -> Result<(), String> {
        // The probe runs through this window's live font atlas.
        paint::italic_glyph_probe(&self.ctx)
    }

    fn screenshot(&mut self, evidence: &str) -> bool {
        if !self.shots_requested.iter().any(|n| n == evidence) {
            self.shots_requested.push(evidence.to_string());
        }
        self.evidence
            .as_ref()
            .is_some_and(|e| e.path(evidence).exists())
    }

    fn set_view(&mut self, zoom: Option<f64>, center: Option<[f64; 2]>) {
        if let Some(zoom) = zoom {
            // Keep the view centre fixed, like ctrl+wheel at the middle.
            let factor = zoom / self.state.zoom.max(1e-6);
            let (cx, cy) = (self.window_w / 2.0, self.window_h / 2.0);
            self.state = events::zoom_at(&self.state, factor, cx, cy);
            self.state.fit_pending = false;
        }
        if let Some(center) = center {
            // Center the world point: cam is the top-left world coordinate.
            self.state.cam_x = center[0] - self.window_w / (2.0 * self.state.zoom);
            self.state.cam_y = center[1] - self.window_h / (2.0 * self.state.zoom);
        }
    }

    fn quit(&mut self, code: u8) {
        self.exit.store(code, Ordering::SeqCst);
        self.quit = true;
    }
}
