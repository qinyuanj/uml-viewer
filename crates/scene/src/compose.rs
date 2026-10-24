//! Scene compilation (port of `love/uml/engine/compose.lua`): layout, route,
//! fit, then `prepare_scene`. The Lua original rebuilt the class index per
//! edge (O(E·N)); here the index and a spatial grid over class rects are
//! built once per scene, so preparation is near-linear. Geometry is
//! unchanged: the grid yields the same obstacle set and `gap_segment`
//! merges intervals independent of rect order.

use crate::curve;
use crate::geom::{self, Rect};
use edn::Value;
use edn::record::{flag, get, put, seq};
use std::collections::HashMap;

/// Class rectangles keyed by encoded id, plus the obstacle grid. Rects of
/// dummy boxes are tracked but never clip strokes.
struct ClassIndex {
    rect_by_id: HashMap<String, Rect>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    obstacles: Vec<(Value, Rect, bool)>, // (id, rect, dummy?)
}

const CELL: f64 = 256.0;

impl ClassIndex {
    fn new(scene: &Value) -> ClassIndex {
        let mut index = ClassIndex {
            rect_by_id: HashMap::new(),
            cells: HashMap::new(),
            obstacles: Vec::new(),
        };
        for c in seq(get(scene, "classes")) {
            let (Some(id), Some(rect)) = (get(c, "id"), get(c, "rect").and_then(Rect::from_value))
            else {
                continue;
            };
            index.rect_by_id.insert(edn::encode(id).unwrap(), rect);
            let dummy = get(c, "dummy?").is_some();
            let slot = index.obstacles.len();
            index.obstacles.push((id.clone(), rect, dummy));
            let (x0, y0) = (
                (rect.x / CELL).floor() as i64,
                (rect.y / CELL).floor() as i64,
            );
            let (x1, y1) = (
                (rect.right() / CELL).floor() as i64,
                (rect.bottom() / CELL).floor() as i64,
            );
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    index.cells.entry((cx, cy)).or_default().push(slot);
                }
            }
        }
        index
    }

    fn rect_of(&self, id: &Value) -> Option<Rect> {
        self.rect_by_id.get(&edn::encode(id).unwrap()).copied()
    }

    /// Obstacle rects overlapping the probe, excluding dummy boxes and the
    /// edge's own endpoints.
    fn obstacles_in(&self, probe: Rect, from: Option<&Value>, to: Option<&Value>) -> Vec<Rect> {
        let (x0, y0) = (
            (probe.x / CELL).floor() as i64,
            (probe.y / CELL).floor() as i64,
        );
        let (x1, y1) = (
            (probe.right() / CELL).floor() as i64,
            (probe.bottom() / CELL).floor() as i64,
        );
        let mut seen: Vec<bool> = vec![false; self.obstacles.len()];
        let mut out = Vec::new();
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                let Some(slots) = self.cells.get(&(cx, cy)) else {
                    continue;
                };
                for &slot in slots {
                    if seen[slot] {
                        continue;
                    }
                    seen[slot] = true;
                    let (id, rect, dummy) = &self.obstacles[slot];
                    if *dummy || Some(id) == from || Some(id) == to || !probe.overlaps(*rect) {
                        continue;
                    }
                    out.push(*rect);
                }
            }
        }
        out
    }
}

/// The constrained basis path of a routed edge, when it has ≥ 2 points.
fn path(scene_edge: &Value, index: &ClassIndex) -> Option<Value> {
    let ps = match get(scene_edge, "points") {
        Some(Value::Vector(items)) if items.len() >= 2 => items,
        _ => return None,
    };
    let a = get(scene_edge, "from").and_then(|id| index.rect_of(id));
    let b = get(scene_edge, "to").and_then(|id| index.rect_of(id));
    Some(curve::constrain_ends(&curve::basis_path(ps), a, b))
}

fn translate(scene: &mut Value, dx: f64, dy: f64) {
    for key in ["classes", "packages"] {
        if let Some(Value::Vector(items)) = get(scene, key).cloned() {
            let mut moved = Vec::with_capacity(items.len());
            for mut x in items.into_iter() {
                if let Some(r) = get(&x, "rect").and_then(Rect::from_value) {
                    put(&mut x, "rect", r.translate(dx, dy).to_value());
                }
                moved.push(x);
            }
            put(scene, key, Value::Vector(moved));
        }
    }
    if let Some(Value::Vector(edges)) = get(scene, "edges").cloned() {
        let mut shifted = Vec::with_capacity(edges.len());
        for mut e in edges.into_iter() {
            if let Some(Value::Vector(points)) = get(&e, "points") {
                let ps: Vec<Value> = points
                    .iter()
                    .map(|p| {
                        let p = geom::vec2(p).unwrap();
                        geom::point([p[0] + dx, p[1] + dy])
                    })
                    .collect();
                put(&mut e, "points", Value::Vector(ps));
            }
            shifted.push(e);
        }
        put(scene, "edges", Value::Vector(shifted));
    }
}

fn bounds(scene: &Value, index: &ClassIndex) -> Rect {
    let mut rects: Vec<Option<Rect>> = Vec::new();
    for key in ["classes", "packages"] {
        for c in seq(get(scene, key)) {
            rects.push(get(c, "rect").and_then(Rect::from_value));
        }
    }
    for e in seq(get(scene, "edges")) {
        if let Some(p) = path(e, index) {
            rects.push(curve::path_bounds(&p));
        }
    }
    geom::union(&rects)
        .map(|b| b.inflate(16.0))
        .unwrap_or(Rect::new(0.0, 0.0, 400.0, 300.0))
}

fn size(scene: &mut Value) {
    // Rebuild the index: rects moved since the caller's index was built.
    let index = ClassIndex::new(scene);
    let b = bounds(scene, &index);
    put(
        scene,
        "size",
        Value::Map(vec![
            (Value::keyword("w"), geom::numval(b.right() + 40.0)),
            (Value::keyword("h"), geom::numval(b.bottom() + 40.0)),
            (Value::keyword("min-x"), geom::numval((b.x - 40.0).min(0.0))),
            (Value::keyword("min-y"), geom::numval((b.y - 40.0).min(0.0))),
        ]),
    );
}

fn fit(scene: &mut Value) {
    let index = ClassIndex::new(scene);
    let b = bounds(scene, &index);
    translate(scene, (40.0 - b.x).max(0.0), (40.0 - b.y).max(0.0));
    size(scene);
}

/// `prepare_scene`: per edge, clip the flattened curve against every
/// overlapping class box and record `:tip`, `:behind`, `:draw-bounds` and
/// `:strokes`; then index classes and packages by id.
pub fn prepare_scene(scene: &mut Value) {
    let index = ClassIndex::new(scene);
    let mut edges: Vec<Value> = Vec::new();
    for raw in seq(get(scene, "edges")) {
        let mut e = raw.clone();
        if let Some(p) = path(&e, &index) {
            let bb = curve::path_bounds(&p).unwrap();
            let probe = bb.inflate(13.0);
            let rects = index.obstacles_in(probe, get(&e, "from"), get(&e, "to"));
            let tangent = curve::end_tangent(&p);
            put(&mut e, "behind", tangent[0].clone());
            put(&mut e, "tip", tangent[1].clone());
            put(&mut e, "draw-bounds", bb.to_value());
            let flat = curve::flatten_path(&p, 0.0625);
            let strokes = geom::gap_polyline(&flat, &rects, 5.0);
            put(
                &mut e,
                "strokes",
                Value::Vector(
                    strokes
                        .iter()
                        .map(|s| Value::Vector(s.iter().map(|p| geom::point(*p)).collect()))
                        .collect(),
                ),
            );
        }
        edges.push(e);
    }
    put(scene, "edges", Value::Vector(edges));
    for (list, key) in [("classes", "class-by-id"), ("packages", "package-by-id")] {
        let mut idx = Value::Map(vec![]);
        for c in seq(get(scene, list)) {
            if let Some(id) = get(c, "id") {
                idx.assoc(id.clone(), c.clone());
            }
        }
        put(scene, key, idx);
    }
}

/// One diagram: layout, route, fit, prepare; `:hide-edges` swaps strokes for
/// dependency indicators.
pub fn compile_diagram(diagram: &Value) -> Value {
    let mut scene = crate::layout::layout(diagram);
    crate::route::route(&mut scene);
    fit(&mut scene);
    prepare_scene(&mut scene);
    if flag(diagram, "hide-edges") {
        let indicators = crate::hit::dep_indicators(&scene);
        put(&mut scene, "dep-indicators", indicators);
    }
    scene
}

/// Rename a scene's ids with a diagram prefix, so several diagrams can share
/// one merged scene without id collisions.
fn prefix_ids(scene: &mut Value, prefix: &str) {
    let q = |id: &Value| {
        Value::keyword(&format!(
            "{}{}",
            prefix,
            edn::record::local_name(id).unwrap_or_default().to_owned()
        ))
    };
    for key in ["packages", "classes"] {
        if let Some(Value::Vector(items)) = get(scene, key).cloned() {
            let mut renamed = Vec::with_capacity(items.len());
            for mut c in items.into_iter() {
                if let Some(id) = get(&c, "id").cloned() {
                    put(&mut c, "id", q(&id));
                }
                if let Some(pkg) = get(&c, "package").cloned() {
                    put(&mut c, "package", q(&pkg));
                }
                renamed.push(c);
            }
            put(scene, key, Value::Vector(renamed));
        }
    }
    if let Some(Value::Vector(items)) = get(scene, "edges").cloned() {
        let mut renamed = Vec::with_capacity(items.len());
        for mut e in items.into_iter() {
            if let Some(from) = get(&e, "from").cloned() {
                put(&mut e, "from", q(&from));
            }
            if let Some(to) = get(&e, "to").cloned() {
                put(&mut e, "to", q(&to));
            }
            renamed.push(e);
        }
        put(scene, "edges", Value::Vector(renamed));
    }
}

/// A document compiled as one or more sections, stacked vertically with
/// `d<i>-` id prefixes so the sections cannot collide.
pub fn compile_document(doc: &Value) -> Value {
    let (mut sections, mut classes, mut packages, mut edges) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut y = 24.0;
    let diagrams = match get(doc, "diagrams") {
        Some(Value::Vector(ds)) => ds.clone(),
        _ => vec![doc.clone()],
    };
    for (i, d) in diagrams.iter().enumerate() {
        let mut scene = crate::layout::layout(d);
        crate::route::route(&mut scene);
        fit(&mut scene);
        let h = get(&scene, "size")
            .and_then(|s| get(s, "h"))
            .and_then(edn::record::num)
            .unwrap();
        let prefix = format!("d{}-", i);
        prefix_ids(&mut scene, &prefix);
        translate(&mut scene, 40.0, y + 40.0);
        put(
            &mut scene,
            "title",
            get(d, "title").cloned().unwrap_or(Value::Nil),
        );
        put(
            &mut scene,
            "crap",
            get(d, "crap").cloned().unwrap_or(Value::Nil),
        );
        put(&mut scene, "title-y", geom::numval(y));
        for key in ["classes", "packages", "edges"] {
            match key {
                "classes" => classes.extend(seq(get(&scene, key)).iter().cloned()),
                "packages" => packages.extend(seq(get(&scene, key)).iter().cloned()),
                _ => edges.extend(seq(get(&scene, key)).iter().cloned()),
            }
        }
        sections.push(scene);
        y += 40.0 + h + 64.0;
    }
    let title = get(doc, "title").cloned().unwrap_or(Value::Nil);
    let mut scene = Value::Map(vec![
        (Value::keyword("title"), title.clone()),
        (
            Value::keyword("diagram"),
            Value::Map(vec![(Value::keyword("title"), title)]),
        ),
        (Value::keyword("sections"), Value::Vector(sections)),
        (Value::keyword("classes"), Value::Vector(classes)),
        (Value::keyword("packages"), Value::Vector(packages)),
        (Value::keyword("edges"), Value::Vector(edges)),
    ]);
    let index = ClassIndex::new(&scene);
    let b = bounds(&scene, &index);
    let dx = (40.0 - b.x).max(0.0);
    if dx > 0.0 {
        translate(&mut scene, dx, 0.0);
    }
    size(&mut scene);
    if let Some(Value::Map(entries)) = edn::record::get_raw(&scene, "size").cloned() {
        let mut sz = Value::Map(entries);
        sz.assoc(Value::keyword("h"), geom::numval(y));
        put(&mut scene, "size", sz);
    }
    prepare_scene(&mut scene);
    scene
}
