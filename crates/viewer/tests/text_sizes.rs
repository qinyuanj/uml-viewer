//! Canvas text draws at one size (the 16px canvas size) in every view of
//! this repository's own diagram, under the provisioned Sarasa face and the
//! real fit the painter applies. Without Sarasa the test reports a skip:
//! another face has other advances, so it would measure nothing.

use std::collections::BTreeMap;
use std::path::Path;

use edn::Value;
use edn::record::{get, is_true, local_name};
use viewer::commands::{self, Command};
use viewer::paint::{self, TextCache};
use viewer::theme::fonts;

#[test]
fn every_view_draws_its_canvas_text_at_the_canvas_size() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ctx = egui::Context::default();
    match viewer::fonts::install(&ctx, &root) {
        Some((_, "sarasa")) => {}
        other => {
            eprintln!("skip: needs the provisioned Sarasa face (get-uml-viewer); found {other:?}");
            return;
        }
    }
    ctx.begin_pass(egui::RawInput::default());
    // The layout sizes boxes for Sarasa's half-width Latin at the canvas
    // size; the painter must measure the same advance.
    let advance = paint::measure(&ctx, fonts::CANVAS, "x");
    assert_eq!(scene::CHAR_W, advance, "Sarasa advance at the canvas size");
    let text = std::fs::read_to_string(root.join("examples/uml-viewer.edn")).unwrap();
    let doc = scene::ir::read_document(&text).unwrap();
    let (snapshots, _) = viewer::metrics::snapshot(&root);
    let mut cache = TextCache::default();
    let mut off: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut views = 0;
    let mut queue: Vec<Vec<Value>> = vec![vec![]];
    while let Some(focus) = queue.pop() {
        let view = scene::document::compile_view(&doc, &snapshots, &focus, &Value::Map(vec![]));
        views += 1;
        for c in match get(&view, "classes") {
            Some(Value::Vector(cs)) => cs.as_slice(),
            _ => &[],
        } {
            if is_true(c, "drill?") {
                let id = local_name(get(c, "id").unwrap()).unwrap_or_default();
                queue.push(id.split('.').map(Value::keyword).collect());
            }
        }
        for command in commands::commands(&view, None, None) {
            let (role, label, size, max_w, bold, italic) = match &command {
                Command::Text {
                    role,
                    text,
                    size,
                    max_w,
                    bold,
                    italic,
                    ..
                } => (role.as_str(), text, *size, *max_w, *bold, *italic),
                Command::Package {
                    text, size, max_w, ..
                } => ("package", text, *size, *max_w, false, true),
                _ => continue,
            };
            if label.is_empty() {
                continue;
            }
            let px = paint::fit_px(
                &ctx,
                &mut cache,
                size as f32,
                label,
                max_w as f32,
                bold,
                italic,
            );
            if px != fonts::CANVAS {
                off.entry(role.to_owned())
                    .or_default()
                    .push(format!("{label} at {px}px in {focus:?}"));
            }
        }
    }
    ctx.end_pass().textures_delta.clear();
    assert!(views > 1, "the diagram has no drillable views");
    assert!(off.is_empty(), "canvas text off the canvas size: {off:#?}");
}
