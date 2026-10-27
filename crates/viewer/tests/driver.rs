//! Declarative driver scripts: parsing, sequencing, file/evidence assertions
//! and the quit contract, exercised headlessly with a fake host.

use std::path::{Path, PathBuf};
use viewer::driver::{Input, Progress, Runner, Script, ViewRecord};

#[derive(Default)]
struct FakeHost {
    inputs: Vec<Input>,
    shots: Vec<String>,
    views: Vec<(Option<f64>, Option<[f64; 2]>)>,
    quit: Option<u8>,
    italic: bool,
    record: ViewRecord,
    fitted: bool,
    blank: bool,
    class_points: Vec<(String, bool)>,
    declutter_clicks: u32,
    fonts_ok: bool,
    fits_ok: bool,
    display: Option<edn::Value>,
    quality: Option<String>,
    resized: Vec<(f64, f64)>,
}

impl viewer::driver::Host for FakeHost {
    fn inject(&mut self, input: Input) {
        self.inputs.push(input);
    }
    fn assert_italic_glyphs(&mut self) -> Result<(), String> {
        if self.italic {
            Ok(())
        } else {
            Err("italic names must slant real glyphs".into())
        }
    }
    fn screenshot(&mut self, evidence: &str) -> bool {
        self.shots.push(evidence.into());
        true
    }
    fn set_view(&mut self, zoom: Option<f64>, center: Option<[f64; 2]>) {
        self.views.push((zoom, center));
    }
    fn quit(&mut self, code: u8) {
        self.quit = Some(code);
    }
    fn view_record(&mut self, anchor: Option<[f64; 2]>) -> ViewRecord {
        let mut record = self.record.clone();
        record.anchor = anchor.map(|[x, y]| (x + 1000.0, y + 1000.0));
        record
    }
    fn is_fitted(&self) -> bool {
        self.fitted
    }
    fn is_blank(&self, _x: f64, _y: f64) -> bool {
        self.blank
    }
    fn click_class(&mut self, id: &str, double: bool, _: bool) -> Result<(), String> {
        if id == "missing" {
            return Err(format!("no class {id} on the scene"));
        }
        self.class_points.push((id.to_string(), double));
        Ok(())
    }
    fn click_declutter(&mut self) {
        self.declutter_clicks += 1;
    }
    fn assert_fonts(&mut self) -> Result<(), String> {
        if self.fonts_ok {
            Ok(())
        } else {
            Err("font source system expected sarasa".into())
        }
    }
    fn assert_text_fits(&mut self) -> Result<(), String> {
        if self.fits_ok {
            Ok(())
        } else {
            Err("text exceeds horizontal slot: 解析器".into())
        }
    }
    fn display_record(&self) -> edn::Value {
        self.display.clone().unwrap_or(edn::Value::Nil)
    }
    fn quality_line(&self, id: &str) -> Option<String> {
        self.quality.clone().map(|line| format!("{id}: {line}"))
    }
    fn resize_window(&mut self, w: f64, h: f64) {
        self.resized.push((w, h));
    }
}

fn temp(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("viewer-driver-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let evidence = base.join("evidence");
    let project = base.join("project");
    std::fs::create_dir_all(&evidence).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    (evidence, project)
}

fn run_all(
    script: &str,
    evidence: &Path,
    project: &Path,
    host: &mut FakeHost,
    frames: u64,
) -> Result<Progress, String> {
    let mut runner = Runner::new(Script::parse(script).unwrap());
    let mut last = Progress::Running;
    for frame in 1..=frames {
        last = runner.run(frame, Some(evidence), project, host)?;
        if last == Progress::Done {
            break;
        }
    }
    Ok(last)
}

#[test]
fn parses_every_op_the_ticket_lists() {
    let script = Script::parse(
        r#"{:version 1 :steps [
            {:op :wait-frames :n 3}
            {:op :click :at [10 20]}
            {:op :double-click :at [30 40]}
            {:op :drag :from [1 2] :to [3 4]}
            {:op :wheel :dx 0 :dy -2}
            {:op :key :key "r" :ctrl true}
            {:op :text :text "中文"}
            {:op :view :zoom 0.2 :center [5 6]}
            {:op :screenshot :evidence "shot.png"}
            {:op :quit :code 0}]}"#,
    )
    .unwrap();
    assert_eq!(10, script.steps.len());
    assert!(Script::parse("{:steps []}").unwrap().steps.is_empty());
    assert!(Script::parse("{:nope 1}").is_err());
    assert!(Script::parse("{:steps [{:op :bogus}]}").is_err());
    assert!(Script::parse("{:steps [{:op :click}]}").is_err());
}

#[test]
fn a_script_may_pin_the_base_rank_order() {
    let order = |src: &str| Script::parse(src).map(|s| s.rank_order);
    assert_eq!(Ok(scene::RankOrder::Span), order("{:steps []}"));
    assert_eq!(
        Ok(scene::RankOrder::Base),
        order("{:rank-order :base :steps []}")
    );
    assert_eq!(
        Ok(scene::RankOrder::Span),
        order("{:rank-order :span :steps []}")
    );
    assert!(order("{:rank-order :fanin :steps []}").is_err());
}

#[test]
fn replays_inputs_in_order_and_quits_with_the_requested_code() {
    let (evidence, project) = temp("inputs");
    let mut host = FakeHost::default();
    let progress = run_all(
        r#"{:steps [{:op :wait-frames :n 2}
                    {:op :click :at [10 20]}
                    {:op :double-click :at [30 40]}
                    {:op :wheel :dy -1}
                    {:op :key :key "f3"}
                    {:op :text :text "x"}
                    {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        10,
    )
    .unwrap();
    assert_eq!(Progress::Done, progress);
    assert_eq!(
        vec![
            Input::Click { x: 10.0, y: 20.0 },
            Input::DoubleClick { x: 30.0, y: 40.0 },
            Input::Wheel {
                dx: 0.0,
                dy: -1.0,
                ctrl: false,
                shift: false,
            },
            Input::Key {
                key: "f3".into(),
                ctrl: false,
                shift: false
            },
            Input::Text("x".into()),
        ],
        host.inputs
    );
    assert_eq!(Some(0), host.quit);
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn wait_frames_blocks_until_the_budget_elapses() {
    let (evidence, project) = temp("wait");
    let mut host = FakeHost::default();
    let mut runner = Runner::new(
        Script::parse("{:steps [{:op :wait-frames :n 3} {:op :quit :code 0}]}").unwrap(),
    );
    // Frame numbers start at 1 and the runner records its first frame.
    assert_eq!(
        Progress::Running,
        runner.run(1, Some(&evidence), &project, &mut host).unwrap()
    );
    assert_eq!(
        Progress::Running,
        runner.run(3, Some(&evidence), &project, &mut host).unwrap()
    );
    assert_eq!(
        Progress::Done,
        runner.run(4, Some(&evidence), &project, &mut host).unwrap()
    );
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn wait_file_blocks_until_the_evidence_appears() {
    let (evidence, project) = temp("waitfile");
    let mut host = FakeHost::default();
    let mut runner = Runner::new(
        Script::parse("{:steps [{:op :wait-file :evidence \"scene-2.edn\"} {:op :quit :code 0}]}")
            .unwrap(),
    );
    assert_eq!(
        Progress::Running,
        runner.run(1, Some(&evidence), &project, &mut host).unwrap()
    );
    std::fs::write(evidence.join("scene-2.edn"), "{}").unwrap();
    assert_eq!(
        Progress::Done,
        runner.run(2, Some(&evidence), &project, &mut host).unwrap()
    );
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn assertions_compare_evidence_contents_and_report_the_step_message() {
    let (evidence, project) = temp("assert");
    std::fs::write(
        evidence.join("scene-1.edn"),
        "{:mu 1 :strokes [] :tip [1 2]}",
    )
    .unwrap();
    std::fs::write(
        evidence.join("scene-2.edn"),
        "{:mu 9 :strokes [] :tip [1 2]}",
    )
    .unwrap();
    let mut host = FakeHost::default();
    let ok = run_all(
        r#"{:steps [
            {:op :assert :evidence "scene-1.edn" :contains-any [":mu 1 " ":mu 1}"]}
            {:op :assert :evidence "scene-2.edn" :differs-from "scene-1.edn"}
            {:op :assert :evidence "scene-2.edn" :contains [":strokes" ":tip"]}
            {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Ok(Progress::Done), ok);
    let mut host = FakeHost::default();
    let failed = run_all(
        r#"{:steps [{:op :assert :evidence "scene-1.edn" :contains [":mu 9"]
                     :message "first scene lacks the crap-9 summary"}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Err("first scene lacks the crap-9 summary".into()), failed);
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn edit_and_mtime_steps_keep_the_metrics_contract() {
    let (evidence, project) = temp("edit");
    std::fs::create_dir_all(project.join(".metrics")).unwrap();
    std::fs::write(project.join("diagram.edn"), "{}").unwrap();
    std::fs::write(project.join(".metrics/crap.edn"), "{:crap 1}").unwrap();
    let mut host = FakeHost::default();
    let progress = run_all(
        r#"{:steps [
            {:op :record-mtime :project "diagram.edn" :as :ir}
            {:op :edit :project ".metrics/crap.edn" :from ":crap 1" :to ":crap 9" :same-length true}
            {:op :assert-mtime :project "diagram.edn" :same-as :ir :message "the IR file must not be rewritten"}
            {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Ok(Progress::Done), progress);
    assert_eq!(
        "{:crap 9}",
        std::fs::read_to_string(project.join(".metrics/crap.edn")).unwrap()
    );
    // A length-changing edit is refused.
    let mut host = FakeHost::default();
    let failed = run_all(
        r#"{:steps [{:op :edit :project ".metrics/crap.edn" :from ":crap 9" :to ":crap 10" :same-length true}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert!(failed.unwrap_err().contains("must keep the byte length"));
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn write_screenshot_and_view_steps_go_through_the_host() {
    let (evidence, project) = temp("write");
    let mut host = FakeHost::default();
    let progress = run_all(
        r#"{:steps [
            {:op :view :zoom 0.1 :center [100 200]}
            {:op :screenshot :evidence "lod-blocks.png"}
            {:op :write :evidence "lod-pass.txt" :text "PASS\n"}
            {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Ok(Progress::Done), progress);
    assert_eq!(vec![(Some(0.1), Some([100.0, 200.0]))], host.views);
    assert_eq!(vec!["lod-blocks.png".to_string()], host.shots);
    assert_eq!(
        "PASS\n",
        std::fs::read_to_string(evidence.join("lod-pass.txt")).unwrap()
    );
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn the_italic_glyph_probe_can_fail_the_driver() {
    let (evidence, project) = temp("italic");
    let mut host = FakeHost::default();
    let failed = run_all(
        r#"{:steps [{:op :assert-italic-glyphs}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(
        Err("italic names must slant real glyphs: italic names must slant real glyphs".into()),
        failed
    );
    host.italic = true;
    let ok = run_all(
        r#"{:steps [{:op :assert-italic-glyphs}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Ok(Progress::Done), ok);
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

fn record(zoom: f64, cam_x: f64, cam_y: f64) -> ViewRecord {
    ViewRecord {
        zoom,
        cam_x,
        cam_y,
        ..ViewRecord::default()
    }
}

#[test]
fn parses_the_navigation_and_display_ops() {
    let script = Script::parse(
        r#"{:version 1 :steps [
            {:op :mouse-move :at [300 260]}
            {:op :wheel :dy 1 :ctrl true}
            {:op :paste :text "中文解析"}
            {:op :dump-view :evidence "navigation-root.edn"}
            {:op :record-view :as :before :anchor [300 260]}
            {:op :assert-view :focus [:source] :fitted true :selected "source"
             :zoom-times [:before 1.1] :cam-shift [[100 50] :before]
             :cam-y-not-below :before :anchor [300 260] :anchor-fixed :before
             :zoom-unchanged :before :cam-unchanged :before :focus-unchanged :before
             :selected-unchanged :before :declutter "arrows" :dragging false
             :message "navigation check failed"}
            {:op :assert-blank :at [900 860]}
            {:op :click-class :id :source}
            {:op :double-click-class :id :source.lua}
            {:op :click-declutter}
            {:op :assert-fonts :message "fonts"}
            {:op :assert-text-fits :message "fits"}
            {:op :dump-display :evidence "display.edn"}
            {:op :dump-quality :evidence "quality-diagnosis.txt" :id :uml.application.detail}
            {:op :resize-window :size [1100 700]}]}"#,
    )
    .unwrap();
    assert_eq!(15, script.steps.len());
    assert!(Script::parse("{:steps [{:op :record-view}]}").is_err());
    assert!(Script::parse("{:steps [{:op :assert-view :zoom-times [:before]}]}").is_err());
    assert!(Script::parse("{:steps [{:op :click-class}]}").is_err());
    assert!(Script::parse("{:steps [{:op :resize-window}]}").is_err());
    assert!(Script::parse("{:steps [{:op :dump-quality :evidence \"q.txt\"}]}").is_err());
}

#[test]
fn inputs_replay_one_per_frame_so_assertions_see_the_applied_state() {
    let (evidence, project) = temp("pacing");
    let mut host = FakeHost::default();
    let mut runner = Runner::new(
        Script::parse(
            "{:steps [{:op :click :at [1 1]} {:op :click :at [2 2]} {:op :quit :code 0}]}",
        )
        .unwrap(),
    );
    assert_eq!(
        Progress::Running,
        runner.run(1, Some(&evidence), &project, &mut host).unwrap()
    );
    assert_eq!(1, host.inputs.len());
    assert_eq!(
        Progress::Running,
        runner.run(2, Some(&evidence), &project, &mut host).unwrap()
    );
    assert_eq!(2, host.inputs.len());
    assert_eq!(
        Progress::Done,
        runner.run(3, Some(&evidence), &project, &mut host).unwrap()
    );
    assert_eq!(Some(0), host.quit);
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn the_new_input_ops_reach_the_host_with_modifiers() {
    let (evidence, project) = temp("new-inputs");
    let mut host = FakeHost::default();
    let progress = run_all(
        r#"{:steps [{:op :mouse-move :at [300 260]}
                    {:op :wheel :dy 1 :ctrl true}
                    {:op :wheel :dx -2 :shift true}
                    {:op :paste :text "中文解析"}
                    {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        8,
    )
    .unwrap();
    assert_eq!(Progress::Done, progress);
    assert_eq!(
        vec![
            Input::MouseMove { x: 300.0, y: 260.0 },
            Input::Wheel {
                dx: 0.0,
                dy: 1.0,
                ctrl: true,
                shift: false,
            },
            Input::Wheel {
                dx: -2.0,
                dy: 0.0,
                ctrl: false,
                shift: true,
            },
            Input::Paste("中文解析".into()),
        ],
        host.inputs
    );
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn step_modifiers_hold_only_for_boolean_true() {
    // Driver steps are bench instrumentation with no Lua reader, not policy,
    // IR or mail: a modifier is held only for `true`; `:yes` and `1` are not.
    let wheel = |ctrl: &str| {
        let (evidence, project) = temp("modifier-flags");
        let mut host = FakeHost::default();
        let script = format!("{{:steps [{{:op :wheel :dy 1 {ctrl}}} {{:op :quit :code 0}}]}}");
        run_all(&script, &evidence, &project, &mut host, 4).unwrap();
        let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
        host.inputs
    };
    let held = |ctrl: bool| {
        vec![Input::Wheel {
            dx: 0.0,
            dy: 1.0,
            ctrl,
            shift: false,
        }]
    };
    for v in ["", ":ctrl false", ":ctrl nil", ":ctrl :yes", ":ctrl 1"] {
        assert_eq!(held(false), wheel(v), "{v:?}");
    }
    assert_eq!(held(true), wheel(":ctrl true"));
}

#[test]
fn dump_view_writes_the_view_record_as_edn() {
    let (evidence, project) = temp("dump-view");
    let mut host = FakeHost {
        record: ViewRecord {
            focus: vec!["source".into()],
            selected: Some("source".into()),
            declutter: "arrows".into(),
            search: viewer::driver::SearchRecord {
                active: true,
                query: "解析".into(),
                results: vec!["alpha.deep.parser".into()],
                selection: Some("alpha.deep.parser".into()),
            },
            ..record(1.0, 0.0, 0.0)
        },
        ..FakeHost::default()
    };
    let progress = run_all(
        r#"{:steps [{:op :dump-view :evidence "navigation-entered.edn"}
                    {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    )
    .unwrap();
    assert_eq!(Progress::Done, progress);
    let dump = std::fs::read_to_string(evidence.join("navigation-entered.edn")).unwrap();
    for needle in [
        ":focus [:source]",
        ":selected :source",
        ":declutter :arrows",
        ":active true",
        ":query \"解析\"",
        ":results [:alpha.deep.parser]",
        ":selection :alpha.deep.parser",
    ] {
        assert!(dump.contains(needle), "dump lacks {needle}: {dump}");
    }
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn assert_view_checks_zoom_steps_and_the_recorded_anchor() {
    let (evidence, project) = temp("assert-view");
    // Recorded at zoom 1, anchor world (1300, 1260); after one ctrl+wheel step
    // the fake host reports zoom 1.1 and the same anchor world point.
    let mut host = FakeHost::default();
    let script = r#"{:steps [{:op :record-view :as :before :anchor [300 260]}
                    {:op :wait-frames :n 1}
                    {:op :assert-view :zoom-times [:before 1.1] :anchor [300 260]
                     :anchor-fixed :before :message "zoom step"}
                    {:op :quit :code 0}]}"#;
    host.record = record(1.0, 0.0, 0.0);
    let mut runner = Runner::new(Script::parse(script).unwrap());
    assert_eq!(
        Progress::Running,
        runner.run(1, Some(&evidence), &project, &mut host).unwrap()
    );
    host.record = record(1.1, -27.0, -23.0);
    assert_eq!(
        Progress::Done,
        runner.run(2, Some(&evidence), &project, &mut host).unwrap()
    );
    // A wrong factor fails with the step message.
    let mut host = FakeHost {
        record: record(1.0, 0.0, 0.0),
        ..FakeHost::default()
    };
    let mut runner = Runner::new(Script::parse(script).unwrap());
    let _ = runner.run(1, Some(&evidence), &project, &mut host);
    host.record = record(1.2, 0.0, 0.0);
    assert_eq!(
        Err("zoom step".to_string()),
        runner.run(2, Some(&evidence), &project, &mut host)
    );
    // The zoom clamps at 10.
    let mut host = FakeHost {
        record: record(9.5, 0.0, 0.0),
        ..FakeHost::default()
    };
    let mut runner = Runner::new(
        Script::parse(
            "{:steps [{:op :record-view :as :b} {:op :wait-frames :n 1}
                  {:op :assert-view :zoom-times [:b 1.1] :message \"clamp\"}
                  {:op :quit :code 0}]}",
        )
        .unwrap(),
    );
    let _ = runner.run(1, Some(&evidence), &project, &mut host);
    host.record = record(10.0, 0.0, 0.0);
    assert_eq!(
        Progress::Done,
        runner.run(2, Some(&evidence), &project, &mut host).unwrap()
    );
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn assert_view_checks_pan_shifts_fitting_and_named_fields() {
    let (evidence, project) = temp("assert-pan");
    let script = r#"{:steps [{:op :record-view :as :before}
                    {:op :wait-frames :n 1}
                    {:op :assert-view :cam-shift [[100 50] :before] :dragging false
                     :focus [:source] :selected "source" :declutter "arrows"
                     :fitted true :message "pan"}
                    {:op :assert-view :cam-y-not-below :before :zoom-unchanged :before
                     :cam-unchanged :before :focus-unchanged :before
                     :selected-unchanged :before :message "unchanged"}
                    {:op :quit :code 0}]}"#;
    let mut host = FakeHost {
        fitted: true,
        ..FakeHost::default()
    };
    host.record = ViewRecord {
        zoom: 2.0,
        ..record(2.0, 10.0, 20.0)
    };
    let mut runner = Runner::new(Script::parse(script).unwrap());
    assert_eq!(
        Progress::Running,
        runner.run(1, Some(&evidence), &project, &mut host).unwrap()
    );
    // cam moves by the screen delta over the recorded zoom: +50/+25.
    host.record = ViewRecord {
        focus: vec!["source".into()],
        selected: Some("source".into()),
        declutter: "arrows".into(),
        ..record(2.0, 60.0, 45.0)
    };
    // The unchanged block must fail: cam moved.
    assert_eq!(
        Err("unchanged".to_string()),
        runner.run(2, Some(&evidence), &project, &mut host)
    );
    // The shift/field block alone passes; an unfitted view fails it.
    let shift_only = r#"{:steps [{:op :record-view :as :before}
                    {:op :wait-frames :n 1}
                    {:op :assert-view :cam-shift [[100 50] :before] :dragging false
                     :focus [:source] :selected "source" :declutter "arrows"
                     :fitted true :message "pan"}
                    {:op :quit :code 0}]}"#;
    for (fitted, expected) in [(true, Ok(Progress::Done)), (false, Err("pan".to_string()))] {
        let mut host = FakeHost {
            fitted,
            ..FakeHost::default()
        };
        host.record = record(2.0, 10.0, 20.0);
        let mut runner = Runner::new(Script::parse(shift_only).unwrap());
        let _ = runner.run(1, Some(&evidence), &project, &mut host);
        host.record = ViewRecord {
            focus: vec!["source".into()],
            selected: Some("source".into()),
            declutter: "arrows".into(),
            ..record(2.0, 60.0, 45.0)
        };
        assert_eq!(
            expected,
            runner.run(2, Some(&evidence), &project, &mut host)
        );
    }
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}

#[test]
fn class_clicks_blank_checks_and_chrome_go_through_the_host() {
    let (evidence, project) = temp("host-routing");
    let mut host = FakeHost {
        blank: true,
        fonts_ok: true,
        fits_ok: true,
        display: Some(edn::parse("{:ui 1 :window {:w 1500 :h 920}}").unwrap()),
        quality: Some("missing=true band=red".into()),
        ..FakeHost::default()
    };
    let progress = run_all(
        r#"{:steps [{:op :assert-blank :at [900 860] :message "drag starts on blank canvas"}
                    {:op :click-class :id :source}
                    {:op :double-click-class :id :source.lua}
                    {:op :click-declutter}
                    {:op :assert-fonts :message "fonts"}
                    {:op :assert-text-fits :message "fits"}
                    {:op :dump-display :evidence "display.edn"}
                    {:op :dump-quality :evidence "quality-diagnosis.txt" :id :uml.application.detail}
                    {:op :resize-window :size [1100 700]}
                    {:op :quit :code 0}]}"#,
        &evidence,
        &project,
        &mut host,
        4,
    )
    .unwrap();
    assert_eq!(Progress::Done, progress);
    assert_eq!(
        vec![
            ("source".to_string(), false),
            ("source.lua".to_string(), true)
        ],
        host.class_points
    );
    assert_eq!(1, host.declutter_clicks);
    assert_eq!(vec![(1100.0, 700.0)], host.resized);
    // The EDN encoder sorts map keys.
    assert_eq!(
        "{:ui 1 :window {:h 920 :w 1500}}",
        std::fs::read_to_string(evidence.join("display.edn")).unwrap()
    );
    let quality = std::fs::read_to_string(evidence.join("quality-diagnosis.txt")).unwrap();
    assert!(quality.contains("uml.application.detail: missing=true band=red"));
    // A non-blank point and an unknown class fail with the step message.
    let mut host = FakeHost::default();
    let failed = run_all(
        r#"{:steps [{:op :assert-blank :at [1 1] :message "drag starts on blank canvas"}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Err("drag starts on blank canvas".to_string()), failed);
    let mut host = FakeHost::default();
    let failed = run_all(
        r#"{:steps [{:op :click-class :id :missing}]}"#,
        &evidence,
        &project,
        &mut host,
        2,
    );
    assert_eq!(Err("no class missing on the scene".to_string()), failed);
    let _ = std::fs::remove_dir_all(evidence.parent().unwrap());
}
