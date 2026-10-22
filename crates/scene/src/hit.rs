//! Hit testing over a prepared scene (port of `love/uml/engine/hit.lua`).

use crate::geom::{self, Rect};
use edn::Value;

fn seq(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Vector(items)) => items,
        _ => &[],
    }
}

/// `{:from :to :violating}` records of an edge: its collapsed `:deps`, else a
/// single leaf record built from the endpoints.
pub fn deps_of(e: &Value) -> Option<Value> {
    let deps = geom::get(e, "deps");
    if let Some(Value::Vector(items)) = deps
        && !items.is_empty()
    {
        return Some(Value::Vector(items.clone()));
    }
    let (from, to) = (geom::get(e, "from"), geom::get(e, "to"));
    if let (Some(from), Some(to)) = (from, to) {
        return Some(Value::Vector(vec![Value::Map(vec![
            (Value::keyword("from"), from.clone()),
            (Value::keyword("to"), to.clone()),
            (
                Value::keyword("violating"),
                Value::Bool(geom::get(e, "violating") == Some(&Value::Bool(true))),
            ),
        ])]));
    }
    None
}

pub fn class_by_id<'a>(scene: &'a Value, id: &Value) -> Option<&'a Value> {
    seq(geom::get(scene, "classes"))
        .iter()
        .find(|&c| geom::get(c, "id") == Some(id))
        .map(|v| v as _)
}

pub fn package_by_id<'a>(scene: &'a Value, id: &Value) -> Option<&'a Value> {
    seq(geom::get(scene, "packages"))
        .iter()
        .find(|&c| geom::get(c, "id") == Some(id))
        .map(|v| v as _)
}

/// Edges touching the class id, in scene order.
pub fn connected_edges(scene: &Value, id: &Value) -> Vec<Value> {
    seq(geom::get(scene, "edges"))
        .iter()
        .filter(|e| geom::get(e, "from") == Some(id) || geom::get(e, "to") == Some(id))
        .cloned()
        .collect()
}

/// Collapsed dependency triangles above/below each class box.
pub fn dep_indicators(scene: &Value) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let es = seq(geom::get(scene, "edges"));
    for c in seq(geom::get(scene, "classes")) {
        let Some(r) = geom::get(c, "rect").and_then(Rect::from_value) else {
            continue;
        };
        for dir in ["in", "out"] {
            let endpoint = if dir == "in" { "to" } else { "from" };
            let (mut bx, mut leaf): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
            let id = geom::get(c, "id").unwrap();
            for e in es {
                if let Some(Value::Vector(deps)) = deps_of(e) {
                    for dep in &deps {
                        if geom::get(e, endpoint) == Some(id) {
                            bx.push(dep.clone());
                        }
                        if geom::get(dep, endpoint) == Some(id) {
                            leaf.push(dep.clone());
                        }
                    }
                }
            }
            let deps = if !leaf.is_empty() { leaf } else { bx };
            if deps.is_empty() {
                continue;
            }
            let (x, y, b) = (r.cx(), r.y, r.bottom());
            let h = 12.0 * 0.8660254037844386;
            let tri = if dir == "in" {
                vec![[x, y - h], [x - 6.0, y], [x + 6.0, y]]
            } else {
                vec![[x - 6.0, b], [x + 6.0, b], [x, b + h]]
            };
            let violating = deps
                .iter()
                .any(|dep| geom::get(dep, "violating") == Some(&Value::Bool(true)));
            out.push(Value::Map(vec![
                (Value::keyword("id"), id.clone()),
                (Value::keyword("dir"), Value::keyword(dir)),
                (
                    Value::keyword("triangle"),
                    Value::Vector(tri.iter().map(|p| geom::point(*p)).collect()),
                ),
                (Value::keyword("deps"), Value::Vector(deps)),
                (Value::keyword("violating"), Value::Bool(violating)),
            ]));
        }
    }
    Value::Vector(out)
}

fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (Value::keyword(k), v))
            .collect(),
    )
}

/// Topmost dependency indicator at the point, if any.
pub fn indicator_at(scene: &Value, x: f64, y: f64, pad: f64) -> Option<Value> {
    let inds = seq(geom::get(scene, "dep-indicators"));
    for a in inds.iter().rev() {
        let tri: Vec<[f64; 2]> = seq(geom::get(a, "triangle"))
            .iter()
            .filter_map(geom::vec2)
            .collect();
        if tri.len() == 3 && geom::near_triangle([x, y], &[tri[0], tri[1], tri[2]], pad) {
            return Some(map(vec![
                ("kind", Value::keyword("dep")),
                ("dir", geom::get(a, "dir").cloned().unwrap_or(Value::Nil)),
                ("id", geom::get(a, "id").cloned().unwrap_or(Value::Nil)),
                ("deps", geom::get(a, "deps").cloned().unwrap_or(Value::Nil)),
                (
                    "violating",
                    geom::get(a, "violating").cloned().unwrap_or(Value::Nil),
                ),
            ]));
        }
    }
    None
}

/// Topmost edge at the point, tested against its clipped strokes (the hidden
/// gap never hits), else its raw points.
pub fn edge_at(scene: &Value, x: f64, y: f64, pad: f64) -> Option<Value> {
    let es = seq(geom::get(scene, "edges"));
    for e in es.iter().rev() {
        let strokes = seq(geom::get(e, "strokes"));
        let fallback;
        let paths: &[Value] = if !strokes.is_empty() {
            strokes
        } else {
            let ps = geom::get(e, "points");
            fallback = match ps {
                Some(Value::Vector(items)) if items.len() > 1 => {
                    vec![Value::Vector(items.clone())]
                }
                _ => vec![],
            };
            &fallback
        };
        for path in paths {
            let pts: Vec<[f64; 2]> = seq(Some(path)).iter().filter_map(geom::vec2).collect();
            if geom::near_polyline([x, y], &pts, pad) {
                return Some(map(vec![
                    ("kind", Value::keyword("edge")),
                    ("from", geom::get(e, "from").cloned().unwrap_or(Value::Nil)),
                    ("to", geom::get(e, "to").cloned().unwrap_or(Value::Nil)),
                    ("deps", deps_of(e).unwrap_or(Value::Vector(vec![]))),
                ]));
            }
        }
    }
    None
}

/// Full hit test: indicators, ports, child rows, class boxes, edges, then
/// package frames; later elements sit on top.
pub fn at(scene: &Value, x: f64, y: f64) -> Option<Value> {
    if let Some(ind) = indicator_at(scene, x, y, 5.0) {
        return Some(ind);
    }
    let cs = seq(geom::get(scene, "classes"));
    let visible = cs.iter().any(|c| geom::get(c, "dummy?").is_none());
    for c in cs.iter().rev() {
        if visible && geom::get(c, "dummy?").is_some() {
            continue;
        }
        for dir in ["in", "out"] {
            for p in seq(geom::get(c, &format!("{dir}-ports"))) {
                let hit = geom::get(p, "rect")
                    .and_then(Rect::from_value)
                    .is_some_and(|r| r.contains(x, y));
                if hit {
                    return Some(map(vec![
                        ("kind", Value::keyword("port")),
                        ("id", geom::get(p, "id").cloned().unwrap_or(Value::Nil)),
                        ("parent", geom::get(c, "id").cloned().unwrap_or(Value::Nil)),
                        ("dir", Value::keyword(dir)),
                    ]));
                }
            }
        }
        if let Some(r) = geom::get(c, "rect").and_then(Rect::from_value)
            && r.contains(x, y)
        {
            let mut ly = r.y + 16.0;
            for line in seq(geom::get(c, "lines")) {
                if geom::name(geom::get(line, "kind").unwrap()) == "rule" {
                    ly += 12.0;
                    continue;
                }
                if y >= ly
                    && y <= ly + 17.0
                    && geom::name(geom::get(line, "kind").unwrap()) == "child"
                {
                    return Some(map(vec![
                        ("kind", Value::keyword("child")),
                        ("id", geom::get(line, "id").cloned().unwrap_or(Value::Nil)),
                        ("parent", geom::get(c, "id").cloned().unwrap_or(Value::Nil)),
                        (
                            "drill?",
                            Value::Bool(geom::get(line, "drill?") == Some(&Value::Bool(true))),
                        ),
                    ]));
                }
                ly += 18.0;
            }
            return Some(map(vec![
                ("kind", Value::keyword("class")),
                ("id", geom::get(c, "id").cloned().unwrap_or(Value::Nil)),
                (
                    "drill?",
                    Value::Bool(geom::get(c, "drill?") == Some(&Value::Bool(true))),
                ),
            ]));
        }
    }
    if let Some(edge) = edge_at(scene, x, y, 8.0) {
        return Some(edge);
    }
    let ps = seq(geom::get(scene, "packages"));
    for p in ps.iter().rev() {
        let hit = geom::get(p, "rect")
            .and_then(Rect::from_value)
            .is_some_and(|r| r.contains(x, y));
        if hit {
            return Some(map(vec![
                ("kind", Value::keyword("package")),
                ("id", geom::get(p, "id").cloned().unwrap_or(Value::Nil)),
            ]));
        }
    }
    None
}
