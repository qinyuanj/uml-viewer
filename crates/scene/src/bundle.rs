//! Deterministic dependency bundles over compatible straight stretches of the
//! base curves. The routed points and prepared strokes remain untouched as the
//! migration oracle; final branches and shared trunks are additive scene
//! geometry. Branches are cut from each member's flattened curve and join the
//! trunk through a smooth bend, so bundling never turns a curve rectangular.

use crate::compose::ClassIndex;
use crate::geom;
use edn::Value;
use edn::record::{flag, get, local_name, put, seq};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Most logical edges one trunk may carry.
const MAX_MEMBERS: usize = 12;
/// Widest offset between nearby parallel lines that may move onto one trunk.
const CHANNEL_PITCH: f64 = 48.0;
/// Lines within about 11° of a heading may share a channel along it.
const NEAR_AXIS: f64 = 0.98;
/// Family headings, 15° apart, so every direction is near one or two.
const HEADINGS: usize = 24;
/// A flattened curve runs along its routed line while it stays this close;
/// the basis curve drifts up to a sixth of the 24 px corner rounding.
const ALONG: f64 = 4.5;
/// Shortest trunk worth drawing.
const MIN_TRUNK: f64 = 32.0;
/// Shortest branch piece; a shorter one reads as a stub or a fold-back.
const MIN_BRANCH: f64 = 24.0;
/// Shortest bend from a member's curve onto its trunk.
const MIN_BEND: f64 = 12.0;
/// Bend length per unit of sideways offset onto the trunk: a gentle taper
/// where the channel has room, never steeper than the shortest.
const BEND_SLOPE: f64 = 3.0;
const STEEPEST_BEND_SLOPE: f64 = 1.0;
/// Samples per flattened bend.
const BEND_STEPS: usize = 12;
/// Clearance kept from boxes, titles and ports, as clipping does.
const PAD: f64 = 5.0;

/// The drawing style a dependency shares with its trunk; relations other than
/// plain dependencies never bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Style {
    violating: bool,
}

fn trunk_style(e: &Value) -> Option<Style> {
    (get(e, "kind").and_then(local_name) == Some("dependency")).then(|| Style {
        violating: crate::hit::edge_violating(e),
    })
}

/// A routed straight line, keyed exactly so collinear parts group together.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Line {
    style: Style,
    ux: u64,
    uy: u64,
    offset: u64,
}

impl Line {
    fn through(a: [f64; 2], b: [f64; 2], style: Style) -> Option<Line> {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dy);
        if length <= 1e-6 {
            return None;
        }
        let (ux, uy) = (dx / length, dy / length);
        Some(Line {
            style,
            ux: ux.to_bits(),
            uy: uy.to_bits(),
            offset: (-uy * a[0] + ux * a[1]).to_bits(),
        })
    }

    fn unit(&self) -> [f64; 2] {
        [f64::from_bits(self.ux), f64::from_bits(self.uy)]
    }

    fn offset(&self) -> f64 {
        f64::from_bits(self.offset)
    }

    fn along(&self, p: [f64; 2]) -> f64 {
        let u = self.unit();
        p[0] * u[0] + p[1] * u[1]
    }

    fn across(&self, p: [f64; 2]) -> f64 {
        let u = self.unit();
        -u[1] * p[0] + u[0] * p[1]
    }

    /// The point `t` along this line.
    fn point(&self, t: f64) -> [f64; 2] {
        let (u, o) = (self.unit(), self.offset());
        [u[0] * t - u[1] * o, u[1] * t + u[0] * o]
    }
}

/// The direction a candidate is measured along: one routed line's own, or
/// a family heading shared by nearly parallel lines.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Axis {
    u: [f64; 2],
}

impl Axis {
    fn along(&self, p: [f64; 2]) -> f64 {
        p[0] * self.u[0] + p[1] * self.u[1]
    }

    fn across(&self, p: [f64; 2]) -> f64 {
        -self.u[1] * p[0] + self.u[0] * p[1]
    }

    /// Family heading `k`; the four axes are exact.
    fn heading(k: usize) -> Axis {
        let angle = std::f64::consts::TAU * k as f64 / HEADINGS as f64;
        let exact = |c: f64| if c.abs() < 1e-12 { 0.0 } else { c };
        Axis {
            u: [exact(angle.cos()), exact(angle.sin())],
        }
    }

    /// The headings a line runs nearly parallel to, in the same direction.
    fn near(line: &Line) -> impl Iterator<Item = usize> {
        let u = line.unit();
        (0..HEADINGS).filter(move |k| {
            let h = Axis::heading(*k).u;
            h[0] * u[0] + h[1] * u[1] >= NEAR_AXIS
        })
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn length(path: &[[f64; 2]]) -> f64 {
    path.windows(2).map(|w| distance(w[0], w[1])).sum()
}

/// A flattened base curve, measured by arc length.
struct Curve {
    points: Vec<[f64; 2]>,
    arcs: Vec<f64>,
}

impl Curve {
    fn new(points: Vec<[f64; 2]>) -> Self {
        let mut arcs = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                total += distance(points[i - 1], *p);
            }
            arcs.push(total);
        }
        Curve { points, arcs }
    }

    fn length(&self) -> f64 {
        self.arcs.last().copied().unwrap_or(0.0)
    }

    fn piece(&self, arc: f64) -> usize {
        self.arcs
            .partition_point(|a| *a <= arc)
            .clamp(1, self.points.len() - 1)
            - 1
    }

    fn at(&self, arc: f64) -> [f64; 2] {
        let i = self.piece(arc);
        let span = self.arcs[i + 1] - self.arcs[i];
        let t = if span <= 1e-12 {
            0.0
        } else {
            ((arc - self.arcs[i]) / span).clamp(0.0, 1.0)
        };
        let (a, b) = (self.points[i], self.points[i + 1]);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    /// Unit direction of travel at an arc position.
    fn direction(&self, arc: f64) -> [f64; 2] {
        let i = self.piece(arc);
        let forward = (i..self.points.len() - 1).chain((0..i).rev());
        for j in forward {
            let (a, b) = (self.points[j], self.points[j + 1]);
            let d = distance(a, b);
            if d > 1e-9 {
                return [(b[0] - a[0]) / d, (b[1] - a[1]) / d];
            }
        }
        [1.0, 0.0]
    }

    /// The curve between two arc positions, ends interpolated.
    fn slice(&self, from: f64, to: f64) -> Vec<[f64; 2]> {
        let mut out = vec![self.at(from)];
        for (p, a) in self.points.iter().zip(&self.arcs) {
            if *a > from && *a < to {
                out.push(*p);
            }
        }
        if to > from {
            out.push(self.at(to));
        }
        out
    }
}

/// Where a member's curve runs straight along one routed line, from the
/// line point `from` to `to`.
struct Stretch {
    edge: usize,
    line: Line,
    first: usize,
    last: usize,
    from: [f64; 2],
    to: [f64; 2],
}

impl Stretch {
    /// The stretch measured along an axis.
    fn span(&self, axis: Axis) -> (f64, f64) {
        (axis.along(self.from), axis.along(self.to))
    }

    /// The line's sideways position at `t` along an axis.
    fn across(&self, axis: Axis, t: f64) -> f64 {
        let ((a, b), (p, q)) = (
            self.span(axis),
            (axis.across(self.from), axis.across(self.to)),
        );
        if b - a <= 1e-9 {
            p
        } else {
            p + (q - p) * (t - a) / (b - a)
        }
    }

    /// Arc position on the curve where it passes `t` along an axis.
    fn arc_at(&self, curve: &Curve, axis: Axis, t: f64) -> f64 {
        let along = |k: usize| axis.along(curve.points[k]);
        let (mut j, mut end) = (self.first, self.last + 1);
        while j < end {
            let mid = (j + end) / 2;
            if along(mid) < t {
                j = mid + 1;
            } else {
                end = mid;
            }
        }
        if j <= self.first {
            return curve.arcs[self.first];
        }
        if j > self.last {
            return curve.arcs[self.last];
        }
        let (a, b) = (along(j - 1), along(j));
        let f = if b - a <= 1e-12 {
            0.0
        } else {
            ((t - a) / (b - a)).clamp(0.0, 1.0)
        };
        curve.arcs[j - 1] + f * (curve.arcs[j] - curve.arcs[j - 1])
    }
}

/// The longest stretch of `curve` along each straight routed part.
fn stretches(edge: usize, points: &[[f64; 2]], curve: &Curve, style: Style) -> Vec<Stretch> {
    let mut out = Vec::new();
    let mut cursor = 0;
    let n = curve.points.len();
    for pair in points.windows(2) {
        let Some(line) = Line::through(pair[0], pair[1], style) else {
            continue;
        };
        let (lo, hi) = (line.along(pair[0]), line.along(pair[1]));
        let near = |p: [f64; 2]| {
            (line.across(p) - line.offset()).abs() <= ALONG
                && (lo - ALONG..=hi + ALONG).contains(&line.along(p))
        };
        let mut best: Option<(usize, usize)> = None;
        let mut k = cursor;
        while k < n {
            if !near(curve.points[k]) {
                k += 1;
                continue;
            }
            let first = k;
            while k + 1 < n
                && near(curve.points[k + 1])
                && line.along(curve.points[k + 1]) >= line.along(curve.points[k]) - 1e-9
            {
                k += 1;
            }
            let extent =
                |(a, b): (usize, usize)| line.along(curve.points[b]) - line.along(curve.points[a]);
            if best.is_none_or(|b| extent((first, k)) > extent(b)) {
                best = Some((first, k));
            }
            k += 1;
        }
        let Some((first, last)) = best else {
            continue;
        };
        cursor = last;
        let start = line.along(curve.points[first]).max(lo);
        let end = line.along(curve.points[last]).min(hi);
        if end - start >= MIN_TRUNK {
            out.push(Stretch {
                edge,
                first,
                last,
                from: line.point(start),
                to: line.point(end),
                line,
            });
        }
    }
    out
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

/// Grid cell keys are small integer pairs; a multiplicative hash is enough
/// and much cheaper than the default SipHash for the crossing index.
#[derive(Default)]
struct CellHasher(u64);

impl std::hash::Hasher for CellHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }

    fn write_i64(&mut self, n: i64) {
        self.write_u64(n as u64);
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

type Grid = HashMap<(i64, i64), Vec<usize>, std::hash::BuildHasherDefault<CellHasher>>;

/// Grid cells, `size` wide, that a rect touches.
fn cells_of(rect: geom::Rect, size: f64) -> impl Iterator<Item = (i64, i64)> {
    let x0 = (rect.x / size).floor() as i64;
    let x1 = (rect.right() / size).floor() as i64;
    let y0 = (rect.y / size).floor() as i64;
    let y1 = (rect.bottom() / size).floor() as i64;
    (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
}

fn cells(rect: geom::Rect) -> impl Iterator<Item = (i64, i64)> {
    cells_of(rect, 256.0)
}

/// Fine grid cells a polyline passes through. Drawn segments are dense along
/// channels and may run long and diagonal, so each segment is walked in short
/// steps instead of covering its bounding box.
fn path_cells(path: &[[f64; 2]]) -> Vec<(i64, i64)> {
    const SIZE: f64 = 128.0;
    let mut out = Vec::new();
    for w in path.windows(2) {
        let steps = (distance(w[0], w[1]) / (SIZE / 2.0)).ceil().max(1.0) as usize;
        let at = |i: usize| {
            let t = i as f64 / steps as f64;
            [
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
            ]
        };
        for i in 0..steps {
            out.extend(cells_of(bounds(&[at(i), at(i + 1)]).inflate(1.0), SIZE));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Half-open crossing test: a polyline passing exactly through a vertex of
/// another still crosses it once.
fn crosses(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let above = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]) > 0.0
    };
    above(a, b, c) != above(a, b, d) && above(c, d, a) != above(c, d, b)
}

fn crossings(path: &Chunked, c: [f64; 2], d: [f64; 2]) -> usize {
    let (x0, x1) = (c[0].min(d[0]), c[0].max(d[0]));
    let (y0, y1) = (c[1].min(d[1]), c[1].max(d[1]));
    let overlaps = |e: &[f64; 4]| e[2] >= x0 && e[0] <= x1 && e[3] >= y0 && e[1] <= y1;
    let mut n = 0;
    for (chunk, extent) in path.extents.iter().enumerate() {
        if !overlaps(extent) {
            continue;
        }
        let first = chunk * CHUNK + 1;
        let last = (first + CHUNK).min(path.points.len());
        for k in first..last {
            let (a, b) = (path.points[k - 1], path.points[k]);
            if overlaps(&[
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ]) && crosses(a, b, c, d)
            {
                n += 1;
            }
        }
    }
    n
}

/// A dense polyline with the extent of every run of `CHUNK` segments, so
/// crossing tests skip far runs without visiting their points.
struct Chunked<'a> {
    points: &'a [[f64; 2]],
    extents: Vec<[f64; 4]>,
}

impl<'a> Chunked<'a> {
    fn new(points: &'a [[f64; 2]]) -> Chunked<'a> {
        let segments = points.len().saturating_sub(1);
        let extents = (0..segments.div_ceil(CHUNK))
            .map(|chunk| {
                let run = &points[chunk * CHUNK..(chunk * CHUNK + CHUNK + 1).min(points.len())];
                let b = bounds(run);
                [b.x, b.y, b.x + b.w, b.y + b.h]
            })
            .collect();
        Chunked { points, extents }
    }
}

/// Indices of the points that keep a polyline within a quarter pixel of
/// itself (Douglas–Peucker); long, nearly straight curve spans collapse.
/// Two polylines end to end, without repeating a shared joint.
fn joined(a: &[[f64; 2]], b: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = a.to_vec();
    for p in b {
        if out.last().is_none_or(|q| distance(*q, *p) > 1e-9) {
            out.push(*p);
        }
    }
    out
}

/// The last `reach` of a polyline's length.
fn trailing(points: &[[f64; 2]], reach: f64) -> Vec<[f64; 2]> {
    let mut out = vec![*points.last().unwrap()];
    let mut left = reach;
    for k in (1..points.len()).rev() {
        let (a, b) = (points[k - 1], points[k]);
        let d = distance(a, b);
        if d >= left {
            let f = left / d.max(1e-12);
            out.push([b[0] + (a[0] - b[0]) * f, b[1] + (a[1] - b[1]) * f]);
            break;
        }
        left -= d;
        out.push(a);
    }
    out.reverse();
    out
}

/// The polyline through its `simplified` points.
fn thinned(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points.to_vec();
    }
    simplified(points).into_iter().map(|k| points[k]).collect()
}

fn simplified(points: &[[f64; 2]]) -> Vec<usize> {
    const TOLERANCE: f64 = 0.25;
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut spans = vec![(0, points.len() - 1)];
    while let Some((a, b)) = spans.pop() {
        let (mut far, mut at) = (0.0, a);
        for (k, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = geom::point_seg_dist(*p, points[a], points[b]);
            if d > far {
                (far, at) = (d, k);
            }
        }
        if far > TOLERANCE {
            keep[at] = true;
            spans.push((a, at));
            spans.push((at, b));
        }
    }
    (0..points.len()).filter(|k| keep[*k]).collect()
}

/// A smooth cubic bend from `p` heading `dp` to `q` heading `dq`.
fn bend(p: [f64; 2], dp: [f64; 2], q: [f64; 2], dq: [f64; 2], reach: f64) -> Vec<[f64; 2]> {
    let h = reach / 2.0;
    let c1 = [p[0] + dp[0] * h, p[1] + dp[1] * h];
    let c2 = [q[0] - dq[0] * h, q[1] - dq[1] * h];
    (0..=BEND_STEPS)
        .map(|i| {
            let t = i as f64 / BEND_STEPS as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [
                a * p[0] + b * c1[0] + c * c2[0] + d * q[0],
                a * p[1] + b * c1[1] + c * c2[1] + d * q[1],
            ]
        })
        .collect()
}

/// Where one member leaves its curve for a trunk and returns to it.
#[derive(Clone)]
struct Cut {
    arc_in: f64,
    arc_out: f64,
    /// From the curve onto the trunk entry; empty when the curve already
    /// runs along the trunk line.
    bend_in: Vec<[f64; 2]>,
    /// From the trunk exit back onto the curve.
    bend_out: Vec<[f64; 2]>,
}

/// Who draws a primitive segment in the final picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Owner {
    Base(usize),
    Bend(usize),
    Trunk(usize),
}

/// A run of drawn segments in the crossing index: a chunk of a base curve,
/// a trunk or a bend. Base chunks keep arc positions so cut parts drop out.
struct Primitive {
    owner: Owner,
    points: Vec<[f64; 2]>,
    arcs: Vec<f64>,
}

/// Base curve segments per indexed chunk.
const CHUNK: usize = 8;

struct Obstacle {
    rect: geom::Rect,
    owner: Option<Value>,
}

enum Rejection {
    Member(usize),
    Before(f64),
    After(f64),
}

struct Planner<'a> {
    edges: &'a [Value],
    curves: Vec<Option<Curve>>,
    stretches: Vec<Stretch>,
    obstacles: Vec<Obstacle>,
    obstacle_cells: Grid,
    primitives: Vec<Primitive>,
    primitive_cells: Grid,
    /// Primitive ids by the object that draws them.
    owned: HashMap<Owner, Vec<usize>>,
    cuts: BTreeMap<usize, Vec<Cut>>,
    bundles: Vec<(Vec<usize>, Vec<[f64; 2]>)>,
    failed: std::cell::RefCell<std::collections::HashSet<Attempt>>,
    /// Primitive extents `[x0 y0 x1 y1]`, compact for the crossing scan.
    extents: Vec<[f64; 4]>,
    /// Visit stamps for the crossing scan: current stamp, per primitive.
    seen: std::cell::RefCell<(u32, Vec<u32>)>,
}

/// A planned trunk: member edges, the trunk path and each member's cut.
type Planned = (Vec<usize>, Vec<[f64; 2]>, Vec<Cut>);

/// Members and trunk limits of one planning state.
type Attempt = (Vec<usize>, u64, u64);

impl<'a> Planner<'a> {
    fn new(scene: &Value, edges: &'a [Value], flats: Vec<Option<Vec<[f64; 2]>>>) -> Self {
        let curves: Vec<Option<Curve>> = flats
            .into_iter()
            .map(|f| f.filter(|f| f.len() >= 2).map(Curve::new))
            .collect();
        let mut planner = Planner {
            edges,
            curves,
            stretches: Vec::new(),
            obstacles: Vec::new(),
            obstacle_cells: Grid::default(),
            primitives: Vec::new(),
            primitive_cells: Grid::default(),
            owned: HashMap::new(),
            cuts: BTreeMap::new(),
            bundles: Vec::new(),
            failed: Default::default(),
            extents: Vec::new(),
            seen: Default::default(),
        };
        for c in seq(get(scene, "classes")) {
            let owner = get(c, "id").cloned();
            if !flag(c, "dummy?")
                && let Some(rect) = get(c, "rect").and_then(geom::Rect::from_value)
            {
                planner.obstacle(rect, owner.clone());
            }
            for ports in ["in-ports", "out-ports"] {
                for p in seq(get(c, ports)) {
                    if let Some(rect) = get(p, "rect").and_then(geom::Rect::from_value) {
                        planner.obstacle(rect, owner.clone());
                    }
                }
            }
        }
        for p in seq(get(scene, "packages")) {
            if let Some(r) = get(p, "rect").and_then(geom::Rect::from_value) {
                planner.obstacle(geom::Rect::new(r.x, r.y, r.w, 18.0), None);
            }
        }
        for edge in 0..edges.len() {
            let Some(curve) = &planner.curves[edge] else {
                continue;
            };
            let kept = simplified(&curve.points);
            let last = kept.len() - 1;
            let chunks: Vec<_> = (0..last)
                .step_by(CHUNK)
                .map(|k| {
                    let ks = &kept[k..=(k + CHUNK).min(last)];
                    (
                        ks.iter().map(|i| curve.points[*i]).collect(),
                        ks.iter().map(|i| curve.arcs[*i]).collect(),
                    )
                })
                .collect();
            for (points, arcs) in chunks {
                planner.primitive(Owner::Base(edge), points, arcs);
            }
        }
        for (edge, e) in edges.iter().enumerate() {
            let (Some(style), Some(curve)) = (trunk_style(e), &planner.curves[edge]) else {
                continue;
            };
            let points: Vec<[f64; 2]> = seq(get(e, "points"))
                .iter()
                .filter_map(geom::vec2)
                .collect();
            let found = stretches(edge, &points, curve, style);
            planner.stretches.extend(found);
        }
        planner
    }

    fn obstacle(&mut self, rect: geom::Rect, owner: Option<Value>) {
        let id = self.obstacles.len();
        self.obstacles.push(Obstacle { rect, owner });
        for cell in cells(rect.inflate(PAD)) {
            self.obstacle_cells.entry(cell).or_default().push(id);
        }
    }

    fn primitive(&mut self, owner: Owner, points: Vec<[f64; 2]>, arcs: Vec<f64>) {
        if points.len() < 2 {
            return;
        }
        let id = self.primitives.len();
        let area = bounds(&points);
        self.extents
            .push([area.x, area.y, area.x + area.w, area.y + area.h]);
        self.primitives.push(Primitive {
            owner,
            points,
            arcs,
        });
        for cell in path_cells(&self.primitives[id].points) {
            self.primitive_cells.entry(cell).or_default().push(id);
        }
        self.owned.entry(owner).or_default().push(id);
    }

    /// Whether a stretch of an edge's curve is replaced by one of its cuts.
    fn cut_away(&self, edge: usize, from: f64, to: f64) -> bool {
        self.cuts.get(&edge).is_some_and(|cs| {
            cs.iter()
                .any(|c| c.arc_in <= from + 1e-9 && to <= c.arc_out + 1e-9)
        })
    }

    /// Obstacles near a path, except the boxes of the given endpoints.
    fn obstacles_near(&self, path: &[[f64; 2]], skip: &[Option<&Value>]) -> Vec<geom::Rect> {
        let ids: BTreeSet<usize> = cells(bounds(path).inflate(PAD))
            .filter_map(|cell| self.obstacle_cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        ids.into_iter()
            .map(|i| &self.obstacles[i])
            .filter(|o| o.owner.is_none() || !skip.contains(&o.owner.as_ref()))
            .map(|o| o.rect)
            .collect()
    }

    fn clear(&self, path: &[[f64; 2]], skip: &[Option<&Value>]) -> bool {
        let rects = self.obstacles_near(path, skip);
        path.len() < 2
            || rects.is_empty()
            || geom::gap_polyline(path, &rects, PAD) == vec![path.to_vec()]
    }

    /// Member stretches that overlap along an axis, one per edge.
    fn shared_sets(&self, axis: Axis, ids: &[usize]) -> Vec<Vec<usize>> {
        let mut events: Vec<(f64, bool, usize)> = ids
            .iter()
            .flat_map(|&i| {
                let (start, end) = self.stretches[i].span(axis);
                [(start, true, i), (end, false, i)]
            })
            .collect();
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut active = BTreeSet::new();
        let mut out = Vec::new();
        let mut i = 0;
        while i < events.len() {
            let at = events[i].0;
            while i < events.len() && events[i].0 == at {
                if events[i].1 {
                    active.insert(events[i].2);
                } else {
                    active.remove(&events[i].2);
                }
                i += 1;
            }
            if events.get(i).is_none_or(|e| e.0 <= at) {
                continue;
            }
            let mut by_edge: BTreeMap<usize, usize> = BTreeMap::new();
            for &s in &active {
                by_edge.entry(self.stretches[s].edge).or_insert(s);
            }
            if by_edge.len() >= 2 {
                let mut set: Vec<usize> = by_edge.into_values().collect();
                set.sort_unstable();
                out.push(set);
            }
        }
        out
    }

    /// Candidate member sets: stretches on one exact line, then ranked
    /// stretches on nearby, nearly parallel lines found by a bounded
    /// sideways window, never an all-line pairing. Larger savings first.
    fn candidates(&self, ranked: &[bool]) -> Vec<(Axis, Vec<usize>)> {
        let mut lines: BTreeMap<&Line, Vec<usize>> = BTreeMap::new();
        for (i, s) in self.stretches.iter().enumerate() {
            lines.entry(&s.line).or_default().push(i);
        }
        let mut found: BTreeMap<Vec<usize>, Axis> = BTreeMap::new();
        for (line, ids) in &lines {
            let axis = Axis { u: line.unit() };
            for set in self.shared_sets(axis, ids) {
                found.entry(set).or_insert(axis);
            }
        }
        let mut families: BTreeMap<(Style, usize), Vec<usize>> = BTreeMap::new();
        for (i, s) in self.stretches.iter().enumerate() {
            if ranked[s.edge] {
                for k in Axis::near(&s.line) {
                    families.entry((s.line.style, k)).or_default().push(i);
                }
            }
        }
        for ((_, k), family) in &mut families {
            let axis = Axis::heading(*k);
            // Lines converging on one box meet only near that end, so the
            // window slides over each end as well as the middle.
            for w in [0.0, 0.5, 1.0] {
                let side = |s: usize| {
                    let st = &self.stretches[s];
                    axis.across([
                        st.from[0] + (st.to[0] - st.from[0]) * w,
                        st.from[1] + (st.to[1] - st.from[1]) * w,
                    ])
                };
                family.sort_by(|a, b| side(*a).total_cmp(&side(*b)).then(a.cmp(b)));
                let mut window = 0;
                let starts: Vec<usize> = (0..family.len())
                    .map(|i| {
                        while side(family[i]) - side(family[window]) > CHANNEL_PITCH {
                            window += 1;
                        }
                        window.max(i.saturating_sub(MAX_MEMBERS - 1))
                    })
                    .collect();
                // Only maximal windows: a window inside the next one adds
                // nothing the planner would not reach by dropping members.
                for (i, &from) in starts.iter().enumerate() {
                    if from == i || starts.get(i + 1).is_some_and(|&next| next == from) {
                        continue;
                    }
                    for set in self.shared_sets(axis, &family[from..=i]) {
                        found.entry(set).or_insert(axis);
                    }
                }
            }
        }
        let mut scored: Vec<(f64, Axis, Vec<usize>)> = found
            .into_iter()
            .map(|(set, axis)| {
                let (a, b) = self.interval(axis, &set);
                ((set.len() - 1) as f64 * (b - a), axis, set)
            })
            .filter(|(saving, _, _)| *saving > 0.0)
            .collect();
        // Stable: equal savings keep the deterministic member order.
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored
            .into_iter()
            .map(|(_, axis, set)| (axis, set))
            .collect()
    }

    fn interval(&self, axis: Axis, members: &[usize]) -> (f64, f64) {
        members
            .iter()
            .map(|s| self.stretches[*s].span(axis))
            .fold((f64::NEG_INFINITY, f64::INFINITY), |(a, b), (c, d)| {
                (a.max(c), b.min(d))
            })
    }

    /// Keep the members closest to the median offset, at most the cap.
    fn capped(&self, axis: Axis, mut members: Vec<usize>) -> Vec<usize> {
        if members.len() <= MAX_MEMBERS {
            return members;
        }
        let (a, b) = self.interval(axis, &members);
        let offset = |s: usize| self.stretches[s].across(axis, (a + b) / 2.0);
        let mut sorted = members.clone();
        sorted.sort_by(|a, b| offset(*a).total_cmp(&offset(*b)).then(a.cmp(b)));
        let median = offset(sorted[sorted.len() / 2]);
        members.sort_by(|a, b| {
            (offset(*a) - median)
                .abs()
                .total_cmp(&(offset(*b) - median).abs())
                .then(a.cmp(b))
        });
        members.truncate(MAX_MEMBERS);
        members.sort_unstable();
        members
    }

    /// A branch piece is absent or long enough to read as a branch.
    fn piece_ok(length: f64) -> bool {
        length <= 1e-6 || length >= MIN_BRANCH
    }

    /// Fit a new cut among the member's existing cuts.
    fn fits(&self, edge: usize, cut: &Cut) -> Result<(), Rejection> {
        let curve = self.curves[edge].as_ref().unwrap();
        let existing = self.cuts.get(&edge).map(Vec::as_slice).unwrap_or(&[]);
        if existing
            .iter()
            .any(|c| c.arc_in < cut.arc_out - 1e-6 && c.arc_out > cut.arc_in + 1e-6)
        {
            return Err(Rejection::Member(edge));
        }
        let before = existing
            .iter()
            .filter(|c| c.arc_out <= cut.arc_in + 1e-6)
            .max_by(|a, b| a.arc_out.total_cmp(&b.arc_out));
        let after = existing
            .iter()
            .filter(|c| c.arc_in >= cut.arc_out - 1e-6)
            .min_by(|a, b| a.arc_in.total_cmp(&b.arc_in));
        let lead = cut.arc_in - before.map_or(0.0, |c| c.arc_out)
            + before.map_or(0.0, |c| length(&c.bend_out))
            + length(&cut.bend_in);
        if !Self::piece_ok(lead) {
            return Err(Rejection::Before(MIN_BRANCH - lead));
        }
        let tail = after.map_or(curve.length(), |c| c.arc_in) - cut.arc_out
            + after.map_or(0.0, |c| length(&c.bend_in))
            + length(&cut.bend_out);
        if !Self::piece_ok(tail) {
            return Err(Rejection::After(MIN_BRANCH - tail));
        }
        // A box the branch passes over gaps it like the base stroke; the
        // piece left between that gap and the trunk must still read as one.
        let e = &self.edges[edge];
        let skip = [get(e, "from"), get(e, "to")];
        let visible = |window: Vec<[f64; 2]>, at_end: bool| {
            let rects = self.obstacles_near(&window, &skip);
            if rects.is_empty() {
                return MIN_BRANCH;
            }
            let pieces = geom::gap_polyline(&window, &rects, PAD);
            let joint = if at_end {
                *window.last().unwrap()
            } else {
                window[0]
            };
            let piece = if at_end {
                pieces.last()
            } else {
                pieces.first()
            };
            piece
                .filter(|q| {
                    let end = if at_end { *q.last().unwrap() } else { q[0] };
                    distance(end, joint) <= 1e-6
                })
                .map_or(0.0, |q| length(q))
        };
        if lead > 1e-6 {
            let from = before.map_or(0.0, |c| c.arc_out);
            let window = joined(
                &curve.slice((cut.arc_in - MIN_BRANCH).max(from), cut.arc_in),
                &cut.bend_in,
            );
            let piece = visible(trailing(&window, MIN_BRANCH), true);
            if piece < MIN_BRANCH - 0.5 {
                return Err(Rejection::Before((MIN_BRANCH - piece).max(1.0)));
            }
        }
        if tail > 1e-6 {
            let to = after.map_or(curve.length(), |c| c.arc_in);
            let window = joined(
                &cut.bend_out,
                &curve.slice(cut.arc_out, (cut.arc_out + MIN_BRANCH).min(to)),
            );
            let mut reversed = window;
            reversed.reverse();
            let mut window = trailing(&reversed, MIN_BRANCH);
            window.reverse();
            let piece = visible(window, false);
            if piece < MIN_BRANCH - 0.5 {
                return Err(Rejection::After((MIN_BRANCH - piece).max(1.0)));
            }
        }
        Ok(())
    }

    /// Live drawn segments of one object within an area: an edge's curve
    /// outside its cuts plus its bends, or a trunk.
    fn drawn(&self, object: Owner, area: geom::Rect) -> Vec<([f64; 2], [f64; 2])> {
        let owners = match object {
            Owner::Base(e) | Owner::Bend(e) => vec![Owner::Base(e), Owner::Bend(e)],
            trunk => vec![trunk],
        };
        let [x0, y0, x1, y1] = [area.x, area.y, area.right(), area.bottom()];
        let mut out = Vec::new();
        for id in owners
            .iter()
            .flat_map(|o| self.owned.get(o).into_iter().flatten())
        {
            let e = self.extents[*id];
            if e[0] > x1 || e[2] < x0 || e[1] > y1 || e[3] < y0 {
                continue;
            }
            let p = &self.primitives[*id];
            for j in 1..p.points.len() {
                let (a, b) = (p.points[j - 1], p.points[j]);
                if !area.overlaps(bounds(&[a, b]).inflate(1e-6)) {
                    continue;
                }
                if let Owner::Base(e) = p.owner
                    && self.cut_away(e, p.arcs[j - 1], p.arcs[j])
                {
                    continue;
                }
                out.push((a, b));
            }
        }
        out
    }

    /// The member whose new geometry adds a crossing with something it did
    /// not cross before, or `Err(())` when the trunk itself adds it.
    fn added_crossing(
        &self,
        edges: &[usize],
        trunk: &[[f64; 2]],
        bends: &[(usize, Vec<[f64; 2]>)],
        replaced: &[Vec<[f64; 2]>],
    ) -> Option<Result<usize, ()>> {
        let involved = |owner: Owner| match owner {
            Owner::Base(e) | Owner::Bend(e) => edges.contains(&e),
            Owner::Trunk(t) => self.bundles[t].0.iter().any(|e| edges.contains(e)),
        };
        // Crossings of a new piece with each live drawn object near it that
        // this candidate does not involve.
        let hits = |path: &[[f64; 2]]| -> Vec<(Owner, usize)> {
            let area = bounds(path).inflate(1e-6);
            let chunked = Chunked::new(path);
            let [x0, y0, x1, y1] = [area.x, area.y, area.x + area.w, area.y + area.h];
            // Cells overlap: visit each primitive once, rejecting by its
            // extent before touching its points.
            let mut ids = Vec::new();
            {
                let mut seen = self.seen.borrow_mut();
                seen.0 += 1;
                let stamp = seen.0;
                seen.1.resize(self.primitives.len(), 0);
                for cell in path_cells(path) {
                    for &id in self.primitive_cells.get(&cell).into_iter().flatten() {
                        let e = self.extents[id];
                        if seen.1[id] != stamp
                            && e[0] <= x1
                            && e[2] >= x0
                            && e[1] <= y1
                            && e[3] >= y0
                        {
                            seen.1[id] = stamp;
                            ids.push(id);
                        }
                    }
                }
            }
            ids.sort_unstable();
            let mut out = Vec::new();
            for id in ids {
                let p = &self.primitives[id];
                if involved(p.owner) {
                    continue;
                }
                let mut n = 0;
                for j in 1..p.points.len() {
                    let (a, b) = (p.points[j - 1], p.points[j]);
                    if !area.overlaps(bounds(&[a, b]).inflate(1e-6)) {
                        continue;
                    }
                    if let Owner::Base(e) = p.owner
                        && self.cut_away(e, p.arcs[j - 1], p.arcs[j])
                    {
                        continue;
                    }
                    n += crossings(&chunked, a, b);
                }
                if n > 0 {
                    let group = match p.owner {
                        Owner::Base(e) | Owner::Bend(e) => Owner::Base(e),
                        trunk => trunk,
                    };
                    out.push((group, n));
                }
            }
            out
        };
        // Old crossings are only needed for objects the new geometry
        // crosses; they are counted once, from that object's live geometry.
        let areas: Vec<geom::Rect> = replaced.iter().map(|r| bounds(r).inflate(1e-6)).collect();
        let replaced: Vec<Chunked> = replaced.iter().map(|r| Chunked::new(r)).collect();
        let union = areas
            .iter()
            .copied()
            .reduce(|a, b| {
                let (x, y) = (a.x.min(b.x), a.y.min(b.y));
                geom::Rect::new(
                    x,
                    y,
                    a.right().max(b.right()) - x,
                    a.bottom().max(b.bottom()) - y,
                )
            })
            .unwrap_or(geom::Rect::new(0.0, 0.0, 0.0, 0.0));
        let removed = |group: Owner| {
            let segments = self.drawn(group, union);
            let mut n = 0;
            for (path, area) in replaced.iter().zip(&areas) {
                for (a, b) in &segments {
                    if area.overlaps(bounds(&[*a, *b]).inflate(1e-6)) {
                        n += crossings(path, *a, *b);
                    }
                }
            }
            n
        };
        // Per drawn object: (added, removed). The trunk is checked first,
        // then each bend; the first excess names its culprit.
        let mut tally = BTreeMap::<Owner, (usize, usize)>::new();
        let mut excess = |group: Owner, n: usize| {
            let entry = tally.entry(group).or_insert_with(|| (0, removed(group)));
            entry.0 += n;
            entry.0 > entry.1
        };
        for (group, n) in hits(trunk) {
            if excess(group, n) {
                return Some(Err(()));
            }
        }
        for (edge, bend) in bends {
            for (group, n) in hits(bend) {
                if excess(group, n) {
                    return Some(Ok(*edge));
                }
            }
        }
        None
    }

    /// Plan one candidate. A conflicting member is dropped and the rest
    /// retried; obstacles and short branches shorten the trunk instead.
    /// Overlapping candidates often shrink to the same failing state, so
    /// failures are remembered until the next trunk changes the picture.
    fn attempt(&self, axis: Axis, members: &[usize]) -> Option<Planned> {
        let mut visited = Vec::new();
        let planned = self.try_members(axis, members, &mut visited);
        if planned.is_none() {
            self.failed.borrow_mut().extend(visited);
        }
        planned
    }

    /// Where two stretches stay within one channel pitch of each other
    /// along the axis: their sideways gap is linear in `t`.
    fn channel(&self, axis: Axis, x: usize, y: usize) -> (f64, f64) {
        let (a, b) = self.interval(axis, &[x, y]);
        let gap = |t: f64| self.stretches[y].across(axis, t) - self.stretches[x].across(axis, t);
        let (ga, gb) = (gap(a), gap(b));
        if (gb - ga).abs() <= 1e-9 {
            return if ga.abs() <= CHANNEL_PITCH {
                (a, b)
            } else {
                (b, a)
            };
        }
        let at = |g: f64| a + (g - ga) * (b - a) / (gb - ga);
        let (p, q) = (at(-CHANNEL_PITCH), at(CHANNEL_PITCH));
        (a.max(p.min(q)), b.min(p.max(q)))
    }

    fn try_members(
        &self,
        axis: Axis,
        members: &[usize],
        visited: &mut Vec<Attempt>,
    ) -> Option<Planned> {
        // Members already cut inside this interval keep that sharing only.
        let (a, b) = self.interval(axis, members);
        let free: Vec<usize> = members
            .iter()
            .copied()
            .filter(|s| {
                let stretch = &self.stretches[*s];
                let curve = self.curves[stretch.edge].as_ref().unwrap();
                let (from, to) = (
                    stretch.arc_at(curve, axis, a),
                    stretch.arc_at(curve, axis, b),
                );
                !self
                    .cuts
                    .get(&stretch.edge)
                    .is_some_and(|cs| cs.iter().any(|c| c.arc_in < to && c.arc_out > from))
            })
            .collect();
        let mut members = self.capped(axis, free);
        let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
        let drop = |members: &mut Vec<usize>, edge: usize| {
            members.retain(|s| self.stretches[*s].edge != edge);
        };
        for _ in 0..4 * MAX_MEMBERS {
            if members.len() < 2 {
                return None;
            }
            let state = (members.clone(), lo.to_bits(), hi.to_bits());
            if self.failed.borrow().contains(&state) {
                return None;
            }
            visited.push(state);
            // The trunk is the middle member's own curve; the others bend
            // onto it, farther ones over a longer run.
            let (a, b) = self.interval(axis, &members);
            let mid = (a + b) / 2.0;
            let mut by_offset = members.clone();
            by_offset.sort_by(|x, y| {
                let offset = |s: &usize| self.stretches[*s].across(axis, mid);
                offset(x).total_cmp(&offset(y)).then(x.cmp(y))
            });
            let middle = by_offset[by_offset.len() / 2];
            let carrier = self.curves[self.stretches[middle].edge].as_ref().unwrap();
            let gap = |s: usize, t: f64| {
                (self.stretches[s].across(axis, t) - self.stretches[middle].across(axis, t)).abs()
            };
            // Within the channel pitch of the carrier only.
            let (a, b) = members
                .iter()
                .filter(|s| **s != middle)
                .map(|s| self.channel(axis, middle, *s))
                .fold((a, b), |(a, b), (c, d)| (a.max(c), b.min(d)));
            let delta = |s: usize| gap(s, a).max(gap(s, b));
            let gap = members
                .iter()
                .filter(|s| **s != middle)
                .map(|s| delta(*s) + ALONG)
                .fold(0.0, f64::max);
            // Bends take at most half a short channel.
            let slope = ((b - a) / 4.0 / gap.max(1e-9)).clamp(STEEPEST_BEND_SLOPE, BEND_SLOPE);
            let reach = |gap: f64| {
                if gap <= 0.5 {
                    0.0
                } else {
                    (slope * gap).max(MIN_BEND)
                }
            };
            let widest = reach(gap);
            let (ts, te) = ((a + widest).max(lo), (b - widest).min(hi));
            if te - ts < MIN_TRUNK {
                if lo.is_finite() || hi.is_finite() {
                    return None;
                }
                // Drop whichever end-limiting member frees the longer trunk.
                let span = |s: &usize| {
                    if *s == middle {
                        self.stretches[*s].span(axis)
                    } else {
                        self.channel(axis, middle, *s)
                    }
                };
                let latest = *members
                    .iter()
                    .max_by(|x, y| span(x).0.total_cmp(&span(y).0).then(x.cmp(y)))
                    .unwrap();
                let earliest = *members
                    .iter()
                    .min_by(|x, y| span(x).1.total_cmp(&span(y).1).then(x.cmp(y)))
                    .unwrap();
                let without = |gone: usize| {
                    members
                        .iter()
                        .filter(|s| **s != gone)
                        .map(span)
                        .fold((f64::NEG_INFINITY, f64::INFINITY), |(a, b), (c, d)| {
                            (a.max(c), b.min(d))
                        })
                };
                let without = |gone: usize| {
                    let (a, b) = without(gone);
                    b - a
                };
                let gone = if without(latest) >= without(earliest) {
                    latest
                } else {
                    earliest
                };
                members.retain(|s| *s != gone);
                continue;
            }
            let (enter, leave) = (
                self.stretches[middle].arc_at(carrier, axis, ts),
                self.stretches[middle].arc_at(carrier, axis, te),
            );
            let trunk = carrier.slice(enter, leave);
            let rects = self.obstacles_near(&trunk, &[]);
            let visible = geom::gap_polyline(&trunk, &rects, PAD);
            if visible != vec![trunk.clone()] {
                let longest = visible
                    .iter()
                    .max_by(|x, y| length(x).total_cmp(&length(y)))?;
                lo = axis.along(longest[0]);
                hi = axis.along(*longest.last().unwrap());
                continue;
            }
            let (q_in, q_out) = (trunk[0], *trunk.last().unwrap());
            let (d_in, d_out) = (carrier.direction(enter), carrier.direction(leave));
            let mut cuts = Vec::new();
            let mut rejection = None;
            for &s in &members {
                let stretch = &self.stretches[s];
                let e = &self.edges[stretch.edge];
                let curve = self.curves[stretch.edge].as_ref().unwrap();
                let (on_in, on_out) = (
                    stretch.arc_at(curve, axis, ts),
                    stretch.arc_at(curve, axis, te),
                );
                // A member already on the trunk meets it without a bend.
                let (l_in, l_out) = (
                    reach(distance(curve.at(on_in), q_in)),
                    reach(distance(curve.at(on_out), q_out)),
                );
                let arc_in = if l_in == 0.0 {
                    on_in
                } else {
                    (on_in - l_in).max(0.0)
                };
                let arc_out = if l_out == 0.0 {
                    on_out
                } else {
                    (on_out + l_out).min(curve.length())
                };
                let cut = Cut {
                    arc_in,
                    arc_out,
                    bend_in: if l_in == 0.0 {
                        Vec::new()
                    } else {
                        bend(curve.at(arc_in), curve.direction(arc_in), q_in, d_in, l_in)
                    },
                    bend_out: if l_out == 0.0 {
                        Vec::new()
                    } else {
                        bend(
                            q_out,
                            d_out,
                            curve.at(arc_out),
                            curve.direction(arc_out),
                            l_out,
                        )
                    },
                };
                if let Err(r) = self.fits(stretch.edge, &cut) {
                    rejection = Some(r);
                    break;
                }
                // A bend into a box moves that trunk end inward, clear of it.
                let skip = [get(e, "from"), get(e, "to")];
                if !self.clear(&cut.bend_in, &skip) {
                    rejection = Some(Rejection::Before(l_in.max(MIN_BEND)));
                    break;
                }
                if !self.clear(&cut.bend_out, &skip) {
                    rejection = Some(Rejection::After(l_out.max(MIN_BEND)));
                    break;
                }
                cuts.push(cut);
            }
            match rejection {
                Some(Rejection::Member(edge)) => {
                    drop(&mut members, edge);
                    continue;
                }
                Some(Rejection::Before(need)) => {
                    lo = ts + need + 1e-3;
                    continue;
                }
                Some(Rejection::After(need)) => {
                    hi = te - need - 1e-3;
                    continue;
                }
                None => {}
            }
            let edges: Vec<usize> = members.iter().map(|s| self.stretches[*s].edge).collect();
            let bends: Vec<(usize, Vec<[f64; 2]>)> = edges
                .iter()
                .zip(&cuts)
                .flat_map(|(e, c)| [(*e, thinned(&c.bend_in)), (*e, thinned(&c.bend_out))])
                .filter(|(_, b)| b.len() > 1)
                .collect();
            let replaced: Vec<Vec<[f64; 2]>> = edges
                .iter()
                .zip(&cuts)
                .map(|(e, c)| {
                    thinned(&self.curves[*e].as_ref().unwrap().slice(c.arc_in, c.arc_out))
                })
                .collect();
            match self.added_crossing(&edges, &thinned(&trunk), &bends, &replaced) {
                None => return Some((edges, trunk, cuts)),
                Some(Ok(edge)) => drop(&mut members, edge),
                Some(Err(())) => {
                    // The trunk crosses something a shifted member did not:
                    // keep moving the farthest member off it.
                    let farthest = members
                        .iter()
                        .copied()
                        .filter(|s| delta(*s) > 1e-9)
                        .max_by(|x, y| delta(*x).total_cmp(&delta(*y)).then(y.cmp(x)))?;
                    members.retain(|s| *s != farthest);
                }
            }
        }
        None
    }

    fn accept(&mut self, edges: Vec<usize>, trunk: Vec<[f64; 2]>, cuts: Vec<Cut>) {
        self.failed.borrow_mut().clear();
        let bundle = self.bundles.len();
        self.primitive(Owner::Trunk(bundle), thinned(&trunk), Vec::new());
        for (&edge, cut) in edges.iter().zip(cuts) {
            for b in [&cut.bend_in, &cut.bend_out] {
                self.primitive(Owner::Bend(edge), thinned(b), Vec::new());
            }
            let cs = self.cuts.entry(edge).or_default();
            cs.push(cut);
            cs.sort_by(|a, b| a.arc_in.total_cmp(&b.arc_in));
        }
        let mut members = edges;
        members.sort_unstable();
        self.bundles.push((members, trunk));
    }

    /// The member's final branches: its curve outside every cut, joined to
    /// the trunks by their bends.
    fn branches(&self, edge: usize) -> Vec<Vec<[f64; 2]>> {
        let curve = self.curves[edge].as_ref().unwrap();
        let mut out = Vec::new();
        let mut current: Vec<[f64; 2]> = Vec::new();
        let mut cursor = 0.0;
        let push = |current: &mut Vec<[f64; 2]>, ps: &[[f64; 2]]| {
            for p in ps {
                if current.last().is_none_or(|q| distance(*q, *p) > 1e-9) {
                    current.push(*p);
                }
            }
        };
        for cut in &self.cuts[&edge] {
            push(&mut current, &curve.slice(cursor, cut.arc_in));
            push(&mut current, &cut.bend_in);
            if length(&current) > 1e-6 {
                out.push(std::mem::take(&mut current));
            }
            current = cut.bend_out.clone();
            cursor = cut.arc_out;
        }
        push(&mut current, &curve.slice(cursor, curve.length()));
        if length(&current) > 1e-6 {
            out.push(current);
        }
        out
    }
}

fn clip(paths: Vec<Vec<[f64; 2]>>, e: &Value, index: &ClassIndex) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    for path in paths {
        if path.len() < 2 {
            continue;
        }
        let rects = index.obstacles_in(bounds(&path).inflate(13.0), get(e, "from"), get(e, "to"));
        out.extend(geom::gap_polyline(&path, &rects, PAD));
    }
    out
}

fn paths_value(paths: &[Vec<[f64; 2]>]) -> Value {
    Value::Vector(
        paths
            .iter()
            .map(|path| Value::Vector(path.iter().map(|p| geom::point(*p)).collect()))
            .collect(),
    )
}

/// Replace compatible straight stretches with one trunk and explicit
/// per-edge branches. `flats` are the edges' unclipped flattened curves.
pub(crate) fn plan(scene: &mut Value, index: &ClassIndex, flats: Vec<Option<Vec<[f64; 2]>>>) {
    let (branches, bundles) = {
        let edges = seq(get(scene, "edges"));
        let ranked = ranked_edges(scene, edges);
        let mut planner = Planner::new(scene, edges, flats);
        for (axis, set) in planner.candidates(&ranked) {
            if let Some((members, trunk, cuts)) = planner.attempt(axis, &set) {
                planner.accept(members, trunk, cuts);
            }
        }
        if planner.bundles.is_empty() {
            return;
        }
        let branches: Vec<(usize, Value)> = planner
            .cuts
            .keys()
            .map(|&edge| {
                let raw = planner.branches(edge);
                (edge, paths_value(&clip(raw, &edges[edge], index)))
            })
            .collect();
        let bundles: Vec<Value> = planner
            .bundles
            .iter()
            .map(|(members, trunk)| {
                Value::Map(vec![
                    (
                        Value::keyword("members"),
                        Value::Vector(members.iter().map(|i| Value::Integer(*i as i64)).collect()),
                    ),
                    (
                        Value::keyword("strokes"),
                        paths_value(std::slice::from_ref(trunk)),
                    ),
                ])
            })
            .collect();
        (branches, bundles)
    };
    if let Value::Map(entries) = scene
        && let Some((_, Value::Vector(edges))) = entries
            .iter_mut()
            .find(|(k, _)| *k == Value::keyword("edges"))
    {
        for (edge, value) in branches {
            put(&mut edges[edge], "branches", value);
        }
    }
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

    fn polylines(v: Option<&Value>) -> Vec<Vec<[f64; 2]>> {
        seq(v)
            .iter()
            .map(|p| seq(Some(p)).iter().filter_map(crate::geom::vec2).collect())
            .collect()
    }

    /// Sharpest turn, in degrees, between consecutive pieces of a polyline.
    fn sharpest_turn(path: &[[f64; 2]]) -> f64 {
        let steps: Vec<[f64; 2]> = path
            .windows(2)
            .map(|w| [w[1][0] - w[0][0], w[1][1] - w[0][1]])
            .filter(|d| d[0].hypot(d[1]) > 1e-6)
            .collect();
        steps
            .windows(2)
            .map(|w| {
                let cos = (w[0][0] * w[1][0] + w[0][1] * w[1][1])
                    / (w[0][0].hypot(w[0][1]) * w[1][0].hypot(w[1][1]));
                cos.clamp(-1.0, 1.0).acos().to_degrees()
            })
            .fold(0.0, f64::max)
    }

    fn length(path: &[[f64; 2]]) -> f64 {
        path.windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum()
    }

    /// Two nested L-shaped dependencies whose vertical legs run 20 apart.
    const CURVED: &str = "{:classes [{:id :s0 :level 1 :rect {:x 90 :y -40 :w 20 :h 40}}
                                     {:id :s1 :level 1 :rect {:x 112 :y -40 :w 16 :h 40}}
                                     {:id :t0 :level 3 :rect {:x 400 :y 380 :w 40 :h 40}}
                                     {:id :t1 :level 3 :rect {:x 400 :y 330 :w 40 :h 40}}]
                           :edges [{:from :s0 :to :t0 :kind :dependency
                                    :points [[100 0] [100 400] [400 400]]}
                                   {:from :s1 :to :t1 :kind :dependency
                                    :points [[120 0] [120 350] [400 350]]}]}";

    #[test]
    fn branches_follow_the_flattened_base_curve_instead_of_its_corners() {
        let scene = prepared(CURVED);
        assert_eq!(vec![2], members(&scene));
        for e in seq(get(&scene, "edges")) {
            let base = polylines(get(e, "strokes"))
                .iter()
                .map(|s| sharpest_turn(s))
                .fold(0.0, f64::max);
            for branch in polylines(get(e, "branches")) {
                assert!(
                    sharpest_turn(&branch) <= base + 10.0,
                    "branch turns {:.1}° where its curve turns {base:.1}°: {branch:?}",
                    sharpest_turn(&branch)
                );
            }
        }
    }

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
    fn partially_covering_boxes_titles_and_ports_shorten_the_trunk_clear_of_them() {
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
            // Every blocker starts at y 200 and reaches x 104 from the right.
            let padded = crate::geom::Rect::new(99.0, 195.0, 400.0, 28.0);
            let trunks: Vec<_> = seq(get(&scene, "bundles"))
                .iter()
                .flat_map(|b| polylines(get(b, "strokes")))
                .collect();
            assert!(!trunks.is_empty(), "{name}: the channel above still shares");
            for trunk in trunks {
                assert_eq!(
                    vec![trunk.clone()],
                    crate::geom::gap_polyline(&trunk, &[padded], 0.0),
                    "{name}: trunk clear"
                );
            }
            assert_well_formed(&scene);
        }
    }

    #[test]
    fn a_new_crossing_with_another_relation_rejects_the_channel() {
        let crossing = "{:from :n0 :to :n1 :kind :association :points [[110 0] [110 200]]}";
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

    /// Every branch is absent or long enough to read as one, and no branch
    /// turns sharper than its own base curve: no stubs, boxes or fold-backs.
    fn assert_well_formed(scene: &Value) {
        for (i, e) in seq(get(scene, "edges")).iter().enumerate() {
            let base = polylines(get(e, "strokes"))
                .iter()
                .map(|s| sharpest_turn(s))
                .fold(0.0, f64::max);
            for branch in polylines(get(e, "branches")) {
                assert!(
                    length(&branch) >= super::MIN_BRANCH,
                    "edge {i}: stub branch {branch:?}"
                );
                // A smooth bend turns a little at each sample; a box corner
                // or fold-back turns at least a right angle at once.
                assert!(
                    sharpest_turn(&branch) <= base.max(45.0),
                    "edge {i}: branch folds {:.1}°: {branch:?}",
                    sharpest_turn(&branch)
                );
            }
        }
    }

    fn member_sets(scene: &Value) -> Vec<Vec<i64>> {
        seq(get(scene, "bundles"))
            .iter()
            .map(|b| {
                seq(get(b, "members"))
                    .iter()
                    .filter_map(edn::record::num)
                    .map(|n| n as i64)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn adjacent_runs_of_the_same_members_share_one_trunk_without_stubs() {
        // A third dependency joins the channel for a middle stretch only.
        let scene = prepared(&channel(
            T1,
            "{:id :u0 :level 2 :rect {:x 300 :y 130 :w 40 :h 40}}
             {:id :u1 :level 2 :rect {:x 300 :y 230 :w 40 :h 40}}",
            "",
            "{:from :u0 :to :u1 :kind :dependency
              :points [[300 150] [110 150] [110 250] [300 250]]}",
        ));
        let sets = member_sets(&scene);
        assert!(sets.contains(&vec![0, 1]), "{sets:?}");
        let distinct: std::collections::BTreeSet<_> = sets.iter().collect();
        assert_eq!(
            sets.len(),
            distinct.len(),
            "one trunk per member set: {sets:?}"
        );
        assert_well_formed(&scene);
    }

    #[test]
    fn an_exact_line_trunk_carries_at_most_the_member_cap() {
        let classes: String = (0..14)
            .map(|i| {
                format!(
                    "{{:id :s{i} :foreign true :rect {{:x {} :y 600 :w 8 :h 8}}}}",
                    20 * i
                )
            })
            .collect();
        let edges: String = (0..14)
            .map(|i| {
                format!("{{:from :s{i} :to :t :kind :dependency :points [[100 0] [100 400]]}}")
            })
            .collect();
        let scene = prepared(&format!(
            "{{:classes [{classes} {{:id :t :rect {{:x 80 :y 400 :w 40 :h 40}}}}] :edges [{edges}]}}"
        ));
        let sizes = members(&scene);
        assert!(!sizes.is_empty(), "the shared line still bundles");
        assert!(sizes.iter().all(|n| *n <= super::MAX_MEMBERS), "{sizes:?}");
    }

    #[test]
    fn a_member_whose_bend_adds_a_crossing_is_left_out_of_the_trunk() {
        let three = channel(
            T1,
            "{:id :s2 :level 1 :rect {:x 150 :y -60 :w 40 :h 40}}
             {:id :t2 :level 3 :rect {:x 150 :y 420 :w 40 :h 40}}",
            "",
            "{:from :s2 :to :t2 :kind :dependency :points [[140 0] [140 400]]}",
        );
        let alone = prepared(&three);
        assert_eq!(vec![vec![0, 1, 2]], member_sets(&alone));
        let crossing = "{:from :n0 :to :n1 :kind :association :points [[126 15] [126 80]]}";
        let scene = prepared(&three.replace(
            ":points [[140 0] [140 400]]}",
            &format!(":points [[140 0] [140 400]]}} {crossing}"),
        ));
        assert_eq!(vec![vec![0, 1]], member_sets(&scene));
        assert!(get(&seq(get(&scene, "edges"))[2], "branches").is_none());
        assert_well_formed(&scene);
    }

    #[test]
    fn a_box_near_one_end_shortens_the_trunk_instead_of_rejecting_it() {
        let blocker = "{:id :blocker :level 2 :rect {:x 95 :y 340 :w 10 :h 10}}";
        let scene = prepared(&channel(T1, blocker, "", ""));
        assert_eq!(vec![2], members(&scene));
        let padded = crate::geom::Rect::new(90.0, 335.0, 20.0, 20.0);
        let trunk = polylines(get(&seq(get(&scene, "bundles"))[0], "strokes"));
        let mut drawn: Vec<Vec<[f64; 2]>> = trunk;
        for e in seq(get(&scene, "edges")) {
            drawn.extend(polylines(get(e, "branches")));
        }
        for path in drawn {
            let visible = crate::geom::gap_polyline(&path, &[padded], 0.0);
            assert_eq!(vec![path.clone()], visible, "clear of the box: {path:?}");
        }
        assert_well_formed(&scene);
    }
}
