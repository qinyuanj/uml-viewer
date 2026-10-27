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
                        if visible.len() != 1 || visible[0] != [ps[0], ps[1]] {
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

    let safety = Safety::new(scene);
    let mut bundles = Vec::new();
    let mut cuts: Cuts = BTreeMap::new();
    for (key, segments) in &lines {
        let exact: Vec<_> = segments
            .iter()
            .filter(|s| (s.offset - f64::from_bits(key.offset)).abs() < 1e-6)
            .cloned()
            .collect();
        sweep(key, &exact, false, index, &safety, &mut bundles, &mut cuts);
    }
    let ranked = ranked_edges(scene, &edges);
    for (key, group) in channels(&lines) {
        // Segments already split by an exact trunk keep that sharing only;
        // foreign or unranked endpoints never move onto a nearby channel.
        let free: Vec<_> = group
            .into_iter()
            .filter(|s| ranked[s.edge])
            .filter(|s| cuts.get(&s.edge).and_then(|ps| ps.get(&s.part)).is_none())
            .collect();
        sweep(&key, &free, true, index, &safety, &mut bundles, &mut cuts);
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

/// Edges whose two endpoints carry a declared architecture level and are not
/// foreign stubs: only these may be grouped onto a nearby channel.
fn ranked_edges(scene: &Value, edges: &[Value]) -> Vec<bool> {
    let levels: HashMap<String, bool> = seq(get(scene, "classes"))
        .iter()
        .filter_map(|c| {
            let id = edn::encode(get(c, "id")?).ok()?;
            let ranked =
                !flag(c, "foreign") && get(c, "level").and_then(edn::record::num).is_some();
            Some((id, ranked))
        })
        .collect();
    let ranked = |id: Option<&Value>| {
        id.and_then(|id| edn::encode(id).ok())
            .and_then(|id| levels.get(&id).copied())
            .unwrap_or(false)
    };
    edges
        .iter()
        .map(|e| ranked(get(e, "from")) && ranked(get(e, "to")))
        .collect()
}

type Cuts = BTreeMap<usize, BTreeMap<usize, Vec<Cut>>>;

/// Most logical edges one trunk may carry.
const MAX_MEMBERS: usize = 12;
/// Widest offset between nearby parallel lines that may move onto one trunk.
const CHANNEL_PITCH: f64 = 48.0;
/// A shifted trunk keeps this far from the run ends, clear of endpoint boxes.
const CHANNEL_INSET: f64 = 12.0;

/// Nearby axis-parallel lines of one direction and style, found by a sorted
/// offset window of bounded width and size; never an all-line pairing.
/// Wider groups come first so a channel gathers all its members at once.
fn channels(lines: &BTreeMap<LineKey, Vec<Segment>>) -> Vec<(LineKey, Vec<Segment>)> {
    let mut families: BTreeMap<(bool, u64, u64), Vec<&LineKey>> = BTreeMap::new();
    for key in lines.keys() {
        let (ux, uy) = (f64::from_bits(key.ux), f64::from_bits(key.uy));
        if ux.abs() >= 0.995 || uy.abs() >= 0.995 {
            families
                .entry((key.violating, key.ux, key.uy))
                .or_default()
                .push(key);
        }
    }
    let mut out = Vec::new();
    for family in families.values_mut() {
        family.sort_by(|a, b| f64::from_bits(a.offset).total_cmp(&f64::from_bits(b.offset)));
        let mut window = 0;
        for (i, left) in family.iter().enumerate() {
            let offset = f64::from_bits(left.offset);
            while offset - f64::from_bits(family[window].offset) > CHANNEL_PITCH {
                window += 1;
            }
            let from = window.max(i.saturating_sub(MAX_MEMBERS - 1));
            let (mut group, mut width) = (Vec::new(), 1);
            for right in &family[from..i] {
                let overlap = lines[*left].iter().any(|a| {
                    lines[*right]
                        .iter()
                        .any(|b| a.start < b.end && b.start < a.end)
                });
                if overlap {
                    group.extend(lines[*right].iter().cloned());
                    width += 1;
                }
            }
            if !group.is_empty() {
                group.extend(lines[*left].iter().cloned());
                out.push((width, (*left).clone(), group));
            }
        }
    }
    // Stable: equal widths keep the deterministic family and offset order.
    out.sort_by_key(|(width, _, _)| std::cmp::Reverse(*width));
    out.into_iter()
        .map(|(_, key, group)| (key, group))
        .collect()
}

/// Adopt every safe run of one candidate group as a trunk.
fn sweep(
    line: &LineKey,
    segments: &[Segment],
    shifted_group: bool,
    index: &ClassIndex,
    safety: &Safety,
    bundles: &mut Vec<Value>,
    cuts: &mut Cuts,
) {
    for mut run in runs(segments) {
        if shifted_group && run.members.len() > MAX_MEMBERS {
            continue;
        }
        let mut key = line.clone();
        let mut offsets: Vec<f64> = run.segments.iter().map(|i| segments[*i].offset).collect();
        offsets.sort_by(f64::total_cmp);
        key.offset = offsets[offsets.len() / 2].to_bits();
        let shifted = offsets.first() != offsets.last();
        if shifted_group != shifted {
            continue;
        }
        if shifted {
            run.start += CHANNEL_INSET;
            run.end -= CHANNEL_INSET;
            if run.end - run.start < CHANNEL_INSET {
                continue;
            }
        }
        // Runs that only touch an existing cut at an end still share.
        let taken = run.segments.iter().any(|i| {
            let s = &segments[*i];
            let (from, to) = (run.start - s.start, run.end - s.start);
            cuts.get(&s.edge)
                .and_then(|ps| ps.get(&s.part))
                .is_some_and(|cs| cs.iter().any(|c| c.0 < to - 1e-6 && c.1 > from + 1e-6))
        });
        if taken {
            continue;
        }
        let path = vec![point(&key, run.start), point(&key, run.end)];
        let all_obstacles = index.obstacles_in(bounds(&path).inflate(13.0), None, None);
        let visible = geom::gap_polyline(&path, &all_obstacles, 5.0);
        if visible.len() != 1 || visible[0] != path {
            continue;
        }
        let attachments: Vec<Vec<[f64; 2]>> = run
            .segments
            .iter()
            .map(|i| {
                let mut original = key.clone();
                original.offset = segments[*i].offset.to_bits();
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
            (
                Value::keyword("strokes"),
                paths_value(std::slice::from_ref(&path)),
            ),
        ]));
        for segment_index in run.segments {
            let segment = &segments[segment_index];
            let cs = cuts
                .entry(segment.edge)
                .or_default()
                .entry(segment.part)
                .or_default();
            cs.push((
                run.start - segment.start,
                run.end - segment.start,
                path[0],
                path[1],
            ));
            cs.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Fixed geometry through `compose::prepare_scene`: each rejection adds
    //! one obstacle, crossing or endpoint kind to a scene that bundles.
    use edn::Value;
    use edn::record::{get, seq};

    fn prepared(text: &str) -> Value {
        let mut scene = edn::parse(text).unwrap();
        crate::compose::prepare_scene(&mut scene);
        scene
    }

    fn members(scene: &Value) -> Vec<usize> {
        seq(get(scene, "bundles"))
            .iter()
            .map(|b| seq(get(b, "members")).len())
            .collect()
    }

    /// Two ranked dependencies on parallel lines 20 apart, endpoint boxes
    /// clear of the channel; `extra` adds classes, packages or edges.
    fn channel(t1: &str, extra_classes: &str, packages: &str, extra_edges: &str) -> String {
        format!(
            "{{:classes [{{:id :s0 :level 1 :rect {{:x 70 :y -60 :w 40 :h 40}}}}
                       {{:id :s1 :level 1 :rect {{:x 130 :y -60 :w 40 :h 40}}}}
                       {{:id :t0 :level 3 :rect {{:x 70 :y 420 :w 40 :h 40}}}}
                       {t1} {extra_classes}]
              :packages [{packages}]
              :edges [{{:from :s0 :to :t0 :kind :dependency :points [[100 0] [100 400]]}}
                      {{:from :s1 :to :t1 :kind :dependency :points [[120 0] [120 400]]}}
                      {extra_edges}]}}"
        )
    }

    const T1: &str = "{:id :t1 :level 3 :rect {:x 130 :y 420 :w 40 :h 40}}";

    #[test]
    fn nearby_ranked_lines_share_one_trunk_and_keep_endpoints_and_leaves() {
        let scene = prepared(&channel(T1, "", "", ""));
        assert_eq!(vec![2], members(&scene));
        let edges = seq(get(&scene, "edges"));
        for (e, (from, to, x)) in edges.iter().zip([("s0", "t0", 100), ("s1", "t1", 120)]) {
            assert_eq!(Some(&Value::keyword(from)), get(e, "from"));
            assert_eq!(Some(&Value::keyword(to)), get(e, "to"));
            assert_eq!(
                Some(&edn::parse(&format!("[[{x} 0] [{x} 400]]")).unwrap()),
                get(e, "points"),
                "base route kept"
            );
            assert!(get(e, "branches").is_some(), "explicit branches");
        }
        let violating = prepared(&channel(T1, "", "", "").replace(
            ":kind :dependency :points",
            ":kind :dependency :violating true :points",
        ));
        assert_eq!(
            vec![2],
            members(&violating),
            "violating pair shares red trunk"
        );
        assert!(
            seq(get(&violating, "edges"))
                .iter()
                .all(|e| get(e, "violating") == Some(&Value::Bool(true))),
            "violation stays on every member"
        );
    }

    #[test]
    fn partially_covering_boxes_titles_and_ports_reject_the_channel() {
        for (name, classes, packages) in [
            (
                "class box",
                "{:id :blocker :level 2 :rect {:x 104 :y 200 :w 30 :h 20}}",
                "",
            ),
            (
                "component title",
                "",
                "{:id :p :rect {:x 104 :y 200 :w 300 :h 120}}",
            ),
            (
                "port",
                "{:id :other :level 2 :rect {:x 600 :y 200 :w 40 :h 40}
                  :out-ports [{:rect {:x 104 :y 200 :w 30 :h 18}}]}",
                "",
            ),
        ] {
            let scene = prepared(&channel(T1, classes, packages, ""));
            assert!(get(&scene, "bundles").is_none(), "{name} rejects sharing");
            assert!(
                seq(get(&scene, "edges"))
                    .iter()
                    .all(|e| get(e, "branches").is_none()),
                "{name}: base routes retained"
            );
        }
    }

    #[test]
    fn a_new_crossing_with_another_relation_rejects_the_channel() {
        let crossing = "{:from :n0 :to :n1 :kind :association :points [[110 0] [110 30]]}";
        let scene = prepared(&channel(T1, "", "", crossing));
        assert!(get(&scene, "bundles").is_none());
    }

    #[test]
    fn foreign_or_unranked_endpoints_share_only_an_exactly_common_line() {
        let foreign = "{:id :t1 :foreign true :rect {:x 130 :y 420 :w 40 :h 40}}";
        let unranked = "{:id :t1 :rect {:x 130 :y 420 :w 40 :h 40}}";
        for t1 in [foreign, unranked] {
            let scene = prepared(&channel(t1, "", "", ""));
            assert!(
                get(&scene, "bundles").is_none(),
                "not moved onto a channel: {t1}"
            );
        }
        let common = prepared(
            "{:classes [{:id :s0 :level 1 :rect {:x 300 :y -60 :w 40 :h 40}}
                        {:id :t0 :level 3 :rect {:x 300 :y 420 :w 40 :h 40}}
                        {:id :x :foreign true :rect {:x 300 :y 290 :w 40 :h 40}}]
              :edges [{:from :s0 :to :t0 :kind :dependency :points [[100 0] [100 400]]}
                      {:from :s0 :to :x :kind :dependency :points [[100 0] [100 300]]}]}",
        );
        assert_eq!(vec![2], members(&common), "foreign shares its exact line");
    }

    #[test]
    fn runs_touching_at_one_point_both_share() {
        let scene = prepared(
            "{:classes [{:id :a :level 1 :rect {:x 300 :y 0 :w 40 :h 40}}
                        {:id :b :level 2 :rect {:x 300 :y 200 :w 40 :h 40}}
                        {:id :c :level 3 :rect {:x 300 :y 400 :w 40 :h 40}}]
              :edges [{:from :a :to :b :kind :dependency :points [[100 0] [100 200]]}
                      {:from :a :to :c :kind :dependency :points [[100 0] [100 400]]}
                      {:from :b :to :c :kind :dependency :points [[100 200] [100 400]]}]}",
        );
        assert_eq!(vec![2, 2], members(&scene));
    }

    #[test]
    fn a_dense_parallel_family_forms_trunks_of_bounded_size() {
        let classes: String = (0..20)
            .map(|i| {
                format!(
                    "{{:id :s{i} :level 1 :rect {{:x {} :y -60 :w 2 :h 40}}}}
                     {{:id :t{i} :level 3 :rect {{:x {} :y 420 :w 2 :h 40}}}}",
                    99 + 2 * i,
                    99 + 2 * i
                )
            })
            .collect();
        let edges: String = (0..20)
            .map(|i| {
                let x = 100 + 2 * i;
                format!("{{:from :s{i} :to :t{i} :kind :dependency :points [[{x} 0] [{x} 400]]}}")
            })
            .collect();
        let scene = prepared(&format!("{{:classes [{classes}] :edges [{edges}]}}"));
        let sizes = members(&scene);
        // All 20 lines lie inside one channel pitch; no trunk takes them all.
        assert!(!sizes.is_empty(), "dense family bundles");
        assert!(sizes.iter().all(|n| *n <= super::MAX_MEMBERS), "{sizes:?}");
    }
}
