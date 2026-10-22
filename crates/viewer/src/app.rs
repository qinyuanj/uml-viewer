//! eframe application: window, hot reload, evidence publishing, driver
//! replay, input mapping, and the undecorated-window drag/resize regions.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use edn::Value;
use eframe::egui;
use egui::{FontFamily, FontId, ViewportCommand};
use scene::document::{self, ViewState};
use scene::events::{self, Dims};
use scene::geom::{Rect, get, name, num};
use scene::hit;

use crate::commands::{self, Command};
use crate::driver::{self, Input, Runner, ViewRecord};
use crate::evidence::Evidence;
use crate::search_box::SearchBox;
use crate::{cli::Options, fonts, identity, metrics, paint, strings, theme, watch};

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
    // A simulated scale also scales the window, so the canvas stays
    // 1500x920 points (Lua `display.window_size`: view units × ui).
    let scale = dpi_override().unwrap_or(1.0);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size([WINDOW_SIZE[0] * scale, WINDOW_SIZE[1] * scale])
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
            window_w: WINDOW_SIZE[0] as f64,
            window_h: WINDOW_SIZE[1] as f64,
            shots_sent: Vec::new(),
            font,
            search: SearchBox::default(),
            text_events: Vec::new(),
            driver_pointer: None,
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
                }
                return;
            }
            Input::Paste(text) => {
                if self.search.active {
                    self.text_events.push(egui::Event::Paste(text));
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
        if self.state.document.is_some()
            && !self.state.waiting
            && contains(commands::declutter_rect(self.window_w), x, y)
        {
            self.state = events::cycle_declutter(&self.state);
            return true;
        }
        false
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
        if self.driver.is_some() {
            // A driver owns the event stream; real input would race it.
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
            if !dragging && self.search.captures(x, y) {
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
            self.search
                .captures((p.x - screen.min.x) as f64, (p.y - screen.min.y) as f64)
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

fn contains(r: Rect, x: f64, y: f64) -> bool {
    x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom()
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
    items.iter().map(name).collect()
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.frames += 1;
        let now = Instant::now();
        self.frame_ms = now.duration_since(self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;

        // Driver replay: take the runner out so it can borrow the app as host.
        if self.driver.is_some() && !self.driver_failed {
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
        self.draw_search(ctx);

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

    fn view_record(&mut self, anchor: Option<[f64; 2]>) -> ViewRecord {
        let s = &self.state;
        let id_of = |v: &Option<Value>| {
            v.as_ref()
                .map(|v| get(v, "id").map(name).unwrap_or_else(|| name(v)))
        };
        let results = self.search.results(s.document.as_ref());
        ViewRecord {
            focus: keywords_of(&s.focus),
            zoom: events::zoom_of(s),
            cam_x: s.cam_x,
            cam_y: s.cam_y,
            selected: id_of(&s.selected),
            detail: s.detail_id.as_ref().map(name),
            declutter: get(&s.extra, "declutter")
                .map(name)
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
        hit::at(&self.state.scene, wx, wy).is_none()
            && !contains(commands::declutter_rect(self.window_w), x, y)
    }

    fn click_class(&mut self, id: &str, double: bool) -> Result<(), String> {
        let class = hit::class_by_id(&self.state.scene, &Value::keyword(id))
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
        self.pending.push(if double {
            Input::DoubleClick { x, y }
        } else {
            Input::Click { x, y }
        });
        Ok(())
    }

    fn click_declutter(&mut self) {
        let r = commands::declutter_rect(self.window_w);
        self.pending.push(Input::Click {
            x: r.x + r.w / 2.0,
            y: r.y + r.h / 2.0,
        });
    }

    fn assert_fonts(&mut self) -> Result<(), String> {
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
        ];
        let probe =
            strings::glyph_text() + &env_nonempty("UML_FONT_TEST_EXTRA").unwrap_or_default();
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
