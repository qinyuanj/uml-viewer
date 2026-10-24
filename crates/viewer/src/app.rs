//! eframe application: window, hot reload, evidence publishing, driver
//! replay, input mapping, and the undecorated-window drag/resize regions.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use edn::Value;
use edn::record::{get, local_name};
use eframe::egui;
use egui::{FontFamily, FontId, ViewportCommand};
use scene::document::{self, ViewState};
use scene::events::{self, Dims};
use scene::geom::Rect;
use scene::hit;

use crate::bench;
use crate::commands::{self, Command};
use crate::driver::{self, Input, Runner, ViewRecord};
use crate::evidence::Evidence;
use crate::gui::{Action, Gui};
use crate::render_cache::RenderCache;
use crate::search_box::SearchBox;
use crate::{
    cli::Options, fonts, identity, inspector, mail, mail_watch, mailbox, metrics, paint,
    project_files, session, signal, strings,
};
use crate::{clock, source, theme, watch};

pub const WINDOW_TITLE: &str = "UML viewer — Rust";
/// Default window in points (the LOVE viewer's 1500x920 view units); egui
/// multiplies points by pixels-per-point, so a scaled display keeps it.
pub const WINDOW_SIZE: [f32; 2] = [1500.0, 920.0];
pub const WINDOW_MIN: [f32; 2] = [640.0, 480.0];

/// `UML_VIEWER_TEST_DPI_SCALE` simulates a display scale in isolated runs
/// (port of `adapters/dpi.lua` `override`). Real displays need nothing:
/// winit reports each monitor's scale factor and egui re-rasterises.
pub fn dpi_override() -> Option<f32> {
    env_nonempty("UML_VIEWER_TEST_DPI_SCALE")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
}
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

/// `force_undecorated` is the `UML_VIEWER_TEST_UNDECORATED` seam: drivers
/// exercise the libxkbcommon-x11 fallback on a machine that has X11.
pub fn mode_for(x11_available: bool, force_undecorated: bool) -> WindowMode {
    if x11_available && !force_undecorated {
        WindowMode::X11Decorated
    } else {
        WindowMode::WaylandUndecorated
    }
}

/// Side of a window control button, in points.
const CONTROL_W: f32 = 28.0;
const CONTROL_H: f32 = DRAG_STRIP - RESIZE_GRIP - 2.0;

/// The undecorated window's own `[minimise, close]` buttons: at the right
/// of the drag strip, below the north grip and left of the east grip.
pub fn window_controls(screen: egui::Rect) -> [egui::Rect; 2] {
    let top = screen.min.y + RESIZE_GRIP + 1.0;
    let right = screen.max.x - RESIZE_GRIP - 2.0;
    let close = egui::Rect::from_min_max(
        egui::pos2(right - CONTROL_W, top),
        egui::pos2(right, top + CONTROL_H),
    );
    let minimise = close.translate(egui::vec2(-(CONTROL_W + 2.0), 0.0));
    [minimise, close]
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

fn read_session(root: &Path) -> Option<Value> {
    session::read(root)
}

/// Reapply a saved view (port of `application.session.restore`).
fn restore_session(state: &ViewState, snapshot: &Value) -> ViewState {
    session::restore(state, snapshot)
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
    /// Saved window/inspector chrome from a restart session.
    chrome: session::Chrome,
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
        // A restart restores the saved window and inspector width; a fresh
        // launch uses the defaults.
        let chrome = if options.restart {
            session::chrome(snapshot.as_ref())
        } else {
            session::Chrome::default()
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
            chrome,
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
    signal::install();
    let setup = match Setup::from_options(options) {
        Ok(setup) => setup,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let mode = mode_for(
        x11_available(),
        env_nonempty("UML_VIEWER_TEST_UNDECORATED").is_some(),
    );
    let exit = Arc::new(AtomicU8::new(0));
    // A simulated scale also scales the window, so the canvas stays
    // 1500x920 points (Lua `display.window_size`: view units × ui).
    // 1500x920 is also the protocol-1 bench window (#143).
    let scale = dpi_override().unwrap_or(1.0);
    // A restart restores the saved window; a fresh launch keeps the default.
    let [width, height] = session::window_size(&setup.chrome, f64::from(scale));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size([width as f32, height as f32])
        .with_min_inner_size([WINDOW_MIN[0] * scale, WINDOW_MIN[1] * scale])
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
    /// Resolved CJK font and its source (`env`/`sarasa`/`system`).
    font: Option<(PathBuf, &'static str)>,
    search: SearchBox,
    /// Text and paste events for the search field, delivered to egui right
    /// before the field is drawn so they take egui's own input path.
    text_events: Vec<egui::Event>,
    /// Last pointer position of replayed input (view units); a wheel over the
    /// open search box belongs to it even though the canvas saw a leave.
    driver_pointer: Option<(f64, f64)>,
    bench: Option<bench::Bench>,
    bench_geo: Option<bench::Geo>,
    /// Adapter name, backend, driver, driver info (protocol-1 environment).
    adapter: [String; 4],
    /// Class/edge geometry, rebuilt only when the scene, selection, hover or
    /// LOD tier changes (#143).
    render_cache: RenderCache,
    /// Canvas text layouts across frames (#143).
    text_cache: paint::TextCache,
    /// Stops the idle wake thread when the viewer is dropped.
    wake_stop: Arc<AtomicBool>,
    /// Inspector sidebar, class card and source windows (#141).
    gui: Gui,
    /// Driver GUI events, one batch per frame through `raw_input_hook`.
    gui_batches: std::collections::VecDeque<Vec<egui::Event>>,
    /// Where replayed GUI pointer events are (screen points).
    gui_pointer: egui::Pos2,
    /// A batch reached egui this frame and the GUI has not drawn it yet.
    gui_inflight: bool,
    /// The companion helper and the `to-viewer.edn` queue (`--backend herdr`).
    companion: Option<mail::Companion>,
    /// Change detector for the mail queue and the helper's event files.
    mail_watch: mail_watch::Watch,
    /// The identity was recorded once the helper reached ready.
    companion_recorded: bool,
    /// `:quit-for-restart` already asked the helper to stop with `keep`.
    restart_stop: bool,
    /// A normal quit (window close, SIGTERM, test duration) asked the helper
    /// to stop without keeping it.
    normal_stop: bool,
    /// The deadline for a stopping helper; the viewer exits at it regardless.
    stop_deadline: Option<Instant>,
    /// The change-only mailbox waker thread.
    waker: Option<mail_watch::Waker>,
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.wake_stop.store(true, Ordering::SeqCst);
    }
}

/// Idle wake-ups (#143): a stat-only watcher repaints only when the diagram,
/// metrics or mailbox stamp changes, so an idle viewer does not redraw on the
/// poll cadence. The woken frame's `poll` does the actual reload. A pending
/// SIGTERM/SIGINT also wakes it: the signal handler only sets a flag, and an
/// idle viewer would otherwise not run a frame to act on it.
fn spawn_wake(ctx: egui::Context, diagram: PathBuf, root: PathBuf) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let spawned = std::thread::Builder::new()
        .name("uml-viewer-wake".into())
        .spawn(move || {
            let mut wake = watch::Wake::new(&diagram, &root);
            while !flag.load(Ordering::SeqCst) {
                std::thread::sleep(POLL_INTERVAL);
                if wake.poll() || signal::terminated() {
                    ctx.request_repaint();
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("uml viewer: could not start the idle watcher: {error}");
    }
    stop
}

impl Viewer {
    fn new(
        cc: &eframe::CreationContext<'_>,
        setup: Setup,
        mode: WindowMode,
        exit: Arc<AtomicU8>,
    ) -> Result<Viewer, String> {
        let font = fonts::install(&cc.egui_ctx, &setup.project_root);
        if let Some(scale) = dpi_override() {
            cc.egui_ctx.set_zoom_factor(scale);
        }
        // Ctrl± / Ctrl0 zoom the canvas, not egui's UI scale.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
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
        // The first frames measure the window before the content rect is
        // known: start from the requested (or restored) size.
        let [start_w, start_h] =
            session::window_size(&setup.chrome, dpi_override().map(f64::from).unwrap_or(1.0));
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
            window_w: start_w,
            window_h: start_h,
            shots_sent: Vec::new(),
            font,
            search: SearchBox::default(),
            text_events: Vec::new(),
            driver_pointer: None,
            bench: None,
            bench_geo: None,
            adapter,
            render_cache: RenderCache::default(),
            text_cache: paint::TextCache::default(),
            wake_stop: spawn_wake(
                cc.egui_ctx.clone(),
                setup.diagram.clone(),
                setup.project_root.clone(),
            ),
            gui: Gui::default(),
            gui_batches: Default::default(),
            gui_pointer: egui::Pos2::ZERO,
            gui_inflight: false,
            companion: None,
            mail_watch: mail_watch::Watch::new(),
            companion_recorded: false,
            restart_stop: false,
            normal_stop: false,
            stop_deadline: None,
            waker: None,
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
        // The queue is consumed whether or not a companion backend was asked
        // for (LOVE's `mail` exists in both cases); only the helper process is
        // backend-specific and starts after the first scene, as in LOVE.
        app.companion = Some(if setup.backend {
            mail::Companion::start(&setup.project_root, setup.restart)
        } else {
            mail::Companion::mail_only(&setup.project_root)
        });
        app.mail_watch.baseline(&setup.project_root);
        // Mail must be noticed even when nothing else asks for a frame; the
        // waker requests one only when a watched file actually changed.
        let ctx = cc.egui_ctx.clone();
        app.waker = Some(mail_watch::spawn(
            setup.project_root.clone(),
            POLL_INTERVAL,
            move || ctx.request_repaint(),
        ));
        if let Some(width) = setup.chrome.inspector_w {
            app.gui.width = Some(width);
        }
        if let Some(out) = setup.bench.clone() {
            let (classes, _, _) = bench::scene_counts(&app.state.scene);
            let provenance = bench::Provenance::from_env(bench::load_one_minute())?;
            let runner = bench::Bench::new(
                out,
                setup.bench_only.clone(),
                app.adapter[0].clone(),
                classes,
                provenance,
                bench::load_one_minute,
            )?;
            app.bench = Some(runner);
            // The bench sets the unclamped protocol camera; no viewer fit.
            app.state.fit_pending = false;
        }
        // A refused or invalid startup never overwrites an existing identity.
        // With a companion the identity is recorded once it is ready.
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
            view_w: Some(self.gui.canvas_width(screen.width() as f64)),
            control,
            shift,
            ..Dims::default()
        }
    }

    /// Apply one input. Coordinates are view units (points from the canvas
    /// origin), the space `scene::events` converts to world itself.
    fn apply_input(&mut self, input: Input, ctx: &egui::Context) {
        let mut dims = self.dims(ctx);
        let state = self.state.clone();
        match &input {
            Input::Click { x, y } | Input::DoubleClick { x, y } | Input::MouseMove { x, y } => {
                self.driver_pointer = Some((*x, *y))
            }
            Input::Drag { to, .. } => self.driver_pointer = Some((to[0], to[1])),
            _ => {}
        }
        self.state = match input {
            Input::Click { x, y } => {
                if self.chrome_press(x, y) {
                    return;
                }
                let pressed = events::on_left_press(&state, x, y);
                events::on_release(&pressed, x, y)
            }
            Input::DoubleClick { x, y } => {
                if self.chrome_press(x, y) {
                    return;
                }
                events::on_double_press(&state, x, y)
            }
            Input::MouseMove { x, y } => {
                if self.search.captures(x, y) {
                    events::on_leave(&state)
                } else if state.drag.is_some() {
                    events::on_drag(&state, x, y)
                } else {
                    events::on_move(&state, x, y)
                }
            }
            Input::Drag { from, to } => {
                if self.chrome_press(from[0], from[1]) {
                    return;
                }
                let pressed = events::on_left_press(&state, from[0], from[1]);
                let dragged = events::on_drag(&pressed, to[0], to[1]);
                events::on_release(&dragged, to[0], to[1])
            }
            Input::Wheel {
                dx,
                dy,
                ctrl,
                shift,
            } => {
                if self.gui.card_hovered() {
                    // LOVE wheelmoved(0, -4) scrolls the card down four notches.
                    let view_h = self.gui.card.view_h;
                    self.state = inspector::scroll(&state, -dy, view_h);
                    return;
                }
                if self
                    .driver_pointer
                    .or(state.pointer)
                    .is_some_and(|(x, y)| self.search.captures(x, y))
                {
                    return;
                }
                dims.control |= ctrl;
                dims.shift |= shift;
                events::on_wheel(&state, dx, dy, &dims)
            }
            Input::Key { key, ctrl, shift } => {
                dims.control |= ctrl;
                dims.shift |= shift;
                if key == "f" && dims.control {
                    self.search.open();
                    return;
                }
                if self.search.active {
                    // The open search box owns the keyboard (Lua: canvas
                    // keys only when not inspector.search_active()).
                    self.search_key(&key, &dims);
                    return;
                }
                dims.raw_key = Some(key.clone());
                if key == "r" {
                    self.force_refresh = true;
                }
                events::on_key(&state, Some(&Value::String(key)), &dims)
            }
            Input::Text(text) => {
                if self.search.active {
                    self.text_events.push(egui::Event::Text(text));
                } else {
                    // The focused egui field (proposal rename) takes it.
                    self.gui_batches.push_back(vec![egui::Event::Text(text)]);
                }
                return;
            }
            Input::Paste(text) => {
                if self.search.active {
                    self.text_events.push(egui::Event::Paste(text));
                } else {
                    self.gui_batches.push_back(vec![egui::Event::Paste(text)]);
                }
                return;
            }
        };
    }

    /// A press on screen chrome or the open search box; true when consumed.
    /// A press elsewhere closes the search box (it loses focus) and goes on
    /// to the canvas.
    fn chrome_press(&mut self, x: f64, y: f64) -> bool {
        if self.search.active {
            if let Some(id) = self.search.row_at(x, y) {
                self.search.selection = Some(id.clone());
                self.locate(&id);
                return true;
            }
            if self.search.captures(x, y) {
                return true;
            }
            self.search.close();
        }
        // The sidebar, the class card and source windows are egui's.
        self.gui.captures(x, y, self.window_w)
    }

    fn search_key(&mut self, key: &str, dims: &Dims) {
        let results = self.search.results(self.state.document.as_ref());
        match key {
            "down" => self.search.step(true, &results),
            "up" => self.search.step(false, &results),
            "enter" => {
                if let Some(id) = self.search.selection.clone() {
                    self.state = events::locate(&self.state, &id, dims);
                }
            }
            "escape" => self.search.close(),
            _ => {}
        }
    }

    /// Enter the target's namespace, select it, open its card and centre it.
    fn locate(&mut self, id: &Value) {
        let dims = self.dims(&self.ctx.clone());
        self.state = events::locate(&self.state, id, &dims);
    }

    /// Draw the Ctrl+F search box and record its area and rows in view
    /// units, so driver and real pointer input see the same capture.
    fn draw_search(&mut self, ctx: &egui::Context) {
        if !self.search.active {
            return;
        }
        self.search.sync(self.state.revision);
        let origin = ctx.content_rect().min;
        let field = egui::Id::unique("uml-search-field");
        // Driver text/paste go through egui's own event path into the field.
        let events = std::mem::take(&mut self.text_events);
        if !events.is_empty() {
            ctx.input_mut(|i| i.events.extend(events));
        }
        let results = self.search.results(self.state.document.as_ref());
        let mut rows = Vec::new();
        let mut chosen = None;
        let response = egui::Area::new(egui::Id::unique("uml-search"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 44.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(440.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.search.query)
                            .id(field)
                            .hint_text(strings::SEARCH)
                            .font(FontId::new(theme::fonts::BODY, FontFamily::Monospace))
                            .desired_width(f32::INFINITY),
                    );
                    if self.search.focus_request {
                        edit.request_focus();
                        self.search.focus_request = false;
                    }
                    for m in results.iter().take(12) {
                        let selected = self.search.selection.as_ref() == Some(&m.id);
                        let row = ui.selectable_label(selected, &m.label);
                        if row.clicked() {
                            chosen = Some(m.id.clone());
                        }
                        rows.push((view_rect(row.rect, origin), m.id.clone()));
                    }
                    if results.is_empty() && !self.search.query.trim().is_empty() {
                        ui.weak(strings::SEARCH_EMPTY);
                    } else if !results.is_empty() {
                        ui.weak(strings::SEARCH_OPEN);
                    }
                });
            });
        self.search.area = Some(view_rect(response.response.rect, origin));
        self.search.rows = rows;
        if let Some(id) = chosen {
            self.search.selection = Some(id.clone());
            self.locate(&id);
        }
    }

    /// Apply what the inspector, card and source windows asked for.
    fn apply_actions(&mut self, actions: Vec<Action>, ctx: &egui::Context) {
        if actions.is_empty() {
            return;
        }
        let outbox = companion::mailbox::Mailbox::outbox(&self.project_root);
        for action in actions {
            match action {
                Action::Choose(id) => {
                    let choice = inspector::choose(&self.state, id.as_ref());
                    self.state = choice.state;
                    let extra: Vec<(Value, Value)> = match &choice.context {
                        Value::Map(entries) => entries.clone(),
                        _ => Vec::new(),
                    };
                    if let Err(error) = outbox.append("context", &extra) {
                        eprintln!("uml viewer: could not send the context: {error}");
                    }
                    self.state.discussion_context = Some(choice.context);
                }
                Action::NewProposal => {
                    let now = clock::now();
                    self.state = events::add_proposal(
                        &self.state,
                        now,
                        &clock::proposal_name(now),
                        Some(&project_files::Files),
                    );
                    self.gui.invalidate();
                }
                Action::Declutter => self.state = events::cycle_declutter(&self.state),
                Action::Regen => {
                    // The durable queue is authoritative; the wake tells the
                    // agent there is something to consume. Without a helper
                    // the request still waits in the queue (#142).
                    self.status = Some(match outbox.append("regen", &[]) {
                        Ok(cmd) => {
                            let id = companion::record::get_i64(&cmd, "id").unwrap_or(0);
                            let woken = self.companion.as_mut().is_some_and(|c| c.wake(id).is_ok());
                            if woken {
                                "Regen requested.".into()
                            } else {
                                "Regen queued; Claude session not attached.".into()
                            }
                        }
                        Err(_) => "Regen failed.".into(),
                    });
                    self.mail_watch.baseline(&self.project_root);
                }
                Action::Rename(id, new_name) => {
                    self.state = events::rename_proposal(
                        &self.state,
                        Some(&id),
                        &new_name,
                        Some(&project_files::Files),
                    );
                    self.gui.invalidate();
                }
                Action::Delete(id) => {
                    self.state = events::delete_proposal(
                        &self.state,
                        Some(&id),
                        Some(&project_files::Files),
                    );
                    self.gui.invalidate();
                }
                Action::CloseCard => self.state = events::close_detail(&self.state),
                Action::Mail(op) => {
                    // The right-click menu writes the exact `to-agent.edn`
                    // command and wakes the companion for that delivery.
                    let (op, extra) =
                        mailbox::menu_mail(&self.state, self.state.selected.as_ref(), &op);
                    match outbox.append(&op, &extra) {
                        Ok(cmd) => {
                            let id = companion::record::get_i64(&cmd, "id").unwrap_or(0);
                            if let Some(companion) = &mut self.companion
                                && let Err(error) = companion.wake(id)
                            {
                                eprintln!("uml viewer: could not wake the companion: {error}");
                            }
                        }
                        Err(error) => {
                            self.status = Some(format!("Mail failed: {error}"));
                        }
                    }
                    self.mail_watch.baseline(&self.project_root);
                }
                Action::Locate(id) => self.locate(&id),
                Action::OpenSource(ident) => {
                    if let Err(reason) = self.gui.open_source(&ident, &self.project_root) {
                        self.status = Some(strings::SOURCE_NOT_OPENED.replace("%s", &reason));
                    }
                }
            }
        }
        self.publish();
        ctx.request_repaint();
    }

    /// The point a GUI step targets, in screen points.
    fn gui_target(&self, target: &driver::GuiTarget) -> Result<egui::Pos2, String> {
        let origin = self.ctx.content_rect().min;
        let (x, y) = match (&target.at, target.xy) {
            (Some(key), _) => {
                let r = self
                    .gui
                    .bounds
                    .get(key)
                    .ok_or_else(|| format!("missing GUI widget {key}"))?;
                match &target.prefix {
                    Some(prefix) => {
                        let w = paint::measure(&self.ctx, theme::fonts::CODE, prefix);
                        (r.x + w, r.y + r.h / 2.0)
                    }
                    None => (r.x + r.w / 2.0, r.y + r.h / 2.0),
                }
            }
            (None, Some([x, y])) => (x, y),
            (None, None) => return Err("GUI step without a target".into()),
        };
        Ok(egui::Pos2::new(
            origin.x + (x + target.offset[0]) as f32,
            origin.y + (y + target.offset[1]) as f32,
        ))
    }

    fn gui_button_event(&self, secondary: bool, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: self.gui_pointer,
            button: if secondary {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            },
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Font source, role sizes and glyph coverage of the UI table plus
    /// `UML_FONT_TEST_EXTRA` and an optional extra probe.
    fn check_fonts(&mut self, extra: Option<&str>) -> Result<(), String> {
        let source = self.font.as_ref().map(|(_, s)| *s).unwrap_or("none");
        if let Some(expect) = env_nonempty("UML_FONT_TEST_EXPECT")
            && expect != source
        {
            return Err(format!("font source {source} expected {expect}"));
        }
        let roles = [
            ("body", theme::fonts::BODY, 18.0),
            ("title", theme::fonts::TITLE, 18.0),
            ("badge", theme::fonts::BADGE, 9.0),
            ("level", theme::fonts::LEVEL, 11.0),
            ("marker", theme::fonts::MARKER, 12.0),
            ("code", theme::fonts::CODE, 13.0),
        ];
        let probe = strings::glyph_text()
            + &env_nonempty("UML_FONT_TEST_EXTRA").unwrap_or_default()
            + extra.unwrap_or_default();
        let mut seen = Vec::new();
        for c in probe.chars() {
            if (c as u32) > 32 && !c.is_whitespace() && !seen.contains(&c) {
                seen.push(c);
            }
        }
        let mut missing = Vec::new();
        for (role, size, expected) in roles {
            if (size - expected).abs() > 0.01 {
                return Err(format!("{role} font size {size}"));
            }
            let font = FontId::new(size, FontFamily::Monospace);
            self.ctx.fonts_mut(|f| {
                for &c in &seen {
                    if !f.has_glyph(&font, c) {
                        missing.push(format!("{role}:U+{:04X}", c as u32));
                    }
                }
            });
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("missing glyphs {}", missing.join(" ")))
        }
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

    /// What the mail driver observes: the cursor, whether the canvas still
    /// waits for `:display`, the loaded title and the discussion context.
    fn mail_record(&self) -> Value {
        let kw = Value::keyword;
        Value::Map(vec![
            (kw("seen"), Value::Integer(self.state.mail_seen)),
            (kw("waiting"), Value::Bool(self.state.waiting)),
            (
                kw("path"),
                self.state.path.clone().map_or(Value::Nil, Value::String),
            ),
            (
                kw("title"),
                self.state
                    .document
                    .as_ref()
                    .and_then(|d| get(d, "title"))
                    .cloned()
                    .unwrap_or(Value::Nil),
            ),
            (
                kw("context"),
                self.state
                    .discussion_context
                    .as_ref()
                    .and_then(|c| get(c, "context"))
                    .cloned()
                    .unwrap_or(Value::Nil),
            ),
        ])
    }

    /// The 250 ms mail loop (`--backend herdr`): consume worker events, then
    /// the `to-viewer.edn` queue. A poll with nothing new does no work and
    /// asks for no repaint, so an idle viewer stays idle (#142).
    fn poll_mail(&mut self, ctx: &egui::Context) {
        if self.companion.is_none() {
            return;
        }
        let found = self.mail_watch.poll(&self.project_root);
        let mut changed = false;
        if found.worker {
            let ready = self
                .companion
                .as_mut()
                .is_some_and(|companion| companion.poll_worker());
            changed = true;
            if ready && !self.companion_recorded {
                // Identity is written once, after the companion is ready.
                if let Err(error) = identity::record(&self.project_root) {
                    eprintln!("uml viewer: could not record the process identity: {error}");
                }
                self.companion_recorded = true;
            }
        }
        if found.mail {
            let consumed = match &mut self.companion {
                Some(companion) => {
                    let (next, consumed) = companion.consume(&self.state);
                    if consumed {
                        let path_changed = next.path != self.state.path;
                        self.state = next;
                        if let Some(path) = self.state.path.clone() {
                            self.diagram = PathBuf::from(path);
                        }
                        if path_changed {
                            self.hint = None;
                        }
                    }
                    consumed
                }
                None => false,
            };
            if consumed {
                self.gui.invalidate();
                self.publish();
                changed = true;
            }
            // The pop rewrote the file this watcher reads.
            self.mail_watch.baseline(&self.project_root);
        }
        let status = self.companion.as_ref().and_then(|c| c.status.clone());
        if let Some(status) = status
            && self.status.as_deref() != Some(status.as_str())
        {
            self.status = Some(status);
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
    }

    /// A normal close asks the helper to stop without keeping the companion;
    /// the helper then cancels recovery, removes its Herdr session and clears
    /// the ownership record. A viewer without a helper has nothing to stop.
    fn begin_normal_quit(&mut self, ctx: &egui::Context) {
        if self.normal_stop {
            return;
        }
        self.normal_stop = true;
        self.stop_deadline = Some(Instant::now() + Duration::from_secs(5));
        if let Some(companion) = &mut self.companion
            && let Err(error) = companion.stop(false)
        {
            eprintln!("uml viewer: could not stop the companion: {error}");
        }
        if self.companion.as_ref().is_none_or(|c| c.settled()) {
            self.quit = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Wait (bounded) for the helper to settle, then exit 0.
    fn settle_normal_quit(&mut self, ctx: &egui::Context) {
        let settled = match &mut self.companion {
            Some(companion) => companion.poll_worker() || companion.settled(),
            None => true,
        };
        let expired = self.stop_deadline.is_some_and(|at| Instant::now() >= at);
        if settled || expired {
            self.quit = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            // Poll the helper's events while it stops.
            ctx.request_repaint_after(POLL_INTERVAL);
        }
    }

    /// `:quit-for-restart`: stop the helper with `keep` (a failed helper needs
    /// no stop and must not strand the restart), save the session, exit 0.
    fn quit_for_restart(&mut self, ctx: &egui::Context) {
        if let Some(companion) = &mut self.companion {
            if !self.restart_stop {
                if let Err(error) = companion.stop(true) {
                    eprintln!("uml viewer: could not stop the companion: {error}");
                }
                self.restart_stop = true;
            }
            // Poll until the helper settled, so the replacement viewer binds
            // the companion that is actually there.
            companion.poll_worker();
            if !companion.settled() {
                ctx.request_repaint_after(POLL_INTERVAL);
                return;
            }
        }
        let chrome = session::Chrome {
            window_w: Some(self.window_w),
            window_h: Some(self.window_h),
            inspector_w: Some(self.gui.inspector_width()),
        };
        let snapshot = session::snapshot(&self.state, &chrome);
        if let Err(error) = session::save(&self.project_root, &snapshot) {
            eprintln!("uml viewer: could not save the session: {error}");
        }
        self.exit.store(0, Ordering::SeqCst);
        self.quit = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
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

    /// Undecorated fallback: no title bar, so the drag strip carries
    /// minimise and close. Close is the normal close a window-manager close
    /// takes (companion stop without keep, exit 0).
    fn draw_window_controls(&mut self, ctx: &egui::Context) {
        if self.decorated {
            return;
        }
        let screen = ctx.content_rect();
        let [minimise, close] = window_controls(screen);
        let quiet = |ui: &mut egui::Ui, rect: egui::Rect, label: &str, hot: theme::Rgb| {
            let response = ui.interact(rect, ui.scope_id().with(label), egui::Sense::click());
            let painter = ui.painter();
            if response.hovered() {
                painter.rect_filled(rect, 3.0, theme::LINE.color32());
            }
            let ink = if response.hovered() {
                hot
            } else {
                theme::MUTED
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(14.0),
                ink.color32(),
            );
            response.clicked()
        };
        let (minimised, closed) = egui::Area::new(egui::Id::unique("uml-window-controls"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                (
                    quiet(ui, minimise, "—", theme::INK),
                    quiet(ui, close, "×", theme::VIOLATION_HOT),
                )
            })
            .inner;
        self.gui
            .record_bound("window-minimize", minimise, screen.min);
        self.gui.record_bound("window-close", close, screen.min);
        if minimised {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if closed && !self.quit {
            self.begin_normal_quit(ctx);
        }
    }

    /// Undecorated fallback: the top strip drags, the edges resize. Returns
    /// true when the press was consumed by a window region.
    fn window_chrome_input(&mut self, ctx: &egui::Context) -> bool {
        if self.decorated || self.driver.is_some() {
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
        } else if window_controls(screen).iter().any(|r| r.contains(pointer)) {
            // The minimise/close buttons take the press (egui sees it); the
            // canvas and the drag must not.
            return true;
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
            // View units: points from the canvas origin.
            let (x, y) = ((p.x - screen.min.x) as f64, (p.y - screen.min.y) as f64);
            let dragging = self.state.drag.is_some();
            if !dragging && (self.search.captures(x, y) || self.gui.captures(x, y, self.window_w)) {
                // egui owns the search box; the canvas sees nothing.
                self.state = events::on_leave(&self.state);
            } else if (pressed || double) && self.chrome_press(x, y) {
                // Consumed by the Declutter control.
            } else if double {
                self.state = events::on_double_press(&self.state, x, y);
            } else if pressed {
                self.state = events::on_left_press(&self.state, x, y);
            } else if down && dragging {
                self.state = events::on_drag(&self.state, x, y);
            } else if !down {
                self.state = events::on_move(&self.state, x, y);
            }
            if released && self.state.drag.is_some() {
                self.state = events::on_release(&self.state, x, y);
            }
        } else if !down {
            self.state = events::on_leave(&self.state);
        }
        let over_search = pos.is_some_and(|p| {
            let (x, y) = ((p.x - screen.min.x) as f64, (p.y - screen.min.y) as f64);
            self.search.captures(x, y)
                || (self.gui.captures(x, y, self.window_w) && !self.gui.card_hovered())
        });
        // egui turns ctrl+wheel into a zoom factor; convert it back to wheel
        // notches so the canvas zooms by ZOOM_STEP^notches at the cursor.
        let zoom = ctx.input(|i| i.zoom_delta()) as f64;
        if zoom != 1.0 && zoom > 0.0 && !over_search {
            self.pending.push(Input::Wheel {
                dx: 0.0,
                dy: zoom.ln() / events::ZOOM_STEP.ln(),
                ctrl: true,
                shift: false,
            });
        }
        let wheel = ctx.input(|i| i.smooth_scroll_delta);
        if wheel != egui::Vec2::ZERO && !over_search {
            self.pending.push(Input::Wheel {
                dx: wheel.x as f64,
                // egui reports points; LOVE wheelmoved reports notches.
                dy: (wheel.y / 50.0) as f64,
                ctrl: false,
                shift: false,
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
        // A focused rename field owns the keyboard (the search box routes its
        // own keys through search_key).
        let keys = if ctx.text_edit_focused() && !self.search.active {
            Vec::new()
        } else {
            keys
        };
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
        } else if let Some(seconds) = step.wake_s {
            ctx.request_repaint_after(Duration::from_secs_f64(seconds));
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
        egui::Key::F => "f",
        egui::Key::Enter => "enter",
        _ => "",
    }
}

/// An egui screen rect in view units (points from the canvas origin).
fn view_rect(r: egui::Rect, origin: egui::Pos2) -> Rect {
    Rect::new(
        (r.min.x - origin.x) as f64,
        (r.min.y - origin.y) as f64,
        r.width() as f64,
        r.height() as f64,
    )
}

/// EDN number: an integer when it is one (window sizes, scales).
fn edn_number(x: f64) -> Value {
    if (x - x.round()).abs() < 1e-3 {
        Value::Integer(x.round() as i64)
    } else {
        Value::Float(x)
    }
}

fn keywords_of(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .map(|v| local_name(v).unwrap_or_default().to_owned())
        .collect()
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
    fn on_exit(&mut self) {
        if let Some(waker) = &self.waker {
            waker.stop();
        }
        // A window close the frame loop did not see (WM quit): ask the
        // helper to stop without keeping the companion. It then releases the
        // session and ownership on its own.
        if let Some(companion) = &mut self.companion
            && !companion.settled()
            && let Err(error) = companion.stop(false)
        {
            eprintln!("uml viewer: could not stop the companion: {error}");
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.take_gui_batch(raw_input);
    }

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
            // A finished script's `:quit` is the verdict, exactly like the
            // Lua drivers' `love.event.quit(0)`: the companion's normal stop
            // must not turn a passed driver into a failure.
            let finished = runner.is_done() && !self.driver_failed;
            self.driver = Some(runner);
            if self.quit || finished {
                self.quit = true;
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
        // Mail is polled only when the waker saw a change (or at startup):
        // an unchanged project does no mailbox work and no repaint.
        let mail_due = self.frames <= 2 || self.waker.as_ref().is_some_and(|waker| waker.take());
        if mail_due {
            self.poll_mail(ctx);
        }
        // A restart request saves the session and exits; a stopped helper
        // keeps being polled so the replacement viewer never binds a ghost.
        if self.state.quit_for_restart || self.restart_stop {
            self.quit_for_restart(ctx);
        }
        if self.state.fit_pending {
            let dims = self.dims(ctx);
            self.state = events::settle(&self.state, &dims);
        }

        let full = ui.max_rect();
        let rect = full.with_max_x(full.min.x + self.gui.canvas_width(full.width() as f64) as f32);
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
            &mut self.render_cache,
            &mut self.text_cache,
        );
        self.draw_search(ctx);
        let actions = self.gui.draw(ctx, &self.state, self.frames);
        self.apply_actions(actions, ctx);
        self.draw_window_controls(ctx);
        self.gui_inflight = false;
        if self.driver.is_some() || !self.gui_batches.is_empty() {
            // Replayed input needs frames; a real session repaints on input.
            ctx.request_repaint();
        }

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

        // A termination signal is a normal close: release the companion and
        // exit through the same path a window close takes.
        if signal::terminated() && !self.quit {
            self.begin_normal_quit(ctx);
        }
        if self.normal_stop {
            self.settle_normal_quit(ctx);
        }
        if let Some(seconds) = self.test_seconds
            && self.started.elapsed().as_secs_f64() >= seconds
        {
            // The test duration is the normal close: the companion is
            // stopped here and `settle_normal_quit` closes the window.
            self.begin_normal_quit(ctx);
        }
        // Idle: no cadence; input and the wake watcher repaint. Drivers,
        // timed runs and pending screenshots keep the poll cadence.
        let shots_pending = self.shots_requested.len() > self.shots_sent.len()
            || (self.evidence.is_some() && self.frames < 4);
        if self.bench.is_none()
            && (self.driver.is_some() || self.test_seconds.is_some() || shots_pending)
        {
            ctx.request_repaint_after(POLL_INTERVAL);
        }
    }
}

fn fps(ctx: &egui::Context) -> u32 {
    let dt = ctx.input(|i| i.stable_dt).max(1e-6);
    (1.0 / dt).round() as u32
}

impl Viewer {
    /// Deliver one batch of replayed GUI events at the start of a frame, so
    /// egui handles them as real input (the Lua drivers fed imgui's IO).
    fn take_gui_batch(&mut self, raw_input: &mut egui::RawInput) {
        if let Some(batch) = self.gui_batches.pop_front() {
            raw_input.events.extend(batch);
            self.gui_inflight = true;
        }
    }
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

    fn view_record(&mut self, anchor: Option<[f64; 2]>) -> ViewRecord {
        let s = &self.state;
        let id_of = |v: &Option<Value>| {
            v.as_ref().map(|v| {
                get(v, "id")
                    .map(|v| local_name(v).unwrap_or_default().to_owned())
                    .unwrap_or_else(|| local_name(v).unwrap_or_default().to_owned())
            })
        };
        let results = self.search.results(s.document.as_ref());
        ViewRecord {
            focus: keywords_of(&s.focus),
            zoom: events::zoom_of(s),
            cam_x: s.cam_x,
            cam_y: s.cam_y,
            selected: id_of(&s.selected),
            detail: s
                .detail_id
                .as_ref()
                .map(|v| local_name(v).unwrap_or_default().to_owned()),
            declutter: get(&s.extra, "declutter")
                .map(|v| local_name(v).unwrap_or_default().to_owned())
                .unwrap_or_else(|| "full".into()),
            dragging: s.drag.is_some(),
            anchor: anchor.map(|[x, y]| events::world_xy(s, x, y)),
            search: self.search.record(&results),
        }
    }

    fn is_fitted(&self) -> bool {
        let fitted = events::fit(&self.state, &self.dims(&self.ctx));
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        !self.state.fit_pending
            && near(fitted.zoom, self.state.zoom)
            && near(fitted.cam_x, self.state.cam_x)
            && near(fitted.cam_y, self.state.cam_y)
    }

    fn is_blank(&self, x: f64, y: f64) -> bool {
        let (wx, wy) = events::world_xy(&self.state, x, y);
        hit::at(&self.state.scene, wx, wy).is_none() && !self.gui.captures(x, y, self.window_w)
    }

    fn click_class(&mut self, id: &str, double: bool, secondary: bool) -> Result<(), String> {
        // The compiled scene prefixes ids per section (`d0-`); the authored
        // IR id is what the driver names, so resolve through the suffix.
        let wanted = Value::keyword(id);
        let class = hit::class_by_id(&self.state.scene, &wanted)
            .or_else(|| {
                let suffix = format!("-{id}");
                edn::record::seq(get(&self.state.scene, "classes"))
                    .iter()
                    .find(|c| {
                        get(c, "id")
                            .map(|v| local_name(v).unwrap_or_default().to_owned())
                            .is_some_and(|n| n.ends_with(&suffix))
                    })
            })
            .ok_or_else(|| format!("no class {id} on the scene"))?;
        let r = get(class, "rect")
            .and_then(Rect::from_value)
            .ok_or_else(|| format!("class {id} has no rect"))?;
        // A few units inside the box under the current camera (Lua on_class).
        let z = events::zoom_of(&self.state);
        let (x, y) = (
            (r.x + 6.0 - self.state.cam_x) * z,
            (r.y + 6.0 - self.state.cam_y) * z,
        );
        if secondary {
            // A right click through egui's own pointer path: the canvas menu
            // opens on the release, where the user's right click lands.
            let origin = self.ctx.content_rect().min;
            let pos = egui::Pos2::new(origin.x + x as f32, origin.y + y as f32);
            self.gui_pointer = pos;
            self.gui_batches
                .push_back(vec![egui::Event::PointerMoved(pos)]);
            self.gui_batches
                .push_back(vec![self.gui_button_event(true, true)]);
            self.gui_batches
                .push_back(vec![self.gui_button_event(true, false)]);
            return Ok(());
        }
        self.pending.push(if double {
            Input::DoubleClick { x, y }
        } else {
            Input::Click { x, y }
        });
        Ok(())
    }

    fn click_declutter(&mut self) {
        // The control lives in the inspector: a real egui click on it.
        let target = driver::GuiTarget {
            at: Some("declutter".into()),
            xy: None,
            offset: [0.0, 0.0],
            prefix: None,
        };
        if let Err(error) = self.gui_click(&target, false) {
            eprintln!("uml viewer: {error}");
        }
    }

    fn assert_fonts(&mut self) -> Result<(), String> {
        self.check_fonts(None)
    }

    fn assert_fonts_with(&mut self, extra: Option<&str>) -> Result<(), String> {
        self.check_fonts(extra)
    }

    fn busy(&self) -> bool {
        // Until the GUI has drawn the last batch its effect is not visible.
        !self.gui_batches.is_empty() || self.gui_inflight
    }

    fn gui_point(&mut self, target: &driver::GuiTarget) -> Result<(), String> {
        let pos = self.gui_target(target)?;
        self.gui_pointer = pos;
        self.gui_batches
            .push_back(vec![egui::Event::PointerMoved(pos)]);
        Ok(())
    }

    fn gui_button(&mut self, secondary: bool, pressed: bool) {
        let event = self.gui_button_event(secondary, pressed);
        self.gui_batches.push_back(vec![event]);
    }

    fn gui_click(&mut self, target: &driver::GuiTarget, secondary: bool) -> Result<(), String> {
        let pos = self.gui_target(target)?;
        self.gui_pointer = pos;
        self.gui_batches
            .push_back(vec![egui::Event::PointerMoved(pos)]);
        let press = self.gui_button_event(secondary, true);
        let release = self.gui_button_event(secondary, false);
        self.gui_batches.push_back(vec![press]);
        self.gui_batches.push_back(vec![release]);
        Ok(())
    }

    fn gui_key(&mut self, key: &str, ctrl: bool, shift: bool) {
        let Some(code) =
            egui::Key::from_name(&key.to_uppercase()).or_else(|| egui::Key::from_name(key))
        else {
            eprintln!("uml viewer: unknown driver key {key}");
            return;
        };
        let modifiers = egui::Modifiers {
            ctrl,
            command: ctrl,
            shift,
            ..egui::Modifiers::NONE
        };
        let event = |pressed| egui::Event::Key {
            key: code,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        let mut batch = vec![egui::Event::ModifiersChanged(modifiers), event(true)];
        if ctrl && code == egui::Key::C {
            batch.push(egui::Event::Copy);
        }
        batch.push(event(false));
        batch.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        self.gui_batches.push_back(batch);
    }

    fn gui_record(&self) -> Value {
        let mut record = self.gui.record(&self.state);
        record.assoc(
            Value::keyword("status"),
            self.status.clone().map_or(Value::Nil, Value::String),
        );
        record.assoc(
            Value::keyword("window"),
            Value::Map(vec![
                (Value::keyword("w"), edn_number(self.window_w)),
                (Value::keyword("h"), edn_number(self.window_h)),
            ]),
        );
        record.assoc(Value::keyword("mail"), self.mail_record());
        record.assoc(Value::keyword("session"), self.session_record());
        record
    }

    fn open_source(&mut self, lang: &str, file: &str, name: Option<&str>) -> Result<(), String> {
        let ident = source::Ident {
            lang: Some(lang.into()),
            file: Some(file.into()),
            ns: None,
            name: name.map(Into::into),
        };
        self.gui.open_source(&ident, &self.project_root)
    }

    fn resize_card(&mut self, w: f64, h: f64) {
        self.gui.card.force = Some((egui::vec2(w as f32, h as f32), 3));
    }

    fn widget_pixels(&self, key: &str) -> Option<[f64; 4]> {
        let r = self.gui.bounds.get(key)?;
        let origin = self.ctx.content_rect().min;
        let ppp = self.ctx.pixels_per_point() as f64;
        let (x, y) = (origin.x as f64 + r.x, origin.y as f64 + r.y);
        Some([x * ppp, y * ppp, (x + r.w) * ppp, (y + r.h) * ppp])
    }

    fn assert_text_fits(&mut self) -> Result<(), String> {
        let ctx = self.ctx.clone();
        let measure = |s: f64, t: &str| paint::measure(&ctx, s as f32, t);
        let mut count = 0;
        let commands = commands::commands(
            &self.state.scene,
            self.state.selected.as_ref(),
            self.state.hover.as_ref(),
        );
        for command in &commands {
            let (text, size, h, max_w, bold, italic) = match command {
                Command::Text {
                    text,
                    size,
                    h,
                    max_w,
                    bold,
                    italic,
                    ..
                } => (text, *size, h.unwrap_or(*size), *max_w, *bold, *italic),
                Command::Package {
                    text, size, max_w, ..
                } => (text, *size, *size, *max_w, false, true),
                _ => continue,
            };
            if text.is_empty() {
                continue;
            }
            let fitted = commands::fit(measure, size.min(h), text, max_w, bold, italic);
            if fitted > 18.0 || fitted > h + 1e-6 {
                return Err(format!("text exceeds vertical slot: {text}"));
            }
            let width = measure(fitted, text)
                + if bold { fitted * 0.05 } else { 0.0 }
                + if italic { fitted * 0.2 } else { 0.0 };
            if width > max_w + 1e-4 {
                return Err(format!(
                    "text exceeds horizontal slot: {text} (size {size} -> {fitted}, width {width} > {max_w})"
                ));
            }
            count += 1;
        }
        if count == 0 {
            return Err("empty capture scene".into());
        }
        Ok(())
    }

    fn session_record(&self) -> Value {
        let chrome = session::Chrome {
            window_w: Some(self.window_w),
            window_h: Some(self.window_h),
            inspector_w: Some(self.gui.inspector_width()),
        };
        session::snapshot(&self.state, &chrome)
    }

    fn display_record(&self) -> Value {
        let screen = self.ctx.content_rect();
        let kw = Value::keyword;
        Value::Map(vec![
            (kw("ui"), edn_number(self.ctx.zoom_factor() as f64)),
            (
                kw("pixels-per-point"),
                edn_number(self.ctx.pixels_per_point() as f64),
            ),
            (
                kw("window"),
                Value::Map(vec![
                    (kw("w"), edn_number(screen.width() as f64)),
                    (kw("h"), edn_number(screen.height() as f64)),
                ]),
            ),
            (
                kw("fonts"),
                Value::Map(vec![
                    (kw("body"), edn_number(theme::fonts::BODY as f64)),
                    (kw("title"), edn_number(theme::fonts::TITLE as f64)),
                    (kw("badge"), edn_number(theme::fonts::BADGE as f64)),
                    (kw("level"), edn_number(theme::fonts::LEVEL as f64)),
                    (kw("marker"), edn_number(theme::fonts::MARKER as f64)),
                ]),
            ),
            (
                kw("font-source"),
                kw(self.font.as_ref().map(|(_, s)| *s).unwrap_or("none")),
            ),
            (kw("decorated"), Value::Bool(self.decorated)),
        ])
    }

    fn quality_line(&self, id: &str) -> Option<String> {
        let target = Value::keyword(id);
        commands::commands(&self.state.scene, None, None)
            .into_iter()
            .find_map(|c| match c {
                Command::Class {
                    id: Some(cid),
                    missing,
                    band,
                    grade,
                    stroke,
                    ..
                } if cid == target => Some(format!(
                    "Detail missing={missing} band={band} continuousGrade={} stroke={},{},{}",
                    grade.map_or("nil".to_string(), |g| g.to_string()),
                    stroke.0,
                    stroke.1,
                    stroke.2
                )),
                _ => None,
            })
    }

    fn resize_window(&mut self, w: f64, h: f64) {
        self.ctx
            .send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(w as f32, h as f32)));
    }
}
