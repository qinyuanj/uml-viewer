//! Deterministic dependency bundles over compatible segments of the base route.
//! The routed points and prepared strokes remain untouched as the migration
//! oracle; final branches and shared trunks are additive scene geometry.

use crate::compose::ClassIndex;
use crate::geom;
use edn::Value;
use edn::record::{flag, get, local_name, put, seq};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone)]
struct Segment {
    edge: usize,
    part: usize,
    start: f64,
    end: f64,
    offset: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LineKey {
    violating: bool,
    ux: u64,
    uy: u64,
    offset: u64,
}

struct Run {
    start: f64,
    end: f64,
    segments: Vec<usize>,
    members: Vec<usize>,
}

fn compatible(e: &Value) -> Option<bool> {
    if get(e, "kind").and_then(local_name) != Some("dependency") {
        return None;
    }
    Some(flag(e, "violating") || seq(get(e, "deps")).iter().any(|d| flag(d, "violating")))
}

fn line_key(a: [f64; 2], b: [f64; 2], violating: bool) -> Option<(LineKey, f64, f64)> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    if length <= 1e-6 {
        return None;
    }
    let (ux, uy) = (dx / length, dy / length);
    let offset = -uy * a[0] + ux * a[1];
    Some((
        LineKey {
            violating,
            ux: ux.to_bits(),
            uy: uy.to_bits(),
            offset: offset.to_bits(),
        },
        a[0] * ux + a[1] * uy,
        b[0] * ux + b[1] * uy,
    ))
}

fn runs(segments: &[Segment]) -> Vec<Run> {
    enum Event {
        Start(usize),
        End(usize),
    }
    let mut events: Vec<(f64, Event)> = segments
        .iter()
        .enumerate()
        .flat_map(|(i, s)| [(s.start, Event::Start(i)), (s.end, Event::End(i))])
        .collect();
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut active = BTreeSet::new();
    let mut out: Vec<Run> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let at = events[i].0;
        while i < events.len() && events[i].0 == at {
            match events[i].1 {
                Event::Start(s) => active.insert(s),
                Event::End(s) => active.remove(&s),
            };
            i += 1;
        }
        let Some(end) = events.get(i).map(|e| e.0).filter(|end| *end > at) else {
            continue;
        };
        let parts: Vec<usize> = active.iter().copied().collect();
        let members: Vec<usize> = parts
            .iter()
            .map(|&s| segments[s].edge)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if members.len() < 2 {
            continue;
        }
        if let Some(last) = out.last_mut()
            && last.end == at
            && last.segments == parts
        {
            last.end = end;
            continue;
        }
        out.push(Run {
            start: at,
            end,
            segments: parts,
            members,
        });
    }
    out
}

fn point(key: &LineKey, t: f64) -> [f64; 2] {
    let (ux, uy) = (f64::from_bits(key.ux), f64::from_bits(key.uy));
    let offset = f64::from_bits(key.offset);
    [ux * t - uy * offset, uy * t + ux * offset]
}

fn bounds(points: &[[f64; 2]]) -> geom::Rect {
    let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
    let (mut x1, mut y1) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in points {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    geom::Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn clip(paths: Vec<Vec<[f64; 2]>>, e: &Value, index: &ClassIndex) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    for path in paths {
        if path.len() < 2 {
            continue;
        }
        let rects = index.obstacles_in(bounds(&path).inflate(13.0), get(e, "from"), get(e, "to"));
        out.extend(geom::gap_polyline(&path, &rects, 5.0));
    }
    out
}

fn point_value(p: [f64; 2]) -> Value {
    geom::point(p)
}

fn paths_value(paths: &[Vec<[f64; 2]>]) -> Value {
    Value::Vector(
        paths
            .iter()
            .map(|path| Value::Vector(path.iter().map(|p| point_value(*p)).collect()))
            .collect(),
    )
}

fn add_piece(current: &mut Vec<[f64; 2]>, a: [f64; 2], b: [f64; 2]) {
    if a == b {
        return;
    }
    if current.last() != Some(&a) {
        current.push(a);
    }
    current.push(b);
}

type Cut = (f64, f64, [f64; 2], [f64; 2]);

fn branches(e: &Value, cuts: &BTreeMap<usize, Vec<Cut>>) -> Vec<Vec<[f64; 2]>> {
    let points: Vec<[f64; 2]> = seq(get(e, "points"))
        .iter()
        .filter_map(geom::vec2)
        .collect();
    let mut out = Vec::new();
    let mut current = Vec::new();
    for (part, pair) in points.windows(2).enumerate() {
        let (p, q) = (pair[0], pair[1]);
        let length = (q[0] - p[0]).hypot(q[1] - p[1]);
        if length <= 1e-6 {
            continue;
        }
        let unit = [(q[0] - p[0]) / length, (q[1] - p[1]) / length];
        let at = |t: f64| [p[0] + unit[0] * t, p[1] + unit[1] * t];
        let mut cursor = 0.0;
        for &(start, end, entry, exit) in cuts.get(&part).into_iter().flatten() {
            let (start, end) = (start.max(0.0).min(length), end.max(0.0).min(length));
            if end <= cursor || end <= start {
                continue;
            }
            if start > cursor {
                add_piece(&mut current, at(cursor), at(start));
            }
            add_piece(&mut current, at(start), entry);
            if current.len() > 1 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            add_piece(&mut current, exit, at(end));
            cursor = end;
        }
        if cursor < length {
            add_piece(&mut current, at(cursor), q);
        }
    }
    if current.len() > 1 {
        out.push(current);
    }
    out
}

// Spatially bound safety queries to the same local channels as planning.
struct RouteSegment {
    edge: usize,
    a: [f64; 2],
    b: [f64; 2],
}

struct Safety {
    obstacles: HashMap<(i64, i64), Vec<geom::Rect>>,
    routes: HashMap<(i64, i64), Vec<RouteSegment>>,
}

fn cells(rect: geom::Rect) -> impl Iterator<Item = (i64, i64)> {
    let x0 = (rect.x / 256.0).floor() as i64;
    let x1 = (rect.right() / 256.0).floor() as i64;
    let y0 = (rect.y / 256.0).floor() as i64;
    let y1 = (rect.bottom() / 256.0).floor() as i64;
    (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
}

impl Safety {
    fn new(scene: &Value) -> Self {
        let mut out = Self {
            obstacles: HashMap::new(),
            routes: HashMap::new(),
        };
        let mut rects = Vec::new();
        for c in seq(get(scene, "classes")) {
            if !flag(c, "dummy?") {
                rects.extend(get(c, "rect").and_then(geom::Rect::from_value));
            }
            for ports in ["in-ports", "out-ports"] {
                rects.extend(
                    seq(get(c, ports))
                        .iter()
                        .filter_map(|p| get(p, "rect").and_then(geom::Rect::from_value)),
                );
            }
        }
        for p in seq(get(scene, "packages")) {
            if let Some(r) = get(p, "rect").and_then(geom::Rect::from_value) {
                rects.push(geom::Rect::new(r.x, r.y, r.w, 18.0));
            }
        }
        for r in rects {
            for cell in cells(r.inflate(5.0)) {
                out.obstacles.entry(cell).or_default().push(r);
            }
        }
        for (i, e) in seq(get(scene, "edges")).iter().enumerate() {
            let points: Vec<_> = seq(get(e, "points"))
                .iter()
                .filter_map(geom::vec2)
                .collect();
            for ps in points.windows(2) {
                for cell in cells(bounds(ps).inflate(1.0)) {
                    out.routes.entry(cell).or_default().push(RouteSegment {
                        edge: i,
                        a: ps[0],
                        b: ps[1],
                    });
                }
            }
        }
        out
    }

    fn clear(&self, paths: &[Vec<[f64; 2]>], members: &[usize], shifted: bool) -> bool {
        for path in paths {
            for ps in path.windows(2) {
                if ps[0] == ps[1] {
                    continue;
                }
                for cell in cells(bounds(ps).inflate(5.0)) {
                    if let Some(rects) = self.obstacles.get(&cell) {
                        let visible = geom::gap_segment(ps[0], ps[1], rects, 5.0);
                        if shifted && visible.is_empty() {
                            continue;
                        }
                        if !shifted && (visible.len() != 1 || visible[0] != [ps[0], ps[1]]) {
                            return false;
                        }
                    }
                    if shifted && let Some(routes) = self.routes.get(&cell) {
                        for route in routes {
                            if !members.contains(&route.edge)
                                && crosses(ps[0], ps[1], route.a, route.b)
                            {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }
}

fn crosses(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    side(a, b, c) * side(a, b, d) < -1e-8 && side(c, d, a) * side(c, d, b) < -1e-8
}

/// Replace compatible collinear runs with one trunk and explicit per-edge
/// branches. Candidates are grouped by line, then swept; no all-edge pairing.
pub(crate) fn plan(scene: &mut Value, index: &ClassIndex) {
    let edges = seq(get(scene, "edges")).to_vec();
    let mut lines: BTreeMap<LineKey, Vec<Segment>> = BTreeMap::new();
    for (edge_index, e) in edges.iter().enumerate() {
        let Some(violating) = compatible(e) else {
            continue;
        };
        let points: Vec<[f64; 2]> = seq(get(e, "points"))
            .iter()
            .filter_map(geom::vec2)
            .collect();
        for (part, pair) in points.windows(2).enumerate() {
            if let Some((key, start, end)) = line_key(pair[0], pair[1], violating) {
                let offset = f64::from_bits(key.offset);
                lines.entry(key).or_default().push(Segment {
                    edge: edge_index,
                    part,
                    start,
                    end,
                    offset,
                });
            }
        }
    }

    let mut bundles = Vec::new();
    let mut cuts: BTreeMap<usize, BTreeMap<usize, Vec<Cut>>> = BTreeMap::new();
    let mut channels = Vec::new();
    let keys: Vec<_> = lines.keys().cloned().collect();
    for (i, left) in keys.iter().enumerate() {
        let (ux, uy) = (f64::from_bits(left.ux), f64::from_bits(left.uy));
        if ux.abs() < 0.995 && uy.abs() < 0.995 {
            continue;
        }
        let pitch = if uy.abs() >= 0.995 { 160.0 } else { 180.0 };
        let mut group = Vec::new();
        for right in &keys[i + 1..] {
            if left.violating != right.violating || left.ux != right.ux || left.uy != right.uy {
                continue;
            }
            if (f64::from_bits(left.offset) - f64::from_bits(right.offset)).abs() > pitch {
                continue;
            }
            let overlap = lines[left].iter().any(|a| {
                lines[right]
                    .iter()
                    .any(|b| a.start < b.end && b.start < a.end)
            });
            if overlap && (f64::from_bits(left.offset) - f64::from_bits(right.offset)).abs() > 1e-6
            {
                if group.is_empty() {
                    group.extend(lines[left].iter().cloned());
                }
                group.extend(lines[right].iter().cloned());
            }
        }
        if group.len() > lines[left].len() {
            channels.push((left.clone(), group));
        }
    }
    let safety = Safety::new(scene);
    let exact_groups: Vec<(LineKey, Vec<Segment>, bool)> = lines
        .iter()
        .map(|(k, v)| {
            let exact: Vec<_> = v
                .iter()
                .filter(|s| (s.offset - f64::from_bits(k.offset)).abs() < 1e-6)
                .cloned()
                .collect();
            (k.clone(), exact, false)
        })
        .collect();
    let pending = channels;
    for (key, segments, shifted_group) in exact_groups {
        for mut run in runs(&segments) {
            let mut key = key.clone();
            let mut offsets: Vec<f64> = run.segments.iter().map(|i| segments[*i].offset).collect();
            offsets.sort_by(f64::total_cmp);
            key.offset = offsets[offsets.len() / 2].to_bits();
            let shifted = shifted_group && offsets.first() != offsets.last();
            if shifted {
                run.start += 1.0;
                run.end -= 1.0;
            }
            if shifted && offsets.iter().all(|o| (o - offsets[0]).abs() < 1e-6) {
                continue;
            }
            if shifted && run.end - run.start < 4.0 {
                continue;
            }
            if run.segments.iter().any(|i| {
                let s = &segments[*i];
                {
                    let local_start = run.start - s.start;
                    let local_end = run.end - s.start;
                    cuts.get(&s.edge)
                        .and_then(|ps| ps.get(&s.part))
                        .is_some_and(|cs| cs.iter().any(|c| c.0 <= local_end && c.1 >= local_start))
                }
            }) {
                continue;
            }
            let path = vec![point(&key, run.start), point(&key, run.end)];
            let all_obstacles = index.obstacles_in(bounds(&path).inflate(13.0), None, None);
            let visible = geom::gap_polyline(&path, &all_obstacles, 5.0);
            if !shifted && (visible.len() != 1 || visible[0] != path) {
                continue;
            }
            if shifted && visible.is_empty() {
                continue;
            }
            let attachments: Vec<Vec<[f64; 2]>> = run
                .segments
                .iter()
                .map(|i| {
                    let s = &segments[*i];
                    let mut original = key.clone();
                    original.offset = s.offset.to_bits();
                    vec![
                        point(&original, run.start),
                        path[0],
                        path[1],
                        point(&original, run.end),
                    ]
                })
                .collect();
            if !safety.clear(&attachments, &run.members, shifted) {
                continue;
            }
            let member_values = run
                .members
                .iter()
                .map(|i| Value::Integer(*i as i64))
                .collect();
            bundles.push(Value::Map(vec![
                (Value::keyword("members"), Value::Vector(member_values)),
                (Value::keyword("strokes"), paths_value(&[path])),
            ]));
            for segment_index in run.segments {
                let segment = &segments[segment_index];
                cuts.entry(segment.edge)
                    .or_default()
                    .entry(segment.part)
                    .or_default()
                    .push((
                        run.start - segment.start,
                        run.end - segment.start,
                        point(&key, run.start),
                        point(&key, run.end),
                    ));
                let cs = cuts
                    .get_mut(&segment.edge)
                    .unwrap()
                    .get_mut(&segment.part)
                    .unwrap();
                cs.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut merged: Vec<Cut> = Vec::new();
                for c in cs.drain(..) {
                    if let Some(last) = merged.last_mut()
                        && c.0 <= last.1
                    {
                        last.1 = last.1.max(c.1);
                        last.3 = c.3;
                    } else {
                        merged.push(c);
                    }
                }
                *cs = merged;
            }
        }
    }
    let near_groups: Vec<_> = pending
        .into_iter()
        .map(|(k, v)| {
            let v: Vec<_> = v
                .into_iter()
                .filter(|s| cuts.get(&s.edge).and_then(|ps| ps.get(&s.part)).is_none())
                .collect();
            (k, v, true)
        })
        .filter(|(_, v, _)| v.len() >= 2)
        .collect();
    for (key, segments, _) in near_groups {
        for mut run in runs(&segments) {
            let mut key = key.clone();
            let mut offsets: Vec<f64> = run.segments.iter().map(|i| segments[*i].offset).collect();
            offsets.sort_by(f64::total_cmp);
            key.offset = offsets[offsets.len() / 2].to_bits();
            let shifted = true;
            if shifted {
                run.start += 1.0;
                run.end -= 1.0;
            }
            if shifted && offsets.iter().all(|o| (o - offsets[0]).abs() < 1e-6) {
                continue;
            }
            if shifted && run.end - run.start < 4.0 {
                continue;
            }
            if run.segments.iter().any(|i| {
                let s = &segments[*i];
                {
                    let local_start = run.start - s.start;
                    let local_end = run.end - s.start;
                    cuts.get(&s.edge)
                        .and_then(|ps| ps.get(&s.part))
                        .is_some_and(|cs| cs.iter().any(|c| c.0 <= local_end && c.1 >= local_start))
                }
            }) {
                continue;
            }
            let path = vec![point(&key, run.start), point(&key, run.end)];
            let all_obstacles = index.obstacles_in(bounds(&path).inflate(13.0), None, None);
            let visible = geom::gap_polyline(&path, &all_obstacles, 5.0);
            if !shifted && (visible.len() != 1 || visible[0] != path) {
                continue;
            }
            if shifted && visible.is_empty() {
                continue;
            }
            let attachments: Vec<Vec<[f64; 2]>> = run
                .segments
                .iter()
                .map(|i| {
                    let s = &segments[*i];
                    let mut original = key.clone();
                    original.offset = s.offset.to_bits();
                    vec![
                        point(&original, run.start),
                        path[0],
                        path[1],
                        point(&original, run.end),
                    ]
                })
                .collect();
            if !safety.clear(&attachments, &run.members, shifted) {
                continue;
            }
            let member_values = run
                .members
                .iter()
                .map(|i| Value::Integer(*i as i64))
                .collect();
            bundles.push(Value::Map(vec![
                (Value::keyword("members"), Value::Vector(member_values)),
                (Value::keyword("strokes"), paths_value(&[path])),
            ]));
            for segment_index in run.segments {
                let segment = &segments[segment_index];
                cuts.entry(segment.edge)
                    .or_default()
                    .entry(segment.part)
                    .or_default()
                    .push((
                        run.start - segment.start,
                        run.end - segment.start,
                        point(&key, run.start),
                        point(&key, run.end),
                    ));
                let cs = cuts
                    .get_mut(&segment.edge)
                    .unwrap()
                    .get_mut(&segment.part)
                    .unwrap();
                cs.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut merged: Vec<Cut> = Vec::new();
                for c in cs.drain(..) {
                    if let Some(last) = merged.last_mut()
                        && c.0 <= last.1
                    {
                        last.1 = last.1.max(c.1);
                        last.3 = c.3;
                    } else {
                        merged.push(c);
                    }
                }
                *cs = merged;
            }
        }
    }
    if bundles.is_empty() {
        return;
    }
    let mut final_edges = edges;
    for (edge_index, mut edge_cuts) in cuts {
        for cs in edge_cuts.values_mut() {
            cs.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        let e = &mut final_edges[edge_index];
        let raw = branches(e, &edge_cuts);
        put(e, "branches", paths_value(&clip(raw, e, index)));
    }
    put(scene, "edges", Value::Vector(final_edges));
    put(scene, "bundles", Value::Vector(bundles));
}
