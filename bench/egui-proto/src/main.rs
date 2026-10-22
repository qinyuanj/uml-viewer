//! PROTOTYPE (#131), throwaway: egui/eframe renderer for bench/ scene JSON,
//! measured with protocol 1 (bench/README.md). Not production code.
//!
//! Usage: egui-bench-proto <scene.json> <out-dir>
//! Env:   BENCH_VSYNC=0|1 (default 1), BENCH_SHOTS=1 (screenshots only),
//!        BENCH_TEXT_QUANT=px (font size quantum, default 1; 0 = unquantized)
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Pos2,
    Rect, Shape, Stroke, StrokeKind, Vec2,
};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

#[derive(Deserialize)]
struct Scene {
    size: Size,
    packages: Vec<Package>,
    classes: Vec<Class>,
    edges: Vec<Edge>,
}
#[derive(Deserialize)]
struct Size {
    w: f32,
    h: f32,
    #[serde(rename = "min-x")]
    min_x: f32,
    #[serde(rename = "min-y")]
    min_y: f32,
}
#[derive(Deserialize)]
struct Package {
    label: String,
    rect: [f32; 4],
    fill: [u8; 3],
    stroke: [u8; 3],
}
#[derive(Deserialize)]
struct Class {
    rect: [f32; 4],
    level: Option<i32>,
    missing: bool,
    fill: [u8; 3],
    stroke: [u8; 3],
    #[serde(rename = "crap-band")]
    crap_band: String,
    #[serde(rename = "mutation-band")]
    mutation_band: String,
    lines: Vec<Line>,
}
#[derive(Deserialize)]
struct Line {
    kind: String,
    text: Option<String>,
}
#[derive(Deserialize)]
struct Edge {
    kind: String,
    violating: bool,
    strokes: Vec<Vec<f32>>,
    tip: [f32; 2],
    behind: [f32; 2],
}

// Theme (love/uml/domain/theme.lua).
const BG: Color32 = Color32::from_rgb(22, 28, 32);
const GOLD: Color32 = Color32::from_rgb(232, 196, 72);
const INK: Color32 = Color32::from_rgb(236, 236, 228);
const MUTED: Color32 = Color32::from_rgb(157, 184, 168);
const EDGE: Color32 = Color32::from_rgb(157, 184, 168);
const EDGE_BAD: Color32 = Color32::from_rgb(196, 42, 36);
const SIDEBAR_W: f32 = 280.0; // canvas 1220 px wide, as LOVE

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
fn rgba(c: [u8; 3], a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}
fn band(b: &str) -> Color32 {
    match b {
        "green" => Color32::from_rgb(95, 181, 138),
        "yellow" => Color32::from_rgb(232, 196, 72),
        _ => Color32::from_rgb(196, 42, 36),
    }
}

/// Edge with its world bounding box, for culling.
struct EdgeGeo {
    bbox: Rect,
    pts: Vec<Vec<Pos2>>,
    tip: Pos2,
    behind: Pos2,
    inherit: bool,
    violating: bool,
}

#[derive(Clone, Copy)]
struct Geo {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    fit: f32,
    zoom_hi: f32,
}

const PHASES: [&str; 5] = ["idle", "pan-detail", "zoom-detail", "pan-fit", "zoom-fit"];
const WARMUP_S: f64 = 3.0;
const MIN_S: f64 = 10.0;
const MIN_FRAMES: usize = 20;
const MAX_S: f64 = 180.0;
const TAU: f64 = std::f64::consts::TAU;

fn trajectory(name: &str, t: f64, g: &Geo) -> Option<(f32, f32, f32)> {
    let (cx, cy) = (g.cx as f64, g.cy as f64);
    Some(match name {
        "pan-detail" => (cx + 1200.0 * (TAU * t / 8.0).cos(), cy + 1200.0 * (TAU * t / 8.0).sin(), 1.0),
        "zoom-detail" => {
            let u = 0.5 - 0.5 * (TAU * t / 4.0).cos();
            (cx, cy, (0.5f64.ln() + u * (2f64.ln() - 0.5f64.ln())).exp())
        }
        "pan-fit" => (
            cx + 0.25 * g.w as f64 * (TAU * t / 8.0).sin(),
            cy + 0.25 * g.h as f64 * (2.0 * TAU * t / 8.0).sin(),
            g.fit as f64,
        ),
        "zoom-fit" => {
            let u = 0.5 - 0.5 * (TAU * t / 4.0).cos();
            (cx, cy, g.fit as f64 * (u * (g.zoom_hi as f64 / g.fit as f64).ln()).exp())
        }
        _ => return None,
    })
    .map(|(x, y, z)| (x as f32, y as f32, z as f32))
}

fn cpu_s() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let rest = s.rsplit_once(") ").map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    let tck = 100.0; // getconf CLK_TCK on this machine
    (f.get(11).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0)
        + f.get(12).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0))
        / tck
}

struct PhaseResult {
    name: &'static str,
    frames: usize,
    wall: f64,
    cpu_pct: f64,
    stats: Option<[f64; 6]>, // fps mean median p95 p99 max
    cpu_ms_p95: Option<f64>,
    zoom: Option<(f32, f32)>,
}

fn stats(mut v: Vec<f64>) -> Option<[f64; 6]> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let total: f64 = v.iter().sum();
    let q = |p: f64| v[((v.len() as f64 * p).ceil() as usize).max(1) - 1];
    Some([1000.0 * v.len() as f64 / total, total / v.len() as f64, q(0.5), q(0.95), q(0.99), v[v.len() - 1]])
}

struct App {
    scene: Scene,
    edges: Vec<EdgeGeo>,
    out: std::path::PathBuf,
    shots: bool,
    quant: f32,
    decim2: f32,
    // camera: world point at canvas centre + zoom
    cam: (f32, f32, f32),
    geo: Option<Geo>,
    started: Option<Instant>,
    last: Option<Instant>,
    phase: Option<(usize, Instant, f64)>,
    samples: Vec<f64>,
    cpu_samples: Vec<f64>,
    zoom_range: Option<(f32, f32)>,
    results: Vec<PhaseResult>,
    meta: String,
    shot_i: usize,
    shot_frames: u32,
    shot_pending: Arc<std::sync::Mutex<Option<Arc<egui::ColorImage>>>>,
    painted: Painted,
    paint_ms: Vec<f64>,
}

#[derive(Default, Clone, Copy)]
struct Painted {
    classes: usize,
    edges: usize,
    texts: usize,
    lod: u8,
}

impl App {
    fn new(cc: &eframe::CreationContext, scene: Scene, out: std::path::PathBuf) -> Self {
        let mut fonts = FontDefinitions::default();
        let cjk = std::fs::read("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc").expect("Noto CJK");
        let mut fd = FontData::from_owned(cjk);
        fd.index = 2; // Noto Sans CJK SC
        fonts.font_data.insert("cjk".into(), Arc::new(fd));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("cjk".into());
        cc.egui_ctx.set_fonts(fonts);

        let edges = scene
            .edges
            .iter()
            .map(|e| {
                let pts: Vec<Vec<Pos2>> = e
                    .strokes
                    .iter()
                    .map(|s| s.chunks_exact(2).map(|p| Pos2::new(p[0], p[1])).collect())
                    .collect();
                let mut bbox = Rect::NOTHING;
                for p in pts.iter().flatten() {
                    bbox.extend_with(*p);
                }
                let tip = Pos2::new(e.tip[0], e.tip[1]);
                bbox.extend_with(tip);
                EdgeGeo {
                    bbox: bbox.expand(20.0),
                    pts,
                    tip,
                    behind: Pos2::new(e.behind[0], e.behind[1]),
                    inherit: e.kind == "inheritance",
                    violating: e.violating,
                }
            })
            .collect();
        let quant = std::env::var("BENCH_TEXT_QUANT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        App {
            scene,
            edges,
            out,
            shots: std::env::var("BENCH_SHOTS").as_deref() == Ok("1"),
            quant,
            decim2: std::env::var("BENCH_DECIM").ok().and_then(|v| v.parse::<f32>().ok()).map_or(64.0, |d| d * d),
            cam: (0.0, 0.0, 1.0),
            geo: None,
            started: None,
            last: None,
            phase: None,
            samples: vec![],
            cpu_samples: vec![],
            zoom_range: None,
            results: vec![],
            meta: String::new(),
            shot_i: 0,
            shot_frames: 0,
            shot_pending: Default::default(),
            painted: Painted::default(),
            paint_ms: vec![],
        }
    }

    fn font_px(&self, world: f32, zoom: f32) -> f32 {
        let s = world * zoom;
        if self.quant > 0.0 {
            (s / self.quant).round() * self.quant
        } else {
            s
        }
    }

    /// LOD bands by zoom: 0 = blocks + package titles; 1 = + strokes, dots,
    /// decimated edges; 2 = full detail (text, arrows, rules).
    fn paint(&mut self, painter: &egui::Painter, canvas: Rect) {
        let (cx, cy, z) = self.cam;
        let origin = canvas.center() - Vec2::new(cx * z, cy * z);
        let to_s = |p: Pos2| Pos2::new(origin.x + p.x * z, origin.y + p.y * z);
        let wr = |r: [f32; 4]| Rect::from_min_size(to_s(Pos2::new(r[0], r[1])), Vec2::new(r[2] * z, r[3] * z));
        // Visible world rect.
        let view = Rect::from_min_max(
            Pos2::new((canvas.min.x - origin.x) / z, (canvas.min.y - origin.y) / z),
            Pos2::new((canvas.max.x - origin.x) / z, (canvas.max.y - origin.y) / z),
        );
        let lod: u8 = if z < 0.15 { 0 } else if z < 0.4 { 1 } else { 2 };
        let mut n = Painted { lod, ..Default::default() };
        let mut shapes: Vec<Shape> = Vec::with_capacity(4096);
        let wvis = |r: [f32; 4]| view.intersects(Rect::from_min_size(Pos2::new(r[0], r[1]), Vec2::new(r[2], r[3])));

        for p in &self.scene.packages {
            if !wvis(p.rect) {
                continue;
            }
            let r = wr(p.rect);
            shapes.push(Shape::rect_filled(r, CornerRadius::same((8.0 * z).min(255.0) as u8), rgba(p.fill, 80)));
            shapes.push(Shape::rect_stroke(r, CornerRadius::same((8.0 * z).min(255.0) as u8), Stroke::new(2.0, rgba(p.stroke, 170)), StrokeKind::Inside));
        }
        // Package titles: fixed 13 px below detail zoom, so the overview stays legible.
        let mut titles: Vec<(Pos2, &str, f32)> = vec![];
        for p in &self.scene.packages {
            if wvis(p.rect) {
                let size = if lod == 2 { self.font_px(18.0, z) } else { 13.0 };
                let r = wr(p.rect);
                if r.width() > 60.0 {
                    titles.push((r.min + Vec2::new(12.0 * z.max(0.3), 8.0 * z.max(0.3)), &p.label, size));
                }
            }
        }

        // Edges (lod >= 1), under the classes as in LOVE.
        if lod >= 1 {
            for e in &self.edges {
                if !view.intersects(e.bbox) {
                    continue;
                }
                n.edges += 1;
                let color = if e.violating { EDGE_BAD } else { EDGE };
                let stroke = Stroke::new(if e.violating { 2.4 } else { 1.6 }, color);
                for s in &e.pts {
                    let mut pts: Vec<Pos2> = Vec::with_capacity(s.len());
                    for p in s {
                        let q = to_s(*p);
                        // Decimate at lod 1: drop points closer than 8 px (BENCH_DECIM) to the last kept;
                        // 3 px left 500 zoom-fit at p95 27.6 ms (egui feathered tessellation).
                        if lod == 1 {
                            if let Some(l) = pts.last() {
                                if l.distance_sq(q) < self.decim2 {
                                    continue;
                                }
                            }
                        }
                        pts.push(q);
                    }
                    if let (Some(l), Some(lastw)) = (pts.last().copied(), s.last()) {
                        let q = to_s(*lastw);
                        if l != q {
                            pts.push(q);
                        }
                    }
                    if pts.len() >= 2 {
                        shapes.push(Shape::line(pts, stroke));
                    }
                }
                if lod == 2 {
                    let p = e.tip;
                    let d = (e.tip - e.behind).normalized();
                    let d = if d.is_finite() { d } else { Vec2::X };
                    let (c, s) = ((std::f32::consts::PI / 9.0).cos(), (std::f32::consts::PI / 9.0).sin());
                    let a = Pos2::new(p.x - 16.0 * (d.x * c - d.y * s), p.y - 16.0 * (d.y * c + d.x * s));
                    let b = Pos2::new(p.x - 16.0 * (d.x * c + d.y * s), p.y - 16.0 * (d.y * c - d.x * s));
                    if e.inherit {
                        // Hollow triangle: canvas-coloured fill + stroke.
                        shapes.push(Shape::convex_polygon(vec![to_s(p), to_s(a), to_s(b)], BG, stroke));
                    } else {
                        shapes.push(Shape::line_segment([to_s(p), to_s(a)], stroke));
                        shapes.push(Shape::line_segment([to_s(p), to_s(b)], stroke));
                    }
                }
            }
        }

        let mut texts: Vec<(Pos2, Align2, String, f32, Color32)> = vec![];
        for c in &self.scene.classes {
            if !wvis(c.rect) {
                continue;
            }
            n.classes += 1;
            let r = wr(c.rect);
            let radius = CornerRadius::same((4.0 * z).min(255.0) as u8);
            if lod == 0 {
                // Overview block: the grade's stroke colour reads at a few px.
                shapes.push(Shape::rect_filled(r, radius, rgba(c.stroke, 200)));
                continue;
            }
            shapes.push(Shape::rect_filled(r, radius, rgb(c.fill)));
            let stroke = Stroke::new(2.5, rgb(c.stroke));
            if c.missing && lod == 2 {
                let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                shapes.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
            } else {
                shapes.push(Shape::rect_stroke(r, radius, stroke, StrokeKind::Inside));
            }
            // C/M dots.
            let (rx, ry, rw, rh) = (c.rect[0], c.rect[1], c.rect[2], c.rect[3]);
            for (i, b) in [&c.crap_band, &c.mutation_band].iter().enumerate() {
                let ctr = to_s(Pos2::new(rx + rw - 24.0 + i as f32 * 14.0, ry + 10.0));
                shapes.push(Shape::circle_filled(ctr, 6.0 * z, band(b)));
                if lod == 2 {
                    texts.push((ctr, Align2::CENTER_CENTER, if i == 0 { "C" } else { "M" }.into(), self.font_px(9.0, z), Color32::WHITE));
                }
            }
            if lod < 2 {
                continue;
            }
            if let Some(l) = c.level {
                texts.push((to_s(Pos2::new(rx + 4.0, ry + 2.0)), Align2::LEFT_TOP, format!("L{l}"), self.font_px(11.0, z), MUTED));
            }
            // Text lines at the layout's 18 px pitch; rules take 12 px.
            let mut y = ry + 16.0;
            for line in &c.lines {
                if line.kind == "rule" {
                    let yy = to_s(Pos2::new(0.0, y - 4.0)).y;
                    shapes.push(Shape::line_segment([Pos2::new(r.left(), yy), Pos2::new(r.right(), yy)], Stroke::new(1.0, rgb(c.stroke))));
                    y += 12.0;
                    continue;
                }
                let t = line.text.clone().unwrap_or_default();
                if line.kind == "name" {
                    texts.push((to_s(Pos2::new(rx + rw / 2.0, y)), Align2::CENTER_TOP, t, self.font_px(16.0, z), INK));
                } else {
                    texts.push((to_s(Pos2::new(rx + 12.0, y)), Align2::LEFT_TOP, t, self.font_px(15.0, z), MUTED));
                }
                y += 18.0;
            }
            if c.missing {
                texts.push((to_s(Pos2::new(rx + rw - 12.0, ry + rh - 14.0)), Align2::RIGHT_TOP, "无数据".into(), self.font_px(11.0, z), band("red")));
            }
        }
        painter.extend(shapes);
        // Text: egui caches laid-out galleys per (text, size); quantized sizes
        // bound the number of distinct rasterized glyph sizes.
        for (pos, label, size) in titles {
            painter.text(pos, Align2::LEFT_TOP, label, FontId::proportional(size), GOLD);
            n.texts += 1;
        }
        for (pos, al, t, size, color) in texts {
            if size >= 4.0 && canvas.contains(pos) {
                painter.text(pos, al, t, FontId::proportional(size), color);
                n.texts += 1;
            }
        }
        self.painted = n;
    }

    fn write_results(&self) {
        let mut ph = String::new();
        let mut sum = String::new();
        let f = |x: Option<f64>| x.map(|v| format!("{v:.2}")).unwrap_or("-".into());
        for r in &self.results {
            let s = r.stats;
            let g = |i: usize| s.map(|s| s[i]);
            let e = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or("nil".into());
            ph += &format!(
                "{{:phase \"{}\" :frames {} :wall-s {} :cpu-pct {} :fps {} :mean-ms {} :median-ms {} :p95-ms {} :p99-ms {} :max-ms {} :cpu-ms-p95 {} :zoom-min {} :zoom-max {}}} ",
                r.name, r.frames, r.wall, r.cpu_pct, e(g(0)), e(g(1)), e(g(2)), e(g(3)), e(g(4)), e(g(5)), e(r.cpu_ms_p95),
                e(r.zoom.map(|z| z.0 as f64)), e(r.zoom.map(|z| z.1 as f64))
            );
            sum += &format!(
                "phase={} frames={} fps={} mean_ms={} median_ms={} p95_ms={} p99_ms={} max_ms={} cpu_pct={:.2} app_cpu_ms_p95={} zoom={}..{}\n",
                r.name, r.frames, f(g(0)), f(g(1)), f(g(2)), f(g(3)), f(g(4)), f(g(5)), r.cpu_pct, f(r.cpu_ms_p95),
                f(r.zoom.map(|z| z.0 as f64)), f(r.zoom.map(|z| z.1 as f64))
            );
        }
        std::fs::write(self.out.join("protocol.edn"), format!("{{:protocol 1 :environment {} :phases [{}]}}\n", self.meta, ph)).unwrap();
        let text = format!("{}\n{}", self.meta, sum);
        std::fs::write(self.out.join("summary.txt"), &text).unwrap();
        print!("{text}");
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if std::env::var("BENCH_FEATHER").as_deref() == Ok("0") {
            ctx.tessellation_options_mut(|o| o.feathering = false);
        }
        let now = Instant::now();
        egui::Panel::left("side").exact_size(SIDEBAR_W).resizable(false).show(ui, |ui| {
            ui.label("PROTOTYPE #131 egui bench");
            ui.label(format!("phase: {}", self.phase.map(|p| PHASES[p.0]).unwrap_or("warmup")));
        });
        let canvas = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG))
            .show(ui, |ui| {
                let canvas = ui.max_rect();
                let painter = ui.painter_at(canvas);
                let t = Instant::now();
                self.paint(&painter, canvas);
                self.paint_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                canvas
            })
            .inner;

        if self.geo.is_none() {
            let s = &self.scene.size;
            let (w, h) = (s.w - s.min_x, s.h - s.min_y);
            let fit = 1f32.min(canvas.width() / w).min(canvas.height() / h);
            self.geo = Some(Geo { cx: s.min_x + w / 2.0, cy: s.min_y + h / 2.0, w, h, fit, zoom_hi: fit * 4.0 });
            let info = frame.wgpu_render_state().map(|rs| rs.adapter.get_info());
            let (name, backend, driver, driver_info) = info
                .map(|i| (i.name, format!("{:?}", i.backend), i.driver, i.driver_info))
                .unwrap_or_default();
            let ppp = ctx.pixels_per_point();
            self.meta = format!(
                "{{:stack \"egui/eframe 0.37 wgpu\" :renderer \"{}\" :backend \"{}\" :driver \"{}\" :driver-info \"{}\" :gallium-driver \"{}\" :wgpu-backend \"{}\" :vsync {} :dpi {} :view-w {} :view-h {} :scene-w {} :scene-h {} :fit-zoom {} :classes {} :edges {} :packages {} :text-quant {}}}",
                name, backend, driver, driver_info.replace('"', "'"),
                std::env::var("GALLIUM_DRIVER").unwrap_or_default(), std::env::var("WGPU_BACKEND").unwrap_or_default(),
                std::env::var("BENCH_VSYNC").as_deref() != Ok("0"), ppp, canvas.width(), canvas.height(), w, h, fit,
                self.scene.classes.len(), self.edges.len(), self.scene.packages.len(), self.quant
            );
            eprintln!("{}", self.meta);
            self.started = Some(now);
        }
        let geo = self.geo.unwrap();

        if self.shots {
            return self.shots_step(&ctx, geo);
        }

        let age = now.duration_since(self.started.unwrap()).as_secs_f64();
        if age < WARMUP_S {
            self.cam = (geo.cx, geo.cy, 1.0);
            self.last = Some(now);
            ctx.request_repaint();
            return;
        }
        if self.phase.is_none() {
            self.phase = Some((0, now, cpu_s()));
            self.samples.clear();
            self.cpu_samples.clear();
            self.zoom_range = None;
        }
        let (mut pi, t0, cpu0) = self.phase.unwrap();
        if let Ok(only) = std::env::var("BENCH_ONLY") {
            while PHASES[pi] != only && pi + 1 < PHASES.len() && self.results.is_empty() {
                pi += 1;
            }
            self.phase = Some((pi, t0, cpu0));
        }
        let name = PHASES[pi];
        if let Some(l) = self.last {
            self.samples.push(now.duration_since(l).as_secs_f64() * 1000.0);
        }
        if let Some(c) = frame.info().cpu_usage {
            self.cpu_samples.push(c as f64 * 1000.0);
        }
        self.last = Some(now);
        let t = now.duration_since(t0).as_secs_f64();
        if let Some((x, y, z)) = trajectory(name, t, &geo) {
            self.cam = (x, y, z);
            self.zoom_range = Some(self.zoom_range.map_or((z, z), |(a, b)| (a.min(z), b.max(z))));
        }
        let done = (t >= MIN_S && self.samples.len() >= MIN_FRAMES) || t >= MAX_S;
        // Idle: request nothing but a wake-up at MIN_S, so the phase ends.
        let idle_done = name == "idle" && t >= MIN_S;
        if done || idle_done {
            let cpu1 = cpu_s();
            let samples = std::mem::take(&mut self.samples);
            let mut cpu = std::mem::take(&mut self.cpu_samples);
            cpu.sort_by(|a, b| a.partial_cmp(b).unwrap());
            self.results.push(PhaseResult {
                name,
                frames: samples.len(),
                wall: t,
                cpu_pct: 100.0 * (cpu1 - cpu0) / t,
                stats: stats(samples),
                cpu_ms_p95: if cpu.is_empty() { None } else { Some(cpu[((cpu.len() as f64 * 0.95).ceil() as usize).max(1) - 1]) },
                zoom: self.zoom_range,
            });
            let mut pm = std::mem::take(&mut self.paint_ms);
            pm.sort_by(|a, b| a.partial_cmp(b).unwrap());
            eprintln!("phase {name} done; last painted (lod, classes, edges, texts) {:?}; paint-build p95 {:.2} ms", (self.painted.lod, self.painted.classes, self.painted.edges, self.painted.texts), pm.get((pm.len() * 95 / 100).min(pm.len().saturating_sub(1))).copied().unwrap_or(0.0));
            if pi + 1 >= PHASES.len() || std::env::var("BENCH_ONLY").is_ok() {
                self.write_results();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.phase = Some((pi + 1, now, cpu1));
            self.zoom_range = None;
            self.last = Some(now);
            ctx.request_repaint();
            return;
        }
        if name == "idle" {
            // Idle frames are not intervals of a continuous sequence.
            self.last = None;
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((MIN_S - t).max(0.0) + 0.01));
        } else {
            ctx.request_repaint();
        }
    }
}

impl App {
    /// Screenshot mode: fixed cameras, one PNG each, then exit.
    fn shots_step(&mut self, ctx: &egui::Context, geo: Geo) {
        let cams: [(&str, f32); 6] = [("zoom-1", 1.0), ("zoom-2", 2.0), ("zoom-0.5", 0.5), ("lod1-0.25", 0.25), ("lod0-4fit", geo.fit * 4.0), ("fit", geo.fit)];
        if let Some(img) = self.shot_pending.lock().unwrap().take() {
            let (name, _) = cams[self.shot_i];
            let px: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            image::save_buffer(self.out.join(format!("shot-{name}.png")), &px, img.size[0] as u32, img.size[1] as u32, image::ExtendedColorType::Rgba8).unwrap();
            self.shot_i += 1;
            self.shot_frames = 0;
            if self.shot_i >= cams.len() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        if self.shot_i >= cams.len() {
            return;
        }
        let (_, z) = cams[self.shot_i];
        // Offset off-centre a little so the shot shows a busy region.
        self.cam = (geo.cx - geo.w * 0.2, geo.cy - geo.h * 0.3, z);
        if z <= geo.fit * 4.0 {
            self.cam = (geo.cx, geo.cy, z);
        } else if std::env::var("BENCH_CJK").as_deref() == Ok("1") {
            // Centre on the first class with a CJK name.
            if let Some(c) = self.scene.classes.iter().find(|c| c.lines.iter().any(|l| l.kind == "name" && l.text.as_deref().is_some_and(|t| !t.is_ascii()))) {
                self.cam = (c.rect[0] + c.rect[2] / 2.0, c.rect[1] + c.rect[3] / 2.0, z);
            }
        }
        // Let the camera settle a few frames, then request exactly one shot.
        self.shot_frames += 1;
        if self.shot_frames == 10 {
            let slot = self.shot_pending.clone();
            ctx.request_screenshot(move |img| *slot.lock().unwrap() = Some(img));
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let scene: Scene = serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("scene json")).expect("parse");
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).unwrap();
    let vsync = std::env::var("BENCH_VSYNC").as_deref() != Ok("0");
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 920.0]).with_resizable(false).with_decorations(std::env::var("BENCH_DECOR").as_deref() == Ok("1")),
        ..Default::default()
    };
    options.wgpu_options.surface.present_mode = if vsync { eframe::wgpu::PresentMode::AutoVsync } else { eframe::wgpu::PresentMode::AutoNoVsync };
    eframe::run_native("egui-bench-proto", options, Box::new(move |cc| Ok(Box::new(App::new(cc, scene, out)))))
}
