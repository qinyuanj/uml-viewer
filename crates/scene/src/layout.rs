//! Class box layout (port of `love/uml/engine/layout.lua`). Text width is a
//! pure estimate — `char_w = 8` per half-width character, CJK and astral
//! characters count as two — so layout never depends on a loaded font.

use crate::config;
use crate::geom::{self, Rect, numval};
use edn::Value;
use edn::record::{flag, get, local_name, num, put, seq};

/// Estimated advance of one half-width character at the 16px body font.
pub const CHAR_W: f64 = 8.0;

// East Asian Wide/Fullwidth BMP blocks: CJK, kana, Hangul, full-width forms.
const WIDE: &[(u32, u32)] = &[
    (0x1100, 0x115F),
    (0x2E80, 0x303E),
    (0x3041, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE4F),
    (0xFF00, 0xFF60),
    (0xFFE0, 0xFFE6),
];

fn units(c: char) -> usize {
    if c.len_utf8() == 4 {
        return 2; // one JVM UTF-16 surrogate pair
    }
    if c.len_utf8() == 3 {
        let code = c as u32;
        if WIDE.iter().any(|&(lo, hi)| code >= lo && code <= hi) {
            return 2;
        }
    }
    1
}

/// JVM String.count in UTF-16 units, except full-width CJK counts double.
pub fn text_w(s: Option<&str>) -> f64 {
    let n: usize = s.unwrap_or("").chars().map(units).sum();
    CHAR_W * n as f64
}

fn line(kind: &str, text: Option<Value>, extra: Vec<(Value, Value)>) -> Value {
    let mut l = Value::Map(vec![
        (Value::keyword("kind"), Value::keyword(kind)),
        (Value::keyword("text"), text.unwrap_or(Value::Nil)),
    ]);
    for (k, v) in extra {
        l.assoc(k, v);
    }
    l
}

/// The display lines of a class box: stereotype, name, then members (or
/// drillable children) below rule separators. Private ops are hidden.
pub fn class_lines(c: &Value) -> Vec<Value> {
    let mut lines = Vec::new();
    if let Some(stereotype) = get(c, "stereotype") {
        lines.push(line(
            "stereo",
            Some(Value::String(format!("«{}»", geom::name(stereotype)))),
            vec![],
        ));
    }
    lines.push(line("name", get(c, "name").cloned(), vec![]));
    // Lua `not g(c,'hide-members')`: an explicit `false` (every normalized
    // IR class carries one) still shows members (#148).
    if !flag(c, "hide-members") {
        let kids = seq(get(c, "contents"));
        if !kids.is_empty() {
            lines.push(line("rule", None, vec![]));
            for ch in kids {
                lines.push(line(
                    "child",
                    get(ch, "name").cloned(),
                    vec![
                        (
                            Value::keyword("id"),
                            get(ch, "id").cloned().unwrap_or(Value::Nil),
                        ),
                        (
                            Value::keyword("drill?"),
                            Value::Bool(edn::record::is_true(ch, "drill?")),
                        ),
                    ],
                ));
            }
        } else {
            for kind in ["fields", "ops"] {
                let shown: Vec<&Value> = seq(get(c, kind))
                    .iter()
                    .filter(|x| kind != "ops" || !flag(x, "private"))
                    .collect();
                if !shown.is_empty() {
                    lines.push(line("rule", None, vec![]));
                    for x in shown {
                        lines.push(line(
                            if kind == "fields" { "field" } else { "op" },
                            get(x, "text").cloned(),
                            vec![],
                        ));
                    }
                }
            }
        }
    }
    lines
}

/// Width, height and lines of one class box.
pub fn class_box_size(c: &Value) -> (f64, f64, Vec<Value>) {
    let lines = class_lines(c);
    let (mut w, mut h): (f64, f64) = (120.0, 24.0);
    for l in &lines {
        if local_name(get(l, "kind").unwrap()).unwrap_or_default() == "rule" {
            h += 12.0;
        } else {
            h += 18.0;
            w = w.max(
                24.0 + text_w(get(l, "text").and_then(|t| match t {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })),
            );
        }
    }
    (w, h, lines)
}

fn port_w(d: &Value) -> f64 {
    (44.0_f64).max(
        10.0 + text_w(Some(&match get(d, "name") {
            Some(v @ Value::String(_)) => geom::name(v),
            _ => local_name(get(d, "id").unwrap())
                .unwrap_or_default()
                .to_owned(),
        })),
    )
}

/// Clone a class with its box metrics and display lines (Lua `pack`).
fn pack(c: &Value) -> Value {
    let mut c = c.clone();
    let (w, h, lines) = class_box_size(&c);
    let ins = seq(get(&c, "in-deps"));
    let outs = seq(get(&c, "out-deps"));
    let mut iw: f64 = 0.0;
    for (i, d) in ins.iter().enumerate() {
        iw += port_w(d) + if i > 0 { 6.0 } else { 0.0 };
    }
    let mut ow: f64 = 0.0;
    for d in outs {
        ow = ow.max(port_w(d));
    }
    if outs.len() >= 2 {
        ow += 16.0;
    }
    let isp = if ins.is_empty() { 0.0 } else { 40.0 };
    let osp = if outs.is_empty() {
        0.0
    } else {
        58.0 + (outs.len() as f64 - 1.0) * 28.0
    };
    for (k, v) in [
        ("content-w", w),
        ("content-h", h),
        ("in-space", isp),
        ("out-space", osp),
        ("w", w.max(iw).max(ow)),
        ("h", isp + h + osp),
    ] {
        put(&mut c, k, numval(v));
    }
    put(&mut c, "lines", Value::Vector(lines));
    c
}

/// Dependency port rectangles above and below the class box (Lua `ports`).
fn ports(c: &mut Value) {
    let r = Rect::from_value(get(c, "rect").unwrap()).unwrap();
    let ins = seq(get(c, "in-deps")).to_vec();
    let outs = seq(get(c, "out-deps")).to_vec();
    let mut total: f64 = 0.0;
    for (i, d) in ins.iter().enumerate() {
        total += port_w(d) + if i > 0 { 6.0 } else { 0.0 };
    }
    let mut ip = Vec::new();
    let mut x = r.cx() - total / 2.0;
    for d in &ins {
        let mut p = (*d).clone();
        put(
            &mut p,
            "rect",
            Rect::new(x, r.y - 40.0, port_w(d), 18.0).to_value(),
        );
        ip.push(p);
        x += port_w(d) + 6.0;
    }
    let mut widest: f64 = 0.0;
    for d in &outs {
        widest = widest.max(port_w(d));
    }
    x = r.cx() - (widest + if outs.len() >= 2 { 16.0 } else { 0.0 }) / 2.0;
    let mut op = Vec::new();
    for (i, d) in outs.iter().enumerate() {
        let mut p = (*d).clone();
        put(
            &mut p,
            "rect",
            Rect::new(
                x + (i % 2) as f64 * 16.0,
                r.bottom() + 40.0 + i as f64 * 28.0,
                port_w(d),
                18.0,
            )
            .to_value(),
        );
        op.push(p);
    }
    put(c, "in-ports", Value::Vector(ip));
    put(c, "out-ports", Value::Vector(op));
}

/// Gap between two neighbouring lanes, and between a lane and a box.
const LANE_GAP: f64 = 12.0;
const LANE_MARGIN: f64 = 20.0;
/// A lane runs this far into the gaps on both sides of its rank, so the
/// rounded bend onto it (24px corners, `curve::basis_path`) stays clear of
/// the rank's boxes.
const LANE_REACH: f64 = 30.0;

/// The through-points of one long edge: an entry and an exit per rank it
/// crosses, ordered from its `:from` end.
type Lane = (usize, Vec<[f64; 2]>);
type Pass = (usize, [f64; 2], [f64; 2]);

/// Rank classes by dependency depth and place them in rank columns/rows
/// (Lua `place`). Positions are multiples of 80 with barycentric sweeps.
/// An edge spanning more than one rank gets a zero-width lane in every rank
/// between its ends; the lanes are ordered with the boxes, so the edge
/// passes each rank in a reserved gap instead of across a box.
fn place(classes: &[Value], edges: &[Value], direction: &str) -> (Vec<Value>, Vec<Lane>) {
    use std::collections::HashMap;
    let mut cs: Vec<Value> = Vec::with_capacity(classes.len());
    let mut ids: Vec<String> = Vec::with_capacity(classes.len());
    let mut idx: HashMap<String, usize> = HashMap::new();
    let mut out_edges: HashMap<String, Vec<String>> = HashMap::new();
    let mut nbr: HashMap<String, Vec<String>> = HashMap::new();
    let mut high: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, c) in classes.iter().enumerate() {
        let packed = pack(c);
        let id = edn::encode(get(&packed, "id").unwrap()).unwrap();
        idx.insert(id.clone(), i);
        ids.push(id.clone());
        out_edges.insert(id.clone(), Vec::new());
        nbr.insert(id.clone(), Vec::new());
        cs.push(packed);
    }
    for e in edges {
        let f = edn::encode(get(e, "from").unwrap()).unwrap();
        let t = edn::encode(get(e, "to").unwrap()).unwrap();
        if !idx.contains_key(&f) || !idx.contains_key(&t) {
            continue;
        }
        if !nbr[&f].contains(&t) {
            nbr.get_mut(&f).unwrap().push(t.clone());
        }
        if !nbr[&t].contains(&f) {
            nbr.get_mut(&t).unwrap().push(f.clone());
        }
        let k = get(e, "kind")
            .and_then(local_name)
            .unwrap_or_default()
            .to_owned();
        let (f, t) = if k == "inheritance" || k == "implements" {
            (t, f)
        } else {
            (f, t)
        };
        out_edges.get_mut(&f).unwrap().push(t.clone());
        high.insert(t);
    }
    let mut queue: Vec<String> = Vec::new();
    let mut ranks: HashMap<String, f64> = HashMap::new();
    for id in &ids {
        if !high.contains(id) {
            queue.push(id.clone());
            ranks.insert(id.clone(), 0.0);
        }
    }
    if queue.is_empty() {
        for id in &ids {
            queue.push(id.clone());
            ranks.insert(id.clone(), 0.0);
        }
    }
    let mut qi = 0;
    while qi < queue.len() {
        let id = queue[qi].clone();
        qi += 1;
        for t in out_edges[&id].clone() {
            if !ranks.contains_key(&t) {
                ranks.insert(t.clone(), ranks[&id] + 1.0);
                queue.push(t);
            }
        }
    }
    let horizontal = direction == "lr" || direction == "rl";
    let mut levels = false;
    for c in &cs {
        if get(c, "level").is_some() {
            levels = true;
        }
    }
    let levels = levels && !horizontal;
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let mut keys: Vec<f64> = Vec::new();
    let mut pos: HashMap<String, f64> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        let r = if levels {
            get(&cs[idx[id]], "level").and_then(num).unwrap_or(0.0)
        } else {
            ranks.get(id).copied().unwrap_or(0.0)
        };
        let key = rank_key(r);
        if !groups.contains_key(&key) {
            groups.insert(key.clone(), Vec::new());
            keys.push(r);
        }
        groups.get_mut(&key).unwrap().push(id.clone());
        pos.insert(id.clone(), (i as f64) * 80.0);
    }
    keys.sort_by(|a, b| {
        if levels {
            b.partial_cmp(a).unwrap()
        } else {
            a.partial_cmp(b).unwrap()
        }
    });
    let order: HashMap<String, usize> = keys
        .iter()
        .enumerate()
        .map(|(k, r)| (rank_key(*r), k))
        .collect();
    let rank_of = |id: &str| {
        let c = &cs[idx[id]];
        if levels {
            get(c, "level").and_then(num).unwrap_or(0.0)
        } else {
            ranks.get(id).copied().unwrap_or(0.0)
        }
    };
    // lane id -> (edge index, step from the :from end, :from nearer rank 0?)
    let mut lane_of: HashMap<String, (usize, usize, bool)> = HashMap::new();
    for (ei, e) in edges.iter().enumerate() {
        let f = edn::encode(get(e, "from").unwrap()).unwrap();
        let t = edn::encode(get(e, "to").unwrap()).unwrap();
        if !idx.contains_key(&f) || !idx.contains_key(&t) {
            continue;
        }
        let (kf, kt) = (order[&rank_key(rank_of(&f))], order[&rank_key(rank_of(&t))]);
        if kf.abs_diff(kt) < 2 {
            continue;
        }
        let steps: Vec<usize> = if kf < kt {
            (kf + 1..kt).collect()
        } else {
            (kt + 1..kf).rev().collect()
        };
        let chain: Vec<String> = (0..steps.len()).map(|j| format!("lane:{ei}:{j}")).collect();
        for (j, (k, id)) in steps.iter().zip(&chain).enumerate() {
            let prev = if j == 0 {
                f.clone()
            } else {
                chain[j - 1].clone()
            };
            let next = chain.get(j + 1).cloned().unwrap_or_else(|| t.clone());
            nbr.insert(id.clone(), vec![prev, next]);
            groups
                .get_mut(&rank_key(keys[*k]))
                .unwrap()
                .push(id.clone());
            pos.insert(id.clone(), pos[&f]);
            lane_of.insert(id.clone(), (ei, j, kf < kt));
        }
    }
    let gap = |a: bool, b: bool| {
        if a && b {
            LANE_GAP
        } else if a || b {
            LANE_MARGIN
        } else {
            40.0
        }
    };
    let ordered = |xs: &[String], pos: &HashMap<String, f64>| -> Vec<String> {
        let score = |id: &str| -> f64 {
            let mut sum = 0.0;
            let mut n = 0.0;
            for t in &nbr[id] {
                if let Some(p) = pos.get(t) {
                    sum += p;
                    n += 1.0;
                }
            }
            if n > 0.0 {
                sum / n
            } else {
                pos.get(id).copied().unwrap_or(0.0)
            }
        };
        let mut list: Vec<(String, usize, f64)> = xs
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i + 1, score(id)))
            .collect();
        list.sort_by(|a, b| {
            if a.2 == b.2 {
                a.1.cmp(&b.1)
            } else {
                a.2.partial_cmp(&b.2).unwrap()
            }
        });
        list.into_iter().map(|x| x.0).collect()
    };
    for _ in 0..5 {
        for r in &keys {
            for (i, id) in ordered(&groups[&rank_key(*r)], &pos).iter().enumerate() {
                pos.insert(id.clone(), (i as f64) * 80.0);
            }
        }
    }
    let mut gs: Vec<(f64, Vec<String>, f64, f64, f64)> = Vec::new();
    let mut total_h: f64 = 0.0;
    for r in &keys {
        let xs = ordered(&groups[&rank_key(*r)], &pos);
        let (mut w, mut h, mut colh): (f64, f64, f64) = (0.0, 0.0, 0.0);
        for (i, id) in xs.iter().enumerate() {
            let lane = lane_of.contains_key(id);
            if i > 0 {
                colh += gap(lane_of.contains_key(&xs[i - 1]), lane);
            }
            if lane {
                continue;
            }
            let c = &cs[idx[id]];
            w = w.max(num(get(c, "w").unwrap()).unwrap());
            h = h.max(num(get(c, "h").unwrap()).unwrap());
            colh += num(get(c, "h").unwrap()).unwrap();
        }
        gs.push((*r, xs, w, h, colh));
        total_h = total_h.max(colh);
    }
    let mut result = Vec::new();
    // edge index -> (step, entry, exit) of each rank it passes
    let mut through: HashMap<usize, Vec<Pass>> = HashMap::new();
    let mut along = 0.0;
    for (rank, group_ids, gw, gh, colh) in &gs {
        let mut across = if horizontal {
            (total_h - colh) / 2.0
        } else {
            0.0
        };
        let depth = if horizontal { *gw } else { *gh };
        for (i, id) in group_ids.iter().enumerate() {
            if i > 0 {
                across += gap(
                    lane_of.contains_key(&group_ids[i - 1]),
                    lane_of.contains_key(id),
                );
            }
            if let Some(&(ei, j, forward)) = lane_of.get(id) {
                let at = |d: f64| if horizontal { [d, across] } else { [across, d] };
                let (near, far) = (at(along - LANE_REACH), at(along + depth + LANE_REACH));
                let (enter, exit) = if forward { (near, far) } else { (far, near) };
                through.entry(ei).or_default().push((j, enter, exit));
                continue;
            }
            let mut c = cs[idx[id]].clone();
            let (cw, ch, content_w, content_h, in_space) = (
                num(get(&c, "w").unwrap()).unwrap(),
                num(get(&c, "h").unwrap()).unwrap(),
                num(get(&c, "content-w").unwrap()).unwrap(),
                num(get(&c, "content-h").unwrap()).unwrap(),
                num(get(&c, "in-space").unwrap()).unwrap(),
            );
            let (x, y) = if horizontal {
                (along, across)
            } else {
                (across, along)
            };
            put(
                &mut c,
                "rect",
                Rect::new(
                    x + (cw - content_w) / 2.0,
                    y + in_space,
                    content_w,
                    content_h,
                )
                .to_value(),
            );
            put(&mut c, "rank", numval(*rank));
            result.push(c);
            across += if horizontal { ch } else { cw };
        }
        along += depth + 140.0;
    }
    let mut lanes: Vec<Lane> = through
        .into_iter()
        .map(|(ei, mut steps)| {
            steps.sort_by_key(|s| s.0);
            (ei, steps.into_iter().flat_map(|s| [s.1, s.2]).collect())
        })
        .collect();
    lanes.sort_by_key(|l| l.0);
    (result, lanes)
}

/// Ranks are integral in practice; Lua keyed hop groups by string
/// concatenation, so any total order preserving distinct values works.
fn rank_key(r: f64) -> String {
    format!("{r:?}")
}

/// `L.layout`: package rows with pooled quality, then foreign ovals to the
/// right of the package bounding box. `:lanes` carries each long edge's
/// through-points (by diagram edge index) to `route`, which consumes it.
pub fn layout(diagram: &Value) -> Value {
    let mut pkgs: Vec<Value> = Vec::new();
    let mut classes: Vec<Value> = Vec::new();
    let mut lanes: Vec<Value> = Vec::new();
    let mut y = 40.0;
    let mut stride: f64 = 2.0;
    for p in seq(get(diagram, "packages")) {
        stride = stride.max(1.0 + seq(get(p, "classes")).len() as f64);
    }
    for (i, p) in seq(get(diagram, "packages")).iter().enumerate() {
        let (inner, through) = place(
            seq(get(p, "classes")),
            seq(get(diagram, "edges")),
            &get(diagram, "direction")
                .map(geom::name)
                .unwrap_or_else(|| "tb".into()),
        );
        for (ei, points) in through {
            let points = points
                .iter()
                .map(|q| geom::point([q[0] + 52.0, q[1] + y + 44.0]))
                .collect();
            lanes.push(edn::record::record(vec![
                ("edge", numval(ei as f64)),
                ("points", Value::Vector(points)),
            ]));
        }
        let mut rects: Vec<Rect> = Vec::new();
        let mut real: Vec<Value> = Vec::new();
        let mut inner = inner;
        for c in inner.iter_mut() {
            let moved = Rect::from_value(get(c, "rect").unwrap())
                .unwrap()
                .translate(52.0, y + 44.0);
            put(c, "rect", moved.to_value());
            put(c, "package", get(p, "id").cloned().unwrap_or(Value::Nil));
            let rank = num(get(c, "rank").unwrap()).unwrap();
            put(c, "rank", numval(i as f64 * stride + rank));
            ports(c);
            if !flag(c, "dummy?") {
                real.push(c.clone());
                rects.push(Rect::from_value(get(c, "rect").unwrap()).unwrap());
                for k in ["in-ports", "out-ports"] {
                    for port in seq(get(c, k)) {
                        rects.push(Rect::from_value(get(port, "rect").unwrap()).unwrap());
                    }
                }
            }
        }
        let body = union_all(&rects).unwrap_or(Rect::new(52.0, y + 44.0, 160.0, 40.0));
        let pr = Rect::new(
            40.0,
            y,
            (body.right() + 12.0 - 40.0)
                .max(
                    24.0 + text_w(get(p, "label").and_then(|l| match l {
                        Value::String(s) => Some(s.as_str()),
                        _ => None,
                    })),
                )
                .max(180.0),
            body.bottom() + 12.0 - y,
        );
        let mut pkg = Value::Map(vec![
            (
                Value::keyword("id"),
                get(p, "id").cloned().unwrap_or(Value::Nil),
            ),
            (
                Value::keyword("label"),
                get(p, "label").cloned().unwrap_or(Value::Nil),
            ),
            (
                Value::keyword("title"),
                get(p, "label").cloned().unwrap_or(Value::Nil),
            ),
            (Value::keyword("rect"), pr.to_value()),
        ]);
        let source = if !real.is_empty() { &real } else { &inner };
        if let Some(crap) = config::pool_crap(source, true) {
            put(&mut pkg, "crap", crap);
        }
        if let Some(Value::Map(entries)) = config::pool_mutants(source) {
            for (k, v) in entries {
                pkg.assoc(k, v);
            }
        }
        pkgs.push(pkg);
        for c in inner.iter_mut() {
            if flag(c, "dummy?") {
                put(c, "rect", pr.to_value());
                ports(c);
            }
            classes.push(c.clone());
        }
        y += pr.h + 80.0;
    }
    let rs: Vec<Rect> = pkgs
        .iter()
        .map(|p| Rect::from_value(get(p, "rect").unwrap()).unwrap())
        .collect();
    let bounds = union_all(&rs).unwrap_or(Rect::new(40.0, 40.0, 400.0, 300.0));
    let mut ovals: Vec<Value> = Vec::new();
    let mut oy = bounds.y;
    for c0 in seq(get(diagram, "foreign")) {
        let mut c = c0.clone();
        let w = (80.0_f64).max(
            24.0 + text_w(get(&c, "name").and_then(|n| match n {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })),
        );
        let h = (44.0_f64).max((0.55 * w).floor());
        put(&mut c, "shape", Value::keyword("oval"));
        let name_line = line("name", get(&c, "name").cloned(), vec![]);
        put(&mut c, "lines", Value::Vector(vec![name_line]));
        put(
            &mut c,
            "rect",
            Rect::new(bounds.right() + 40.0, oy, w, h).to_value(),
        );
        ovals.push(c);
        oy += h + 40.0;
    }
    let oh = if ovals.is_empty() {
        0.0
    } else {
        oy - 40.0 - bounds.y
    };
    let dy = if ovals.is_empty() {
        0.0
    } else {
        ((bounds.h - oh) / 2.0).max(0.0)
    };
    for c in ovals.iter_mut() {
        let moved = Rect::from_value(get(c, "rect").unwrap())
            .unwrap()
            .translate(0.0, dy);
        put(c, "rect", moved.to_value());
        classes.push(c.clone());
    }
    Value::Map(vec![
        (Value::keyword("diagram"), diagram.clone()),
        (Value::keyword("packages"), Value::Vector(pkgs)),
        (Value::keyword("classes"), Value::Vector(classes)),
        (Value::keyword("edges"), Value::Vector(vec![])),
        (Value::keyword("lanes"), Value::Vector(lanes)),
    ])
}

fn union_all(rects: &[Rect]) -> Option<Rect> {
    geom::union(&rects.iter().map(|r| Some(*r)).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests;
