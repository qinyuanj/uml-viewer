//! Protocol-1 measurement harness (bench/README.md, #143): the viewer runs a
//! fixed camera trajectory through warmup + idle/pan-detail/zoom-detail/
//! pan-fit/zoom-fit, records frame intervals and process CPU, and writes
//! `protocol.edn` + `summary.txt` for the acceptance evidence.
//!
//! The trajectory, statistics and verdict rules are ported from the #131
//! prototype (`bench/egui-proto`), so the numbers are comparable with the
//! selection runs. This module is window-free: the eframe seam in `app.rs`
//! supplies the timestamp of each painted frame and applies the camera.

use edn::Value;
use scene::geom::{get, num};
use std::path::PathBuf;
use std::time::Instant;

pub const PHASES: [&str; 5] = ["idle", "pan-detail", "zoom-detail", "pan-fit", "zoom-fit"];
pub const WARMUP_S: f64 = 3.0;
pub const MIN_S: f64 = 10.0;
pub const MIN_FRAMES: usize = 20;
pub const MAX_S: f64 = 180.0;
const TAU: f64 = std::f64::consts::TAU;

/// Protocol-1 thresholds (bench/README.md 判定 table).
pub const P95_2000_MS: f64 = 33.3;
pub const MEAN_FPS_500: f64 = 58.0;
pub const P95_500_MS: f64 = 18.3;
pub const IDLE_CPU_PCT: f64 = 2.0;

/// getconf CLK_TCK on Ubuntu x86_64; /proc/<pid>/stat fields 14/15 are ticks.
const CLK_TCK: f64 = 100.0;

/// Scene geometry the trajectory needs: centre, extents and the protocol fit
/// zoom (`min(1, canvas/w, canvas/h)`), which is deliberately NOT clamped to
/// the viewer's interactive 0.1 lower bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geo {
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    pub fit: f64,
    pub zoom_hi: f64,
}

impl Geo {
    pub fn from_scene(scene: &Value, canvas_w: f64, canvas_h: f64) -> Geo {
        // The compiled scene carries :size {:w :h :min-x :min-y} where :w/:h
        // are the maximum extents, exactly like `events::fit` reads them.
        let size = get(scene, "size");
        let x0 = size
            .and_then(|s| get(s, "min-x"))
            .and_then(num)
            .unwrap_or(0.0);
        let y0 = size
            .and_then(|s| get(s, "min-y"))
            .and_then(num)
            .unwrap_or(0.0);
        let w = (size
            .and_then(|s| get(s, "w"))
            .and_then(num)
            .unwrap_or(800.0)
            - x0)
            .max(1.0);
        let h = (size
            .and_then(|s| get(s, "h"))
            .and_then(num)
            .unwrap_or(600.0)
            - y0)
            .max(1.0);
        let fit = (canvas_w / w).min(canvas_h / h).min(1.0);
        Geo {
            cx: x0 + w / 2.0,
            cy: y0 + h / 2.0,
            w,
            h,
            fit,
            zoom_hi: fit * 4.0,
        }
    }
}

/// Camera for `phase` at `t` seconds in: world centre + zoom. `idle` (and
/// warmup, handled by the caller) keeps the camera where it is.
pub fn trajectory(phase: &str, t: f64, geo: &Geo) -> Option<(f64, f64, f64)> {
    let (cx, cy) = (geo.cx, geo.cy);
    match phase {
        "pan-detail" => Some((
            cx + 1200.0 * (TAU * t / 8.0).cos(),
            cy + 1200.0 * (TAU * t / 8.0).sin(),
            1.0,
        )),
        "zoom-detail" => {
            let u = 0.5 - 0.5 * (TAU * t / 4.0).cos();
            Some((cx, cy, (0.5f64.ln() + u * (2f64.ln() - 0.5f64.ln())).exp()))
        }
        "pan-fit" => Some((
            cx + 0.25 * geo.w * (TAU * t / 8.0).sin(),
            cy + 0.25 * geo.h * (2.0 * TAU * t / 8.0).sin(),
            geo.fit,
        )),
        "zoom-fit" => {
            let u = 0.5 - 0.5 * (TAU * t / 4.0).cos();
            Some((cx, cy, geo.fit * (u * (geo.zoom_hi / geo.fit).ln()).exp()))
        }
        _ => None,
    }
}

/// fps, mean, median, p95, p99, max of frame intervals in milliseconds.
pub fn stats(samples: &[f64]) -> Option<[f64; 6]> {
    if samples.is_empty() {
        return None;
    }
    let mut v = samples.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let total: f64 = v.iter().sum();
    let q = |p: f64| v[((v.len() as f64 * p).ceil() as usize).max(1) - 1];
    Some([
        1000.0 * v.len() as f64 / total,
        total / v.len() as f64,
        q(0.5),
        q(0.95),
        q(0.99),
        v[v.len() - 1],
    ])
}

/// (user+sys) seconds of this process from /proc/self/stat. The viewer
/// measures its own process tree root; it spawns no child processes during a
/// bench run, and threads are already accounted in the process counters.
pub fn parse_proc_stat(text: &str) -> Option<f64> {
    let rest = text.rsplit_once(") ")?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After the comm field, split index 0 is field 3 (state): 11 → utime
    // (field 14), 12 → stime (field 15).
    let utime: f64 = fields.get(11)?.parse().ok()?;
    let stime: f64 = fields.get(12)?.parse().ok()?;
    Some((utime + stime) / CLK_TCK)
}

pub fn self_cpu_seconds() -> Option<f64> {
    parse_proc_stat(&std::fs::read_to_string("/proc/self/stat").ok()?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseResult {
    pub name: &'static str,
    pub frames: usize,
    pub wall: f64,
    pub cpu_pct: f64,
    pub stats: Option<[f64; 6]>,
    pub cpu_ms_p95: Option<f64>,
    pub zoom: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Criterion {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

/// The protocol-1 verdict table (bench/README.md): the 2000-class scene
/// needs p95 ≤ 33.3 ms on every interactive phase; the 500-class scene needs
/// mean fps ≥ 58 and p95 ≤ 18.3 ms; idle CPU ≤ 2 % on both. A renderer
/// string without `D3D12` voids the run: no performance claim survives it.
pub fn evaluate(renderer: &str, classes: usize, results: &[PhaseResult]) -> Vec<Criterion> {
    let mut criteria = vec![Criterion {
        name: "renderer contains D3D12 (otherwise the run is void)".to_string(),
        pass: renderer.contains("D3D12"),
        detail: renderer.to_string(),
    }];
    if !criteria[0].pass {
        return criteria;
    }
    let big = classes >= 1000;
    for result in results {
        if result.name == "idle" {
            criteria.push(Criterion {
                name: "idle CPU <= 2%".to_string(),
                pass: result.cpu_pct <= IDLE_CPU_PCT,
                detail: format!("cpu_pct={:.2}", result.cpu_pct),
            });
            continue;
        }
        let fps = result.stats.map(|s| s[0]);
        let p95 = result.stats.map(|s| s[3]);
        let (pass, detail) = match (big, fps, p95) {
            (true, _, Some(p95)) => (
                p95 <= P95_2000_MS,
                format!("p95_ms={p95:.2} <= {P95_2000_MS}"),
            ),
            (false, Some(fps), Some(p95)) => (
                fps >= MEAN_FPS_500 && p95 <= P95_500_MS,
                format!("fps={fps:.2} >= {MEAN_FPS_500} && p95_ms={p95:.2} <= {P95_500_MS}"),
            ),
            _ => (false, "no frames recorded".to_string()),
        };
        criteria.push(Criterion {
            name: format!(
                "{} {}",
                result.name,
                if big {
                    "p95 <= 33.3ms"
                } else {
                    "fps >= 58 && p95 <= 18.3ms"
                }
            ),
            pass,
            detail,
        });
    }
    criteria
}

/// What the eframe seam does with one painted frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// World centre + zoom to apply; `None` keeps the current camera (idle).
    pub camera: Option<(f64, f64, f64)>,
    /// Continuous phases repaint every frame; idle waits on the poll cadence.
    pub continuous: bool,
    /// All phases done: results are written, the caller closes the window.
    pub finished: bool,
}

#[derive(Debug)]
pub struct Bench {
    out: PathBuf,
    only: Option<String>,
    renderer: String,
    classes: usize,
    meta: String,
    started: Option<Instant>,
    last: Option<Instant>,
    phase: Option<(usize, Instant, f64)>,
    samples: Vec<f64>,
    cpu_samples: Vec<f64>,
    zoom_range: Option<(f64, f64)>,
    results: Vec<PhaseResult>,
    verdict: Option<bool>,
}

impl Bench {
    pub fn new(
        out: PathBuf,
        only: Option<String>,
        renderer: String,
        classes: usize,
    ) -> Result<Bench, String> {
        if let Some(name) = &only
            && !PHASES.contains(&name.as_str())
        {
            return Err(format!(
                "UML_VIEWER_BENCH_ONLY must be one of {PHASES:?}, got {name:?}"
            ));
        }
        std::fs::create_dir_all(&out).map_err(|e| format!("bench output {out:?}: {e}"))?;
        Ok(Bench {
            out,
            only,
            renderer,
            classes,
            meta: String::new(),
            started: None,
            last: None,
            phase: None,
            samples: Vec::new(),
            cpu_samples: Vec::new(),
            zoom_range: None,
            results: Vec::new(),
            verdict: None,
        })
    }

    /// The environment map is known only once the window exists (canvas size,
    /// DPI); the app sets it on the first frame.
    pub fn set_meta(&mut self, meta: String) {
        self.meta = meta;
    }

    /// Overall verdict once `frame` reported `finished`; None before.
    pub fn passed(&self) -> Option<bool> {
        self.verdict
    }

    pub fn frame(&mut self, geo: &Geo, now: Instant, cpu_ms: Option<f64>) -> Step {
        let started = *self.started.get_or_insert(now);
        let age = now.duration_since(started).as_secs_f64();
        if age < WARMUP_S {
            self.last = Some(now);
            return Step {
                camera: Some((geo.cx, geo.cy, 1.0)),
                continuous: true,
                finished: false,
            };
        }
        if self.phase.is_none() {
            let index = self
                .only
                .as_deref()
                .map(|name| {
                    PHASES
                        .iter()
                        .position(|p| *p == name)
                        .expect("validated in new")
                })
                .unwrap_or(0);
            self.phase = Some((index, now, self_cpu_seconds().unwrap_or(0.0)));
            self.samples.clear();
            self.cpu_samples.clear();
            self.zoom_range = None;
        }
        let (pi, t0, cpu0) = self.phase.expect("initialized above");
        let name = PHASES[pi];
        if let Some(last) = self.last {
            self.samples
                .push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        if let Some(cpu) = cpu_ms {
            self.cpu_samples.push(cpu);
        }
        self.last = Some(now);
        let t = now.duration_since(t0).as_secs_f64();
        let camera = trajectory(name, t, geo);
        if let Some((_, _, zoom)) = camera {
            self.zoom_range = Some(
                self.zoom_range
                    .map_or((zoom, zoom), |(lo, hi)| (lo.min(zoom), hi.max(zoom))),
            );
        }
        let done = (t >= MIN_S && self.samples.len() >= MIN_FRAMES) || t >= MAX_S;
        // Idle: request nothing but the poll wake-ups, so the phase ends.
        let idle_done = name == "idle" && t >= MIN_S;
        if done || idle_done {
            let cpu1 = self_cpu_seconds().unwrap_or(cpu0);
            let samples = std::mem::take(&mut self.samples);
            let mut cpu = std::mem::take(&mut self.cpu_samples);
            cpu.sort_by(|a, b| a.partial_cmp(b).unwrap());
            self.results.push(PhaseResult {
                name,
                frames: samples.len(),
                wall: t,
                cpu_pct: 100.0 * (cpu1 - cpu0) / t,
                stats: stats(&samples),
                cpu_ms_p95: if cpu.is_empty() {
                    None
                } else {
                    Some(cpu[((cpu.len() as f64 * 0.95).ceil() as usize).max(1) - 1])
                },
                zoom: self.zoom_range,
            });
            if pi + 1 >= PHASES.len() || self.only.is_some() {
                self.write_results();
                return Step {
                    camera,
                    continuous: false,
                    finished: true,
                };
            }
            self.phase = Some((pi + 1, now, cpu1));
            self.zoom_range = None;
            self.last = Some(now);
            return Step {
                camera,
                continuous: true,
                finished: false,
            };
        }
        if name == "idle" {
            // Idle frames are not intervals of a continuous sequence.
            self.last = None;
            Step {
                camera,
                continuous: false,
                finished: false,
            }
        } else {
            Step {
                camera,
                continuous: true,
                finished: false,
            }
        }
    }

    fn write_results(&mut self) {
        let criteria = evaluate(&self.renderer, self.classes, &self.results);
        let verdict = criteria.iter().all(|c| c.pass);
        self.verdict = Some(verdict);
        let mut phases = String::new();
        let mut summary = String::new();
        let f = |x: Option<f64>| x.map(|v| format!("{v:.2}")).unwrap_or("-".into());
        let e = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or("nil".into());
        for r in &self.results {
            let s = r.stats;
            let g = |i: usize| s.map(|s| s[i]);
            phases += &format!(
                "{{:phase \"{}\" :frames {} :wall-s {} :cpu-pct {} :fps {} :mean-ms {} :median-ms {} :p95-ms {} :p99-ms {} :max-ms {} :cpu-ms-p95 {} :zoom-min {} :zoom-max {}}} ",
                r.name,
                r.frames,
                r.wall,
                r.cpu_pct,
                e(g(0)),
                e(g(1)),
                e(g(2)),
                e(g(3)),
                e(g(4)),
                e(g(5)),
                e(r.cpu_ms_p95),
                e(r.zoom.map(|z| z.0)),
                e(r.zoom.map(|z| z.1))
            );
            summary += &format!(
                "phase={} frames={} fps={} mean_ms={} median_ms={} p95_ms={} p99_ms={} max_ms={} cpu_pct={:.2} app_cpu_ms_p95={} zoom={}..{}\n",
                r.name,
                r.frames,
                f(g(0)),
                f(g(1)),
                f(g(2)),
                f(g(3)),
                f(g(4)),
                f(g(5)),
                r.cpu_pct,
                f(r.cpu_ms_p95),
                f(r.zoom.map(|z| z.0)),
                f(r.zoom.map(|z| z.1))
            );
        }
        let mut criteria_edn = String::new();
        for c in &criteria {
            criteria_edn += &format!(
                "{{:name \"{}\" :pass {} :detail \"{}\"}} ",
                c.name,
                c.pass,
                c.detail.replace('"', "'")
            );
            summary += &format!(
                "criterion name={:?} pass={} detail={}\n",
                c.name, c.pass, c.detail
            );
        }
        summary += &format!("verdict={}\n", if verdict { "PASS" } else { "FAIL" });
        let protocol = format!(
            "{{:protocol 1 :environment {} :phases [{}] :criteria [{}] :verdict \"{}\"}}\n",
            self.meta,
            phases,
            criteria_edn,
            if verdict { "PASS" } else { "FAIL" }
        );
        if let Err(error) = std::fs::write(self.out.join("protocol.edn"), protocol).and_then(|()| {
            std::fs::write(
                self.out.join("summary.txt"),
                format!("{}\n{summary}", self.meta),
            )
        }) {
            eprintln!("uml viewer: could not write bench results: {error}");
        }
    }
}

/// The environment EDN map recorded with every run (ported from the
/// prototype): renderer identity is the D3D12-versus-llvmpipe evidence.
#[allow(clippy::too_many_arguments)]
pub fn environment_meta(
    renderer: &str,
    backend: &str,
    driver: &str,
    driver_info: &str,
    dpi: f64,
    canvas: (f64, f64),
    geo: &Geo,
    counts: (usize, usize, usize),
) -> String {
    format!(
        "{{:stack \"uml-viewer egui/eframe 0.37 wgpu (#139/#143)\" :renderer \"{}\" :backend \"{}\" :driver \"{}\" :driver-info \"{}\" :gallium-driver \"{}\" :wgpu-backend \"{}\" :vsync \"eframe default (AutoVsync)\" :dpi {} :view-w {} :view-h {} :scene-w {} :scene-h {} :fit-zoom {} :classes {} :edges {} :packages {}}}",
        renderer,
        backend,
        driver,
        driver_info.replace('"', "'"),
        std::env::var("GALLIUM_DRIVER").unwrap_or_default(),
        std::env::var("WGPU_BACKEND").unwrap_or_default(),
        dpi,
        canvas.0,
        canvas.1,
        geo.w,
        geo.h,
        geo.fit,
        counts.0,
        counts.1,
        counts.2
    )
}

/// Scene element counts for the meta map: (classes, edges, packages).
pub fn scene_counts(scene: &Value) -> (usize, usize, usize) {
    let len = |key: &str| match get(scene, key) {
        Some(Value::Vector(v)) => v.len(),
        _ => 0,
    };
    (len("classes"), len("edges"), len("packages"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo() -> Geo {
        // The 2000-class scene shape: fit lands far below the viewer's 0.1
        // interactive zoom floor, which the protocol explicitly ignores.
        Geo {
            cx: 7300.0,
            cy: 14500.0,
            w: 14606.0,
            h: 29035.0,
            fit: 0.0317,
            zoom_hi: 0.1268,
        }
    }

    #[test]
    fn geo_reads_scene_size_and_never_clamps_fit() {
        let scene = edn::parse("{:size {:w 14646 :h 29075 :min-x -40 :min-y -40}}").unwrap();
        let g = Geo::from_scene(&scene, 1500.0, 920.0);
        assert!((g.w - 14686.0).abs() < 1e-9);
        assert!((g.h - 29115.0).abs() < 1e-9);
        assert!((g.cx - (-40.0 + 14686.0 / 2.0)).abs() < 1e-9);
        let expected = (1500.0_f64 / 14686.0).min(920.0 / 29115.0);
        assert!((g.fit - expected).abs() < 1e-12);
        assert!(
            g.fit < 0.1,
            "the protocol fit ignores the viewer's 0.1 floor"
        );
        assert!((g.zoom_hi - g.fit * 4.0).abs() < 1e-12);
        // A small scene fits at 100%, never magnified.
        let small = edn::parse("{:size {:w 800 :h 600 :min-x 0 :min-y 0}}").unwrap();
        assert_eq!(1.0, Geo::from_scene(&small, 1500.0, 920.0).fit);
    }

    #[test]
    fn trajectory_matches_the_protocol_camera_paths() {
        let g = geo();
        // pan-detail: circle of radius 1200 around the centre, period 8 s.
        assert_eq!(
            Some((g.cx + 1200.0, g.cy, 1.0)),
            trajectory("pan-detail", 0.0, &g)
        );
        let (x, y, z) = trajectory("pan-detail", 2.0, &g).unwrap();
        assert!((x - g.cx).abs() < 1e-6 && (y - (g.cy + 1200.0)).abs() < 1e-6 && z == 1.0);
        // zoom-detail: geometric interpolation 0.5 ↔ 2 around the centre.
        assert_eq!(Some((g.cx, g.cy, 0.5)), trajectory("zoom-detail", 0.0, &g));
        let (_, _, z) = trajectory("zoom-detail", 2.0, &g).unwrap();
        assert!((z - 2.0).abs() < 1e-9);
        let (_, _, z) = trajectory("zoom-detail", 1.0, &g).unwrap();
        assert!(
            (z - 1.0).abs() < 1e-9,
            "geometric midpoint of 0.5 and 2 is 1"
        );
        // pan-fit: Lissajous over 25% of the scene at the fit zoom.
        assert_eq!(Some((g.cx, g.cy, g.fit)), trajectory("pan-fit", 0.0, &g));
        let (x, y, z) = trajectory("pan-fit", 2.0, &g).unwrap();
        assert!((x - (g.cx + 0.25 * g.w)).abs() < 1e-6 && (y - g.cy).abs() < 1e-6 && z == g.fit);
        // zoom-fit: fit ↔ 4×fit geometric.
        assert_eq!(Some((g.cx, g.cy, g.fit)), trajectory("zoom-fit", 0.0, &g));
        let (_, _, z) = trajectory("zoom-fit", 2.0, &g).unwrap();
        assert!((z - g.zoom_hi).abs() < 1e-9);
        // idle keeps the camera: no trajectory entry.
        assert_eq!(None, trajectory("idle", 5.0, &g));
    }

    #[test]
    fn stats_reports_fps_and_percentiles() {
        let s = stats(&[10.0, 20.0, 30.0, 40.0]).unwrap();
        assert_eq!(40.0, s[0]); // 4 frames in 100 ms
        assert_eq!(25.0, s[1]);
        assert_eq!(20.0, s[2]); // ceil(4*0.5) → 2nd sample
        assert_eq!(40.0, s[3]); // ceil(4*0.95) → 4th sample
        assert_eq!(40.0, s[4]);
        assert_eq!(40.0, s[5]);
        assert_eq!(None, stats(&[]));
    }

    #[test]
    fn proc_stat_parses_user_and_system_ticks() {
        // comm may contain spaces and parentheses; fields after ") " start at
        // field 3, so utime (field 14) is split index 11, stime index 12.
        let text = "1234 (name with ) paren) S 1 2 3 4 5 6 7 8 9 10 200 300 99 88";
        assert_eq!(Some(5.0), parse_proc_stat(text));
        assert_eq!(None, parse_proc_stat("garbage"));
        // The real file parses on this host.
        assert!(self_cpu_seconds().unwrap() >= 0.0);
    }

    #[test]
    fn evaluate_applies_the_protocol_thresholds() {
        let phase = |name: &'static str, fps, p95, cpu| PhaseResult {
            name,
            frames: 600,
            wall: 10.0,
            cpu_pct: cpu,
            stats: Some([fps, 16.0, 16.0, p95, 20.0, 25.0]),
            cpu_ms_p95: None,
            zoom: None,
        };
        let interactive = vec![
            phase("idle", 0.0, 0.0, 1.0),
            phase("pan-detail", 60.0, 16.0, 40.0),
            phase("zoom-detail", 60.0, 16.0, 40.0),
            phase("pan-fit", 60.0, 16.0, 40.0),
            phase("zoom-fit", 60.0, 16.0, 40.0),
        ];
        // 2000-class rule: only p95 matters.
        let criteria = evaluate("D3D12 (GPU)", 2000, &interactive);
        assert!(criteria.iter().all(|c| c.pass), "{criteria:?}");
        let mut slow = interactive.clone();
        slow[1].stats = Some([20.0, 50.0, 50.0, 40.0, 60.0, 80.0]);
        let criteria = evaluate("D3D12 (GPU)", 2000, &slow);
        assert!(!criteria.iter().all(|c| c.pass));
        assert!(
            criteria
                .iter()
                .any(|c| !c.pass && c.name.contains("pan-detail"))
        );
        // 500-class rule: mean fps and a tighter p95.
        let criteria = evaluate("D3D12 (GPU)", 500, &interactive);
        assert!(criteria.iter().all(|c| c.pass), "{criteria:?}");
        let mut sluggish = interactive.clone();
        sluggish[2].stats = Some([57.5, 17.0, 17.0, 18.0, 20.0, 25.0]);
        let criteria = evaluate("D3D12 (GPU)", 500, &sluggish);
        assert!(
            criteria
                .iter()
                .any(|c| !c.pass && c.name.contains("zoom-detail"))
        );
        // idle CPU.
        let mut busy = interactive.clone();
        busy[0].cpu_pct = 5.0;
        let criteria = evaluate("D3D12 (GPU)", 500, &busy);
        assert!(criteria.iter().any(|c| !c.pass && c.name.contains("idle")));
        // A non-D3D12 renderer voids the run: only the renderer criterion.
        let criteria = evaluate("llvmpipe", 500, &interactive);
        assert_eq!(1, criteria.len());
        assert!(!criteria[0].pass);
    }

    #[test]
    fn bench_runs_the_phases_and_writes_results() {
        let out = std::env::temp_dir().join(format!("uml-bench-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let mut bench = Bench::new(out.clone(), None, "D3D12 (test)".into(), 500).unwrap();
        bench.set_meta("{:test true}".into());
        let g = geo();
        let t0 = Instant::now();
        let mut now = t0;
        let mut finished = false;
        // 100 frames per simulated second: every phase ends near MIN_S.
        for _ in 0..100 * 120 {
            now += std::time::Duration::from_millis(10);
            let step = bench.frame(&g, now, Some(2.0));
            if step.finished {
                finished = true;
                break;
            }
        }
        assert!(
            finished,
            "the run must terminate: {:?}",
            bench.results.len()
        );
        assert_eq!(Some(true), bench.passed());
        assert_eq!(PHASES.len(), bench.results.len());
        // Warmup is not a phase; every phase respected the minimum duration.
        assert!(bench.results.iter().all(|r| r.wall >= MIN_S));
        let protocol = std::fs::read_to_string(out.join("protocol.edn")).unwrap();
        assert!(protocol.contains(":protocol 1"));
        assert!(protocol.contains("\"pan-detail\""));
        assert!(protocol.contains(":verdict \"PASS\""));
        let summary = std::fs::read_to_string(out.join("summary.txt")).unwrap();
        assert!(summary.contains("verdict=PASS"));
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn bench_only_runs_a_single_named_phase() {
        let out = std::env::temp_dir().join(format!("uml-bench-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let mut bench =
            Bench::new(out.clone(), Some("zoom-fit".into()), "D3D12".into(), 2000).unwrap();
        let g = geo();
        let t0 = Instant::now();
        let mut now = t0;
        let mut finished = false;
        for _ in 0..100 * 60 {
            now += std::time::Duration::from_millis(10);
            if bench.frame(&g, now, None).finished {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert_eq!(
            vec!["zoom-fit"],
            bench.results.iter().map(|r| r.name).collect::<Vec<_>>()
        );
        // zoom-fit really moved through the fit..4×fit range.
        let (lo, hi) = bench.results[0].zoom.unwrap();
        assert!(
            lo <= g.fit * 1.0001 && hi >= g.zoom_hi * 0.9999,
            "{lo}..{hi}"
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn bench_refuses_an_unknown_only_phase() {
        let out = std::env::temp_dir().join(format!("uml-bench-bad-{}", std::process::id()));
        let error = Bench::new(out, Some("fly".into()), "D3D12".into(), 500).unwrap_err();
        assert!(error.contains("fly"));
    }
}
