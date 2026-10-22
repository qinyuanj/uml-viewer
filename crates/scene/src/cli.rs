//! `uml-viewer scene` command line (#137): read IR EDN, compile one view per
//! focus, write the geometry projection in the JVM golden view shape.
//!
//! ```text
//! uml-viewer scene --ir <file.edn> [--focus <edn>]... [--metrics-root <dir>]
//!                  [--output <file>]
//! ```
//!
//! `--focus` may repeat; `[]` (the default) is the root view. `--metrics-root`
//! loads `<root>/.metrics/crap.edn` and `<root>/.metrics/mutate/*.edn`;
//! without it the scene has no quality overlay, exactly like the parity specs.

use crate::curve;
use crate::document::compile_view;
use crate::geom::{self, get};
use crate::overlay::Snapshot;
use edn::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: uml-viewer scene --ir <file.edn> [--focus <edn>]... \
[--metrics-root <dir>] [--output <file>]";

const EX_USAGE: u8 = 2;
const EX_FAILURE: u8 = 1;

/// Geometry projection of one compiled view: the golden view shape.
pub fn project(doc: &Value, snapshots: Option<&[Snapshot]>, focus: &Value, extra: &Value) -> Value {
    let snapshots = snapshots.unwrap_or(&[]);
    let empty = Vec::new();
    let focus_path = match focus {
        Value::Vector(items) => items.clone(),
        Value::Nil => empty,
        _ => empty,
    };
    let scene = compile_view(doc, snapshots, &focus_path, extra);
    if get(&scene, "sections").is_some() {
        section_projection(&scene, focus)
    } else {
        diagram_projection(&scene, focus)
    }
}

fn diagram_projection(scene: &Value, focus: &Value) -> Value {
    let classes = match get(scene, "classes") {
        Some(Value::Vector(v)) => v.clone(),
        _ => vec![],
    };
    let index: std::collections::HashMap<String, geom::Rect> = classes
        .iter()
        .filter_map(|c| {
            let id = get(c, "id")?;
            let rect = get(c, "rect").and_then(geom::Rect::from_value)?;
            Some((edn::encode(id).unwrap(), rect))
        })
        .collect();
    let packages: Vec<Value> = geom::get_slice(get(scene, "packages"))
        .iter()
        .map(|p| {
            Value::Map(vec![
                (
                    Value::keyword("id"),
                    get(p, "id").cloned().unwrap_or(Value::Nil),
                ),
                (
                    Value::keyword("rect"),
                    get(p, "rect").cloned().unwrap_or(Value::Nil),
                ),
            ])
        })
        .collect();
    let edges: Vec<Value> = geom::get_slice(get(scene, "edges"))
        .iter()
        .map(|e| {
            let from = get(e, "from").cloned().unwrap_or(Value::Nil);
            let to = get(e, "to").cloned().unwrap_or(Value::Nil);
            let points = get(e, "points").cloned().unwrap_or(Value::Vector(vec![]));
            let pts = geom::get_slice(get(e, "points")).to_vec();
            let path = curve::constrain_ends(
                &curve::basis_path(&pts),
                index.get(&edn::encode(&from).unwrap()).copied(),
                index.get(&edn::encode(&to).unwrap()).copied(),
            );
            Value::Map(vec![
                (Value::keyword("from"), from),
                (Value::keyword("to"), to),
                (
                    Value::keyword("kind"),
                    get(e, "kind").cloned().unwrap_or(Value::Nil),
                ),
                (Value::keyword("points"), points),
                (
                    Value::keyword("tip"),
                    get(e, "tip").cloned().unwrap_or(Value::Nil),
                ),
                (
                    Value::keyword("behind"),
                    get(e, "behind").cloned().unwrap_or(Value::Nil),
                ),
                (
                    Value::keyword("draw-bounds"),
                    get(e, "draw-bounds").cloned().unwrap_or(Value::Nil),
                ),
                (Value::keyword("curve"), path),
            ])
        })
        .collect();
    let size = get(scene, "size").cloned().unwrap_or(Value::Map(vec![]));
    let mut view = Value::Map(vec![
        (Value::keyword("focus"), focus.clone()),
        (Value::keyword("size"), size),
        (Value::keyword("packages"), Value::Vector(packages)),
        (Value::keyword("classes"), Value::Vector(classes)),
        (Value::keyword("edges"), Value::Vector(edges)),
    ]);
    for k in ["foreign", "title", "proposal", "dep-indicators"] {
        if let Some(v) = get(scene, k) {
            geom::put(&mut view, k, v.clone());
        }
    }
    view
}

/// Multi-diagram documents have no golden views. Each section is exported
/// with its own size, class rects and routed edges (clipping happens once on
/// the stacked scene, so section edges carry `:points` but no `:strokes`).
fn section_projection(scene: &Value, focus: &Value) -> Value {
    let sections: Vec<Value> = geom::get_slice(get(scene, "sections"))
        .iter()
        .map(|s| {
            let classes: Vec<Value> = geom::get_slice(get(s, "classes")).to_vec();
            let edges: Vec<Value> = geom::get_slice(get(s, "edges")).to_vec();
            Value::Map(vec![
                (
                    Value::keyword("title"),
                    get(s, "title").cloned().unwrap_or(Value::Nil),
                ),
                (
                    Value::keyword("size"),
                    get(s, "size").cloned().unwrap_or(Value::Map(vec![])),
                ),
                (
                    Value::keyword("classes"),
                    Value::Vector(
                        classes
                            .iter()
                            .map(|c| {
                                Value::Map(vec![
                                    (
                                        Value::keyword("id"),
                                        get(c, "id").cloned().unwrap_or(Value::Nil),
                                    ),
                                    (
                                        Value::keyword("rect"),
                                        get(c, "rect").cloned().unwrap_or(Value::Nil),
                                    ),
                                ])
                            })
                            .collect(),
                    ),
                ),
                (Value::keyword("edges"), Value::Vector(edges)),
            ])
        })
        .collect();
    Value::Map(vec![
        (Value::keyword("focus"), focus.clone()),
        (
            Value::keyword("size"),
            get(scene, "size").cloned().unwrap_or(Value::Map(vec![])),
        ),
        (Value::keyword("sections"), Value::Vector(sections)),
    ])
}

#[derive(Default)]
struct Options {
    ir: Option<PathBuf>,
    focuses: Vec<Value>,
    metrics_root: Option<PathBuf>,
    output: Option<PathBuf>,
}

fn parse_args(args: &[OsString]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i]
            .to_str()
            .ok_or_else(|| format!("argument is not valid UTF-8: {args:?}"))?;
        let mut next = |name: &str| -> Result<OsString, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match arg {
            "--ir" => opts.ir = Some(PathBuf::from(next("--ir")?)),
            "--focus" => {
                let raw = next("--focus")?;
                let text = raw
                    .to_str()
                    .ok_or_else(|| "--focus must be valid UTF-8".to_string())?;
                let value = edn::parse(text).map_err(|e| format!("--focus: {e}"))?;
                if !matches!(value, Value::Vector(_)) {
                    return Err("--focus must be an EDN vector of keywords".into());
                }
                opts.focuses.push(value);
            }
            "--metrics-root" => opts.metrics_root = Some(PathBuf::from(next("--metrics-root")?)),
            "--output" => opts.output = Some(PathBuf::from(next("--output")?)),
            other => return Err(format!("unknown option {other}")),
        }
        i += 1;
    }
    if opts.ir.is_none() {
        return Err("--ir is required".into());
    }
    if opts.focuses.is_empty() {
        opts.focuses.push(Value::Vector(vec![]));
    }
    Ok(opts)
}

/// Load quality snapshots from `<root>/.metrics`, mirroring the adapter's
/// file discovery (CRAP tree plus every mutation snapshot).
pub fn load_snapshots(root: &Path) -> Vec<Snapshot> {
    let mut files = Vec::new();
    let crap = root.join(".metrics/crap.edn");
    if let Ok(text) = std::fs::read_to_string(&crap) {
        files.push(Snapshot::new(
            ".metrics/crap.edn",
            &text,
            mtime(".metrics/crap.edn", &crap),
        ));
    }
    let mutate = root.join(".metrics/mutate");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&mutate)
        .map(|dir| {
            dir.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    for path in entries {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let rel = format!(
                ".metrics/mutate/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            files.push(Snapshot::new(&rel, &text, mtime(&rel, &path)));
        }
    }
    files
}

fn mtime(_rel: &str, path: &Path) -> Value {
    match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(time) => match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => Value::Integer(d.as_secs() as i64),
            Err(_) => Value::Integer(0),
        },
        Err(_) => Value::Integer(0),
    }
}

pub fn run(args: &[OsString]) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("uml-viewer scene: {message}\n{USAGE}");
            return ExitCode::from(EX_USAGE);
        }
    };
    match execute(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("uml-viewer scene: {message}");
            ExitCode::from(EX_FAILURE)
        }
    }
}

fn execute(opts: &Options) -> Result<(), String> {
    let ir_path = opts.ir.as_ref().unwrap();
    let text = std::fs::read_to_string(ir_path)
        .map_err(|e| format!("cannot read {}: {e}", ir_path.display()))?;
    let doc = crate::ir::read_document(&text)
        .map_err(|e| format!("invalid IR {}: {e}", ir_path.display()))?;
    let snapshots = match &opts.metrics_root {
        Some(root) => load_snapshots(root),
        None => Vec::new(),
    };
    let views: Vec<Value> = opts
        .focuses
        .iter()
        .map(|focus| project(&doc, Some(&snapshots), focus, &Value::Map(vec![])))
        .collect();
    let out = Value::Map(vec![
        (Value::keyword("version"), Value::Integer(1)),
        (Value::keyword("views"), Value::Vector(views)),
    ]);
    let text = format!("{}\n", edn::encode(&out).map_err(|e| e.to_string())?);
    match &opts.output {
        Some(path) => std::fs::write(path, &text)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?,
        None => print!("{text}"),
    }
    Ok(())
}
