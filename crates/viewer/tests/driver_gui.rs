//! GUI driver ops (#141): pointer/key steps reach the host, the runner waits
//! while queued GUI events are undelivered, and `:assert-gui` compares the
//! published GUI record.

use edn::Value;
use viewer::driver::{
    self, GuiCheck, GuiTarget, Host, Input, Progress, Runner, Script, ViewRecord,
};

#[derive(Default)]
struct GuiHost {
    queued: u32,
    points: Vec<GuiTarget>,
    buttons: Vec<(bool, bool)>,
    keys: Vec<(String, bool)>,
    record: Option<Value>,
    quit: Option<u8>,
}

impl Host for GuiHost {
    fn inject(&mut self, _: Input) {}
    fn assert_italic_glyphs(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn screenshot(&mut self, _: &str) -> bool {
        true
    }
    fn set_view(&mut self, _: Option<f64>, _: Option<[f64; 2]>) {}
    fn quit(&mut self, code: u8) {
        self.quit = Some(code);
    }
    fn view_record(&mut self, _: Option<[f64; 2]>) -> ViewRecord {
        ViewRecord::default()
    }
    fn is_fitted(&self) -> bool {
        true
    }
    fn is_blank(&self, _: f64, _: f64) -> bool {
        true
    }
    fn click_class(&mut self, _: &str, _: bool, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn click_declutter(&mut self) {}
    fn assert_fonts(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn assert_text_fits(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn display_record(&self) -> Value {
        Value::Nil
    }
    fn quality_line(&self, _: &str) -> Option<String> {
        None
    }
    fn resize_window(&mut self, _: f64, _: f64) {}
    fn busy(&self) -> bool {
        self.queued > 0
    }
    fn gui_point(&mut self, target: &GuiTarget) -> Result<(), String> {
        self.points.push(target.clone());
        Ok(())
    }
    fn gui_button(&mut self, secondary: bool, pressed: bool) {
        self.buttons.push((secondary, pressed));
    }
    fn gui_click(&mut self, target: &GuiTarget, _: bool) -> Result<(), String> {
        self.points.push(target.clone());
        self.queued = 3;
        Ok(())
    }
    fn gui_key(&mut self, key: &str, ctrl: bool, _: bool) {
        self.keys.push((key.into(), ctrl));
    }
    fn gui_record(&self) -> Value {
        self.record.clone().unwrap_or(Value::Nil)
    }
}

fn run(runner: &mut Runner, host: &mut GuiHost, frame: u64) -> Result<Progress, String> {
    runner.run(
        frame,
        Some(std::env::temp_dir().as_path()),
        std::path::Path::new("."),
        host,
    )
}

#[test]
fn gui_pointer_and_key_steps_reach_the_host() {
    let script = Script::parse(
        r#"{:steps [{:op :gui-point :at "source-line:82" :prefix "  " :offset [0 2]}
                    {:op :gui-press :button :secondary}
                    {:op :gui-release}
                    {:op :gui-key :key "a" :ctrl true}
                    {:op :quit :code 0}]}"#,
    )
    .unwrap();
    let mut runner = Runner::new(script);
    let mut host = GuiHost::default();
    for frame in 1..10 {
        if run(&mut runner, &mut host, frame).unwrap() == Progress::Done {
            break;
        }
    }
    assert_eq!(host.points[0].at.as_deref(), Some("source-line:82"));
    assert_eq!(host.points[0].prefix.as_deref(), Some("  "));
    assert_eq!(host.points[0].offset, [0.0, 2.0]);
    assert_eq!(host.buttons, [(true, true), (false, false)]);
    assert_eq!(host.keys, [("a".to_string(), true)]);
    assert_eq!(host.quit, Some(0));
}

#[test]
fn the_runner_waits_until_a_staged_click_is_delivered() {
    let script =
        Script::parse(r#"{:steps [{:op :gui-click :at "regen"} {:op :quit :code 0}]}"#).unwrap();
    let mut runner = Runner::new(script);
    let mut host = GuiHost::default();
    assert_eq!(run(&mut runner, &mut host, 1).unwrap(), Progress::Running);
    assert_eq!(run(&mut runner, &mut host, 2).unwrap(), Progress::Running);
    assert_eq!(host.quit, None, "the click is still being delivered");
    host.queued = 0;
    assert_eq!(run(&mut runner, &mut host, 3).unwrap(), Progress::Done);
    assert_eq!(host.quit, Some(0));
}

#[test]
fn assert_gui_compares_the_record_and_reports_the_step_message() {
    let record = edn::parse(
        r#"{:inspector {:width 340.0 :menu-open true}
            :card {:scroll 192.0 :hover "op8" :horizontal {:maximum 0.0}}
            :sources [{:line 82 :selection "function"} {:line nil}]}"#,
    )
    .unwrap();
    let path = |s: &str| match edn::parse(s).unwrap() {
        Value::Vector(v) => v,
        _ => unreachable!(),
    };
    let ok = |p: &str, c: GuiCheck| driver::check_gui(&record, &path(p), &c);
    assert!(ok("[:inspector :width]", GuiCheck::Equals(Value::Integer(340))).is_ok());
    assert!(ok("[:card :scroll]", GuiCheck::Equals(Value::Integer(192))).is_ok());
    assert!(ok("[:card :horizontal :maximum]", GuiCheck::Lt(1.0)).is_ok());
    assert!(
        ok(
            "[:card :hover]",
            GuiCheck::Equals(Value::String("op8".into()))
        )
        .is_ok()
    );
    assert!(
        ok(
            "[:sources 0 :selection]",
            GuiCheck::Equals(Value::String("function".into()))
        )
        .is_ok()
    );
    assert!(ok("[:sources]", GuiCheck::Count(2)).is_ok());
    assert!(ok("[:sources 1 :line]", GuiCheck::Present(false)).is_ok());
    assert!(
        ok(
            "[:inspector :menu-open]",
            GuiCheck::Equals(Value::Bool(true))
        )
        .is_ok()
    );
    let err = ok("[:card :scroll]", GuiCheck::Gt(200.0)).unwrap_err();
    assert!(err.contains("192"), "{err}");

    let script = Script::parse(
        r#"{:steps [{:op :assert-gui :path [:card :scroll] :equals 144 :message "three notches must restore 144px"}]}"#,
    )
    .unwrap();
    let mut runner = Runner::new(script);
    let mut host = GuiHost {
        record: Some(record),
        ..GuiHost::default()
    };
    assert_eq!(
        run(&mut runner, &mut host, 1).unwrap_err(),
        "three notches must restore 144px"
    );
}

#[test]
fn rejects_gui_steps_without_a_target_or_check() {
    assert!(Script::parse(r#"{:steps [{:op :gui-click}]}"#).is_err());
    assert!(Script::parse(r#"{:steps [{:op :assert-gui :path [:a]}]}"#).is_err());
}

#[test]
fn dependency_bundle_driver_uses_the_existing_declarative_viewer_seam() {
    Script::parse(include_str!(
        "../../../scripts/drivers/dependency_bundles.edn"
    ))
    .unwrap();
}
