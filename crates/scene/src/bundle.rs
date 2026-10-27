//! Deterministic dependency bundles over compatible segments of the base route.
//! The routed points and prepared strokes remain untouched as the migration
//! oracle; final branches and shared trunks are additive scene geometry.

use crate::compose::ClassIndex;
use crate::geom;
use edn::Value;
use edn::record::{flag, get, local_name, put, seq};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
struct Segment {
    edge: usize,
    part: usize,
    start: f64,
    end: f64,
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

fn branches(e: &Value, cuts: &BTreeMap<usize, Vec<(f64, f64)>>) -> Vec<Vec<[f64; 2]>> {
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
        for &(start, end) in cuts.get(&part).into_iter().flatten() {
            let (start, end) = (start.max(0.0).min(length), end.max(0.0).min(length));
            if end <= cursor || end <= start {
                continue;
            }
            if start > cursor {
                add_piece(&mut current, at(cursor), at(start));
            }
            if current.len() > 1 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
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
                lines.entry(key).or_default().push(Segment {
                    edge: edge_index,
                    part,
                    start,
                    end,
                });
            }
        }
    }

    let mut bundles = Vec::new();
    let mut cuts: BTreeMap<usize, BTreeMap<usize, Vec<(f64, f64)>>> = BTreeMap::new();
    for (key, segments) in lines {
        for run in runs(&segments) {
            let path = vec![point(&key, run.start), point(&key, run.end)];
            let all_obstacles = index.obstacles_in(bounds(&path).inflate(13.0), None, None);
            let visible = geom::gap_polyline(&path, &all_obstacles, 5.0);
            if visible.len() != 1 || visible[0] != path {
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
                    .push((run.start - segment.start, run.end - segment.start));
            }
        }
    }
    if bundles.is_empty() {
        return;
    }
    let mut final_edges = edges;
    for (edge_index, edge_cuts) in cuts {
        let e = &mut final_edges[edge_index];
        let raw = branches(e, &edge_cuts);
        put(e, "branches", paths_value(&clip(raw, e, index)));
    }
    put(scene, "edges", Value::Vector(final_edges));
    put(scene, "bundles", Value::Vector(bundles));
}
