//! Declarative EDN driver scripts (Seam B, #139): the viewer replays input
//! events and file/evidence assertions from a step list so the behavioural
//! drivers stay declarative and runtime-independent.
//!
//! ```text
//! {:version 1
//!  :steps [{:op :wait-frames :n 3}
//!          {:op :assert :evidence "scene-1.edn"
//!           :contains-any [":mu 1 " ":mu 1}"] :message "first scene lacks the crap-1 summary"}
//!          {:op :edit :project ".metrics/crap.edn" :from ":crap 1" :to ":crap 9"
//!           :same-length true}
//!          {:op :record-mtime :project "diagram.edn" :as :ir}
//!          {:op :wait-file :evidence "scene-2.edn"}
//!          {:op :assert :evidence "scene-2.edn" :differs-from "scene-1.edn"}
//!          {:op :write :evidence "metrics-pass.txt" :text "PASS ...\n"}
//!          {:op :quit :code 0}]}
//! ```
//!
//! The runner is window-free: a `Host` supplies the event injection, the
//! glyph probe and the quit; file checks run here. The app calls [`Runner::run`]
//! once per frame.

use edn::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::watch;

/// One replayed input event (view/canvas coordinates in points).
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Click {
        x: f64,
        y: f64,
    },
    DoubleClick {
        x: f64,
        y: f64,
    },
    Drag {
        from: [f64; 2],
        to: [f64; 2],
    },
    Wheel {
        dx: f64,
        dy: f64,
    },
    Key {
        key: String,
        ctrl: bool,
        shift: bool,
    },
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    WaitFrames(u64),
    WaitFile {
        evidence: String,
    },
    ReadEvidence {
        evidence: String,
        name: String,
    },
    Assert {
        evidence: String,
        contains: Vec<String>,
        contains_any: Vec<String>,
        differs_from: Option<String>,
        message: String,
    },
    Edit {
        project: String,
        from: String,
        to: String,
        same_length: bool,
    },
    RecordMtime {
        project: String,
        name: String,
    },
    AssertMtime {
        project: String,
        name: String,
        message: String,
    },
    Input(Input),
    /// Layout the probe text upright and slanted through the real font atlas
    /// and compare the tessellated glyph vertices (the egui equivalent of the
    /// cimgui vertex-slant assertion).
    AssertItalicGlyphs {
        message: String,
    },
    View {
        zoom: Option<f64>,
        center: Option<[f64; 2]>,
    },
    Screenshot {
        evidence: String,
    },
    Write {
        evidence: String,
        text: String,
    },
    Quit(u8),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Script {
    pub steps: Vec<Step>,
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(&Value::keyword(key))
}

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Keyword(k) | Value::Symbol(k) => Some(k.clone()),
        Value::Integer(n) => Some(n.to_string()),
        Value::Float(x) => Some(x.to_string()),
        _ => None,
    }
}

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Integer(n) => Some(*n as f64),
        Value::Float(x) => Some(*x),
        _ => None,
    }
}

fn flag(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Bool(true)))
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Vector(items)) => items.iter().filter_map(|x| text(Some(x))).collect(),
        Some(other) => text(Some(other)).into_iter().collect(),
        None => Vec::new(),
    }
}

fn pair(v: Option<&Value>) -> Option<[f64; 2]> {
    match v {
        Some(Value::Vector(items)) if items.len() == 2 => {
            Some([number(items.first())?, number(items.get(1))?])
        }
        _ => None,
    }
}

impl Script {
    pub fn parse(source: &str) -> Result<Script, String> {
        let value = edn::parse(source).map_err(|e| format!("driver script is not EDN: {e}"))?;
        let steps = match get(&value, "steps") {
            Some(Value::Vector(steps)) => steps,
            _ => return Err("driver script needs a :steps vector".into()),
        };
        let mut out = Vec::with_capacity(steps.len());
        for step in steps {
            out.push(parse_step(step)?);
        }
        Ok(Script { steps: out })
    }

    pub fn load(path: &Path) -> Result<Script, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read driver script {}: {e}", path.display()))?;
        Script::parse(&source)
    }
}

fn required<'a>(step: &'a Value, key: &str) -> Result<&'a Value, String> {
    get(step, key).ok_or_else(|| {
        format!(
            "driver step needs :{key}: {}",
            edn::encode(step).unwrap_or_default()
        )
    })
}

fn evidence_name(step: &Value) -> Result<String, String> {
    text(get(step, "evidence")).ok_or_else(|| "driver step needs :evidence".into())
}

fn input_of(step: &Value, op: &str) -> Result<Input, String> {
    let at = pair(get(step, "at"));
    let union = |name: &str| -> Result<[f64; 2], String> {
        pair(get(step, name)).ok_or_else(|| format!("{op} needs :{name} [x y]"))
    };
    Ok(match op {
        "click" => {
            let p = at.ok_or("click needs :at [x y]")?;
            Input::Click { x: p[0], y: p[1] }
        }
        "double-click" => {
            let p = at.ok_or("double-click needs :at [x y]")?;
            Input::DoubleClick { x: p[0], y: p[1] }
        }
        "drag" => Input::Drag {
            from: union("from")?,
            to: union("to")?,
        },
        "wheel" => Input::Wheel {
            dx: number(get(step, "dx")).unwrap_or(0.0),
            dy: number(get(step, "dy")).unwrap_or(0.0),
        },
        "key" => Input::Key {
            key: text(get(step, "key")).ok_or("key needs :key")?,
            ctrl: flag(get(step, "ctrl")),
            shift: flag(get(step, "shift")),
        },
        "text" => Input::Text(text(get(step, "text")).ok_or("text needs :text")?),
        other => return Err(format!("unknown driver op :{other}")),
    })
}

fn parse_step(step: &Value) -> Result<Step, String> {
    let op = text(get(step, "op")).ok_or("driver step needs :op")?;
    Ok(match op.as_str() {
        "click" | "double-click" | "drag" | "wheel" | "key" | "text" => {
            Step::Input(input_of(step, &op)?)
        }
        "wait-frames" => Step::WaitFrames(
            number(get(step, "n"))
                .ok_or("wait-frames needs :n")?
                .max(0.0) as u64,
        ),
        "wait-file" => Step::WaitFile {
            evidence: evidence_name(step)?,
        },
        "read" => Step::ReadEvidence {
            evidence: evidence_name(step)?,
            name: text(Some(required(step, "as")?)).ok_or("read needs :as")?,
        },
        "assert" => Step::Assert {
            evidence: evidence_name(step)?,
            contains: strings(get(step, "contains")),
            contains_any: strings(get(step, "contains-any")),
            differs_from: text(get(step, "differs-from")),
            message: text(get(step, "message")).unwrap_or_else(|| "driver assertion failed".into()),
        },
        "edit" => Step::Edit {
            project: text(Some(required(step, "project")?)).ok_or("edit needs :project")?,
            from: text(Some(required(step, "from")?)).ok_or("edit needs :from")?,
            to: text(Some(required(step, "to")?)).ok_or("edit needs :to")?,
            same_length: flag(get(step, "same-length")),
        },
        "record-mtime" => Step::RecordMtime {
            project: text(Some(required(step, "project")?)).ok_or("record-mtime needs :project")?,
            name: text(Some(required(step, "as")?)).ok_or("record-mtime needs :as")?,
        },
        "assert-mtime" => Step::AssertMtime {
            project: text(Some(required(step, "project")?)).ok_or("assert-mtime needs :project")?,
            name: text(Some(required(step, "same-as")?)).ok_or("assert-mtime needs :same-as")?,
            message: text(get(step, "message"))
                .unwrap_or_else(|| "file mtime changed unexpectedly".into()),
        },
        "assert-italic-glyphs" => Step::AssertItalicGlyphs {
            message: text(get(step, "message"))
                .unwrap_or_else(|| "italic names must slant real glyphs".into()),
        },
        "view" => Step::View {
            zoom: number(get(step, "zoom")),
            center: pair(get(step, "center")),
        },
        "screenshot" => Step::Screenshot {
            evidence: evidence_name(step)?,
        },
        "write" => Step::Write {
            evidence: evidence_name(step)?,
            text: text(get(step, "text")).ok_or("write needs :text")?,
        },
        "quit" => Step::Quit(number(get(step, "code")).unwrap_or(0.0).max(0.0) as u8),
        other => return Err(format!("unknown driver op :{other}")),
    })
}

/// The app-side effects of a driver step.
pub trait Host {
    fn inject(&mut self, input: Input);
    /// Lay out `探针` upright and slanted and compare glyph vertices.
    fn assert_italic_glyphs(&mut self) -> Result<(), String>;
    /// Request a screenshot; false until it has been written.
    fn screenshot(&mut self, evidence: &str) -> bool;
    fn set_view(&mut self, zoom: Option<f64>, center: Option<[f64; 2]>);
    fn quit(&mut self, code: u8);
}

#[derive(Debug, Default)]
pub struct Runner {
    script: Script,
    cursor: usize,
    base_frame: u64,
    values: HashMap<String, String>,
    started: bool,
}

/// Outcome of one frame's worth of driver work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Running,
    Done,
}

impl Runner {
    pub fn new(script: Script) -> Runner {
        Runner {
            script,
            cursor: 0,
            base_frame: 0,
            values: HashMap::new(),
            started: false,
        }
    }

    pub fn is_done(&self) -> bool {
        self.cursor >= self.script.steps.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Advance as far as this frame allows. Failures carry the step's
    /// assertion message, which the caller writes to stderr before exiting 1.
    pub fn run(
        &mut self,
        frame: u64,
        evidence: Option<&Path>,
        project: &Path,
        host: &mut dyn Host,
    ) -> Result<Progress, String> {
        if !self.started {
            self.started = true;
            self.base_frame = frame;
        }
        while let Some(step) = self.script.steps.get(self.cursor).cloned() {
            match step {
                Step::WaitFrames(n) => {
                    if frame.saturating_sub(self.base_frame) < n {
                        return Ok(Progress::Running);
                    }
                }
                Step::WaitFile { evidence: name } => {
                    if !self.evidence_path(evidence, &name)?.exists() {
                        return Ok(Progress::Running);
                    }
                }
                Step::ReadEvidence {
                    evidence: name,
                    name: slot,
                } => {
                    let path = self.evidence_path(evidence, &name)?;
                    let Ok(body) = std::fs::read_to_string(&path) else {
                        return Ok(Progress::Running);
                    };
                    self.values.insert(slot, body);
                }
                Step::Assert {
                    evidence: name,
                    contains,
                    contains_any,
                    differs_from,
                    message,
                } => {
                    let path = self.evidence_path(evidence, &name)?;
                    let Ok(body) = std::fs::read_to_string(&path) else {
                        return Ok(Progress::Running);
                    };
                    for needle in &contains {
                        if !body.contains(needle) {
                            return Err(message);
                        }
                    }
                    if !contains_any.is_empty() && !contains_any.iter().any(|n| body.contains(n)) {
                        return Err(message);
                    }
                    if let Some(other) = differs_from {
                        let other_path = self.evidence_path(evidence, &other)?;
                        let Ok(other_body) = std::fs::read_to_string(&other_path) else {
                            return Ok(Progress::Running);
                        };
                        if other_body == body {
                            return Err(message);
                        }
                    }
                }
                Step::Edit {
                    project: rel,
                    from,
                    to,
                    same_length,
                } => {
                    let path = project.join(&rel);
                    let body = std::fs::read_to_string(&path)
                        .map_err(|e| format!("cannot edit {}: {e}", path.display()))?;
                    if !body.contains(&from) {
                        return Err(format!("{} does not contain {from:?}", path.display()));
                    }
                    let edited = body.replace(&from, &to);
                    if same_length && edited.len() != body.len() {
                        return Err(format!(
                            "the edit {} -> {to:?} must keep the byte length",
                            from
                        ));
                    }
                    std::fs::write(&path, edited)
                        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                }
                Step::RecordMtime { project: rel, name } => {
                    let path = project.join(&rel);
                    let stamp = watch::mtime(&path)
                        .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
                    self.values.insert(format!("mtime:{name}"), stamp);
                }
                Step::AssertMtime {
                    project: rel,
                    name,
                    message,
                } => {
                    let path = project.join(&rel);
                    let stamp = watch::mtime(&path)
                        .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
                    if self.values.get(&format!("mtime:{name}")) != Some(&stamp) {
                        return Err(message);
                    }
                }
                Step::Input(input) => host.inject(input),
                Step::AssertItalicGlyphs { message } => host
                    .assert_italic_glyphs()
                    .map_err(|reason| format!("{message}: {reason}"))?,
                Step::View { zoom, center } => host.set_view(zoom, center),
                Step::Screenshot { evidence: name } => {
                    let target = self.evidence_path(evidence, &name)?;
                    if target.exists() || host.screenshot(&name) {
                        // fall through; the file is on disk
                    } else {
                        return Ok(Progress::Running);
                    }
                }
                Step::Write {
                    evidence: name,
                    text,
                } => {
                    let target = self.evidence_path(evidence, &name)?;
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
                    }
                    std::fs::write(&target, text)
                        .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
                }
                Step::Quit(code) => {
                    host.quit(code);
                    self.cursor = self.script.steps.len();
                    return Ok(Progress::Done);
                }
            }
            self.cursor += 1;
        }
        Ok(Progress::Done)
    }

    fn evidence_path(&self, evidence: Option<&Path>, name: &str) -> Result<PathBuf, String> {
        let dir = evidence.ok_or_else(|| format!("driver needs UML_VIEWER_EVIDENCE for {name}"))?;
        Ok(dir.join(name))
    }
}
