//! Declarative driver scripts: parsing, sequencing, file/evidence assertions
//! and the quit contract, exercised headlessly with a fake host.

use std::path::{Path, PathBuf};
use viewer::driver::{Input, Progress, Runner, Script};

#[derive(Default)]
struct FakeHost {
    inputs: Vec<Input>,
    shots: Vec<String>,
    views: Vec<(Option<f64>, Option<[f64; 2]>)>,
    quit: Option<u8>,
    italic: bool,
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
            Input::Wheel { dx: 0.0, dy: -1.0 },
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
