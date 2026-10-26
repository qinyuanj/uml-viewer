//! Pure drawing model (port of `love/uml/adapters/class_boxes.lua` and
//! `edges.lua`): scene EDN plus the selection/hover hits in, renderer
//! commands out. No egui here; `paint` translates commands into shapes and
//! owns LOD/culling. Colors are egui colors resolved through the theme;
//! widths stay in points and scale with DPI at paint time, never with zoom.

use edn::Value;
use edn::record::{flag, get, local_name, seq};
use egui::Color32;
use scene::document::ViewState;
use scene::highlight::Highlight;
use scene::{Rect, vec2};

use crate::strings;
use crate::theme::{self, Rgb, fonts};

// Text keeps the layout's 12px side padding; package titles also clear the
// C/M dots. A class stacks its C/M dots in the right padding and puts its
// α/I marker in the left one, so its text rows keep the full width the
// layout sized the box for.
pub const PAD: f64 = 12.0;
pub const PACKAGE_TITLE_RESERVE: f64 = 48.0;
/// An unmeasured class reserves a bottom strip for its 无数据 marker.
pub const NO_DATA_STRIP: f64 = 12.0;

/// One popup row of the dependency hover list.
#[derive(Debug, Clone, PartialEq)]
pub struct PopupRow {
    pub text: String,
    pub violating: bool,
    pub color: Color32,
}

/// Renderer commands; every class-owned command carries its owner id and the
/// dim flag of the relationship highlighting.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Package {
        id: Option<Value>,
        rect: Rect,
        text: String,
        band: &'static str,
        red: bool,
        size: f64,
        max_w: f64,
        missing: bool,
        missing_count: i64,
        grade: Option<f64>,
        fill: Rgb,
        stroke: Rgb,
        selected: bool,
    },
    Class {
        rect: Rect,
        shape: String,
        red: bool,
        band: &'static str,
        id: Option<Value>,
        missing: bool,
        grade: Option<f64>,
        fill: Rgb,
        stroke: Rgb,
        selected: bool,
        hovered: bool,
        related: bool,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Quality {
        owner: &'static str,
        id: Option<Value>,
        rect: Rect,
        text: &'static str,
        band: &'static str,
        grade: Option<f64>,
        radius: f64,
        x: f64,
        y: f64,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Marker {
        rect: Rect,
        text: String,
        x: f64,
        y: f64,
        size: f64,
        color: Color32,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Level {
        rect: Rect,
        text: String,
        x: f64,
        y: f64,
        size: f64,
        color: Color32,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    /// The 2px title band under the name slot, in the box stroke color.
    Band {
        rect: Rect,
        x: f64,
        w: f64,
        y: f64,
        color: Rgb,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Rule {
        x: f64,
        y: f64,
        w: f64,
        color: Rgb,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Text {
        role: String,
        italic: bool,
        bold: bool,
        center: bool,
        box_w: f64,
        x: f64,
        y: f64,
        text: String,
        size: f64,
        top: Option<f64>,
        h: Option<f64>,
        max_w: f64,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    NoData {
        rect: Rect,
        text: String,
        right: f64,
        y: f64,
        size: f64,
        color: Color32,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    ChildWash {
        rect: Rect,
        color: Color32,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    PortLink {
        behind: [f64; 2],
        tip: [f64; 2],
        owner_id: Option<Value>,
        dimmed: bool,
    },
    Port {
        id: Option<Value>,
        rect: Rect,
        text: String,
        marked: bool,
        owner_id: Option<Value>,
        dimmed: bool,
    },
    // Screen-space chrome (view coordinates, scaled by DPI at paint time).
    Title {
        text: String,
        x: f64,
        y: f64,
        color: Color32,
    },
    ProposalBanner {
        text: String,
        x: f64,
        y: f64,
        color: Color32,
    },
    Back {
        text: String,
        x: f64,
        y: f64,
        color: Color32,
    },
    Status {
        text: String,
        x: f64,
        y: f64,
        color: Color32,
    },
    /// Empty-canvas / missing-file hint, centered on the canvas.
    Hint { text: String },
    DependencyPopup {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rows: Vec<PopupRow>,
    },
}

impl Command {
    pub fn kind(&self) -> &'static str {
        match self {
            Command::Package { .. } => "package",
            Command::Class { .. } => "class",
            Command::Quality { .. } => "quality",
            Command::Marker { .. } => "marker",
            Command::Level { .. } => "level",
            Command::Band { .. } => "band",
            Command::Rule { .. } => "rule",
            Command::Text { .. } => "text",
            Command::NoData { .. } => "no-data",
            Command::ChildWash { .. } => "child-wash",
            Command::PortLink { .. } => "port-link",
            Command::Port { .. } => "port",
            Command::Title { .. } => "title",
            Command::ProposalBanner { .. } => "proposal-banner",
            Command::Back { .. } => "back",
            Command::Status { .. } => "status",
            Command::Hint { .. } => "hint",
            Command::DependencyPopup { .. } => "dependency-popup",
        }
    }

    /// A rough world-space extent for viewport culling; chrome returns None.
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Command::Package { rect, .. }
            | Command::Class { rect, .. }
            | Command::Quality { rect, .. }
            | Command::Marker { rect, .. }
            | Command::Level { rect, .. }
            | Command::Band { rect, .. }
            | Command::NoData { rect, .. }
            | Command::ChildWash { rect, .. }
            | Command::Port { rect, .. } => Some(*rect),
            Command::Rule { x, y, w, .. } => Some(Rect::new(*x, *y - 1.0, *w, 2.0)),
            Command::Text {
                x, y, box_w, size, ..
            } => Some(Rect::new(*x, *y, *box_w, *size + 4.0)),
            Command::PortLink { behind, tip, .. } => {
                let x = behind[0].min(tip[0]);
                let y = behind[1].min(tip[1]);
                Some(Rect::new(
                    x,
                    y,
                    (behind[0] - tip[0]).abs().max(1.0),
                    (behind[1] - tip[1]).abs().max(1.0),
                ))
            }
            _ => None,
        }
    }
}

/// Edge commands (port of `edges.lua`): strokes, oriented heads and the
/// collapsed-leaf dependency indicators.
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeCommand {
    Stroke {
        from: Option<Value>,
        to: Option<Value>,
        points: Vec<[f64; 2]>,
        color: Rgb,
        width: f32,
        hot: bool,
        warm: bool,
        dimmed: bool,
    },
    Head {
        from: Option<Value>,
        to: Option<Value>,
        head: String,
        tip: [f64; 2],
        behind: [f64; 2],
        color: Rgb,
        width: f32,
        hot: bool,
        warm: bool,
        dimmed: bool,
    },
    Indicator {
        id: Option<Value>,
        points: [[f64; 2]; 3],
        violating: bool,
        dimmed: bool,
    },
}

fn quality(commands: &mut Vec<Command>, c: &Value, owner: &'static str) {
    let Some(r) = Rect::from_value(get(c, "rect").unwrap_or(&Value::Nil)) else {
        return;
    };
    let rating = scene::rating(c);
    for (i, (b, g)) in [
        (rating.crap, rating.crap_grade),
        (rating.mutation, rating.mutation_grade),
    ]
    .into_iter()
    .enumerate()
    {
        commands.push(Command::Quality {
            owner,
            id: get(c, "id").cloned(),
            rect: r,
            text: if i == 0 { "C" } else { "M" },
            band: b.name(),
            grade: Some(g),
            radius: if owner == "class" { 5.0 } else { 6.0 },
            x: if owner == "class" {
                r.right() - 6.0
            } else {
                r.right() - 24.0 + i as f64 * 14.0
            },
            y: if owner == "class" {
                r.y + 9.0 + i as f64 * 12.0
            } else {
                r.y + 10.0
            },
            owner_id: None,
            dimmed: false,
        });
    }
}

/// Vertical slots for a class's text lines (port of `text_slots`): the
/// layout keeps its golden 18px line pitch; nominal sizes when they fit,
/// scaled down together when they do not, with even gaps.
pub fn text_slots(c: &Value, r: Rect, unmeasured: bool) -> Vec<Command> {
    let (x, ry, w, h) = (r.x, r.y, r.w, r.h);
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let mut y = ry + 16.0;
    let mut run_top = ry + if get(c, "level").is_some() { 14.0 } else { 4.0 };
    let stereo = local_name(get(c, "stereotype").unwrap_or(&Value::Nil))
        .unwrap_or_default()
        .to_owned();
    let flush = |out: &mut Vec<Command>, run: &mut Vec<usize>, run_top: f64, run_bottom: f64| {
        let total: f64 = run
            .iter()
            .map(|&i| match &out[i] {
                Command::Text { size, .. } => *size,
                _ => 0.0,
            })
            .sum();
        if !run.is_empty() {
            let avail = run_bottom - run_top;
            let k = (avail / total).min(1.0);
            let gap = (avail - total * k) / (run.len() as f64 + 1.0);
            let mut top = run_top + gap;
            for &i in run.iter() {
                if let Command::Text {
                    size,
                    top: t,
                    h: hh,
                    max_w,
                    x: tx,
                    bold,
                    italic,
                    ..
                } = &mut out[i]
                {
                    *t = Some(top);
                    *hh = Some(*size * k);
                    // The bold offset and italic shear overhang into the
                    // side padding, so a name as wide as the layout's
                    // estimate still draws at full size.
                    let overhang =
                        *size * (if *bold { 0.05 } else { 0.0 } + if *italic { 0.2 } else { 0.0 });
                    *max_w = w - 2.0 * PAD + overhang;
                    *tx = x + PAD - overhang / 2.0;
                    top += *size * k + gap;
                }
            }
        }
        run.clear();
    };
    for line in seq(get(c, "lines")) {
        let kind = local_name(get(line, "kind").unwrap_or(&Value::Nil))
            .unwrap_or_default()
            .to_owned();
        if kind == "rule" {
            flush(&mut out, &mut run, run_top, y - 5.0);
            out.push(Command::Rule {
                x,
                y: y - 4.0,
                w,
                color: theme::stroke(None),
                owner_id: None,
                dimmed: false,
            });
            run_top = y - 3.0;
            y += 12.0;
        } else {
            let contents_len = match get(c, "contents") {
                Some(Value::Vector(v)) => v.len(),
                _ => 0,
            };
            let italic = kind == "name"
                && ((!stereo.is_empty() && stereo != "class")
                    || edn::record::is_true(c, "drill?")
                    || contents_len > 0);
            out.push(Command::Text {
                role: kind.clone(),
                italic,
                bold: kind == "name",
                center: kind == "name",
                box_w: w,
                x: x + PAD,
                y,
                text: match get(line, "text") {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                },
                size: fonts::CANVAS as f64,
                top: None,
                h: None,
                max_w: w - 2.0 * PAD,
                owner_id: None,
                dimmed: false,
            });
            run.push(out.len() - 1);
            y += 18.0;
        }
    }
    let bottom = ry + h - 4.0 - if unmeasured { NO_DATA_STRIP } else { 0.0 };
    flush(&mut out, &mut run, run_top, bottom);
    out
}

fn marked(view_hit: Option<&Value>, kind: &str, id: Option<&Value>) -> bool {
    view_hit.is_some_and(|hit| {
        local_name(get(hit, "kind").unwrap_or(&Value::Nil)).unwrap_or_default() == kind
            && get(hit, "id") == id
    })
}

/// The class boxes, packages, badges and ports of one scene.
pub fn commands(scene: &Value, selected: Option<&Value>, hover: Option<&Value>) -> Vec<Command> {
    let sel_id = selected.and_then(|s| get(s, "id"));
    let h = Highlight::new(scene, selected, hover);
    let hover_id = h.hover.clone();
    let mut out = Vec::new();
    for p in seq(get(scene, "packages")) {
        let rating = scene::rating(p);
        let band = rating.band().name();
        let Some(pr) = Rect::from_value(get(p, "rect").unwrap_or(&Value::Nil)) else {
            continue;
        };
        // Pooled package CRAP substitutes mu=20 for unmeasured members, so
        // missing is counted from the member classes themselves.
        let mut count = 0i64;
        for c in seq(get(scene, "classes")) {
            if get(c, "package") == get(p, "id") && scene::rating(c).missing {
                count += 1;
            }
        }
        let g = rating.grade();
        out.push(Command::Package {
            id: get(p, "id").cloned(),
            rect: pr,
            text: match get(p, "title").or_else(|| get(p, "label")) {
                Some(Value::String(s)) => s.clone(),
                Some(v) => local_name(v).unwrap_or_default().to_owned(),
                None => String::new(),
            },
            band,
            red: band == "red",
            size: fonts::CANVAS as f64,
            // The italic shear overhangs into the dots' reserve.
            max_w: pr.w - PACKAGE_TITLE_RESERVE + fonts::CANVAS as f64 * 0.2,
            missing: count > 0,
            missing_count: count,
            grade: g,
            fill: theme::fill(g),
            stroke: theme::stroke(g),
            selected: sel_id.is_some() && get(p, "id") == sel_id,
        });
        quality(&mut out, p, "package");
    }
    for c in seq(get(scene, "classes")) {
        let Some(r) = Rect::from_value(get(c, "rect").unwrap_or(&Value::Nil)) else {
            continue;
        };
        let rating = scene::rating(c);
        let band = rating.band().name();
        let id = get(c, "id").cloned();
        let unmeasured = rating.missing;
        let g = rating.class_grade();
        let dimmed = id.as_ref().is_some_and(|id| h.dimmed(id));
        let tag = |command: &mut Command| match command {
            Command::Class {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Quality {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Marker {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Level {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Band {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Rule {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Text {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::NoData {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::ChildWash {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::PortLink {
                owner_id,
                dimmed: d,
                ..
            }
            | Command::Port {
                owner_id,
                dimmed: d,
                ..
            } => {
                *owner_id = id.clone();
                *d = dimmed;
            }
            _ => {}
        };
        let mut class_commands = Vec::new();
        class_commands.push(Command::Class {
            rect: r,
            shape: local_name(get(c, "shape").unwrap_or(&Value::Nil))
                .unwrap_or_default()
                .to_owned(),
            red: band == "red",
            band,
            id: id.clone(),
            missing: unmeasured,
            grade: g,
            fill: theme::fill(g),
            stroke: theme::stroke(g),
            selected: h.focus.is_some() && h.focus == id,
            hovered: hover_id.is_some() && hover_id == id,
            related: id.as_ref().is_some_and(|id| h.related(id)),
            owner_id: None,
            dimmed: false,
        });
        quality(&mut class_commands, c, "class");
        let stereo = local_name(get(c, "stereotype").unwrap_or(&Value::Nil))
            .unwrap_or_default()
            .to_owned();
        let level = get(c, "level").map(|level| match level {
            Value::Integer(n) => n.to_string(),
            Value::Float(x) => format!("{x}"),
            _ => local_name(level).unwrap_or_default().to_owned(),
        });
        let marker = stereo == "abstract" || stereo == "interface";
        if marker {
            class_commands.push(Command::Marker {
                rect: r,
                text: if stereo == "abstract" {
                    "α".into()
                } else {
                    "I".into()
                },
                // Top-left, after the level number when there is one.
                x: r.x
                    + 4.0
                    + level
                        .as_deref()
                        .map_or(0.0, |t| estimate_w(t, fonts::LEVEL as f64) + 3.0),
                y: r.y + 3.0,
                size: fonts::MARKER as f64,
                color: Color32::WHITE,
                owner_id: None,
                dimmed: false,
            });
        }
        if let Some(level) = level {
            class_commands.push(Command::Level {
                rect: r,
                text: level,
                x: r.x + 4.0,
                y: r.y + 3.0,
                size: fonts::LEVEL as f64,
                color: theme::MUTED.color32(),
                owner_id: None,
                dimmed: false,
            });
        }
        let mut slots = text_slots(c, r, unmeasured);
        // The 2px title band runs under the name slot in the box stroke color.
        for s in &slots {
            if let Command::Text {
                role,
                top: Some(top),
                h: Some(sh),
                ..
            } = s
                && role == "name"
            {
                class_commands.push(Command::Band {
                    rect: r,
                    x: r.x + 6.0,
                    w: r.w - 12.0,
                    y: (r.bottom() - 2.0).min(top + sh + 1.0),
                    color: theme::stroke(g),
                    owner_id: None,
                    dimmed: false,
                });
                break;
            }
        }
        let lines = seq(get(c, "lines"));
        for (i, mut line) in slots.drain(..).enumerate() {
            if let Command::Text {
                role,
                top,
                h: sh,
                y,
                ..
            } = &line
                && role == "child"
            {
                let source_id = lines.get(i).and_then(|l| get(l, "id"));
                if marked(selected, "child", source_id) || marked(hover, "child", source_id) {
                    class_commands.push(Command::ChildWash {
                        rect: Rect::new(
                            r.x + 5.0,
                            top.unwrap_or(*y) - 1.0,
                            r.w - 10.0,
                            sh.unwrap_or(18.0),
                        ),
                        color: theme::CHILD_WASH.with_alpha(theme::ALPHA_CHILD_WASH),
                        owner_id: None,
                        dimmed: false,
                    });
                }
            }
            if let Command::Rule { color, .. } = &mut line {
                *color = theme::stroke(g);
            }
            class_commands.push(line);
        }
        for dir in ["in", "out"] {
            for p in seq(get(c, &format!("{dir}-ports"))) {
                let Some(pr) = Rect::from_value(get(p, "rect").unwrap_or(&Value::Nil)) else {
                    continue;
                };
                let px = pr.cx();
                let bx = (r.x + 6.0).max(px.min(r.right() - 6.0));
                let behind = if dir == "in" {
                    [px, pr.bottom()]
                } else {
                    [bx, r.bottom()]
                };
                let tip = if dir == "in" { [bx, r.y] } else { [px, pr.y] };
                class_commands.push(Command::PortLink {
                    behind,
                    tip,
                    owner_id: None,
                    dimmed: false,
                });
                class_commands.push(Command::Port {
                    id: get(p, "id").cloned(),
                    rect: pr,
                    text: match get(p, "name") {
                        Some(Value::String(s)) => s.clone(),
                        _ => local_name(get(p, "id").unwrap_or(&Value::Nil))
                            .unwrap_or_default()
                            .to_owned(),
                    },
                    marked: marked(selected, "port", get(p, "id"))
                        || marked(hover, "port", get(p, "id")),
                    owner_id: None,
                    dimmed: false,
                });
            }
        }
        if unmeasured {
            class_commands.push(Command::NoData {
                rect: r,
                text: strings::NO_DATA.into(),
                right: r.right() - PAD,
                y: r.bottom() - NO_DATA_STRIP - 2.0,
                size: fonts::LEVEL as f64,
                color: theme::band_color("red").color32(),
                owner_id: None,
                dimmed: false,
            });
        }
        for command in &mut class_commands {
            tag(command);
        }
        out.extend(class_commands);
    }
    out
}

/// Violation semantics win: a violating edge keeps its red family when hot,
/// and a hover lift never thins it below its violation width.
fn edge_style(violating: bool, hot: bool, warm: bool) -> (Rgb, f32) {
    use theme::widths as W;
    if violating {
        return (
            if hot {
                theme::EDGE_VIOLATING_HOT
            } else if warm {
                theme::EDGE_VIOLATING_WARM
            } else {
                theme::EDGE_VIOLATING
            },
            if hot {
                W::EDGE_VIOLATING_HOT
            } else {
                W::EDGE_VIOLATING
            },
        );
    }
    if hot {
        return (theme::EDGE_HOT, W::EDGE_HOT);
    }
    if warm {
        return (theme::EDGE_WARM, W::EDGE_WARM);
    }
    (theme::EDGE, W::EDGE)
}

fn points_of(v: Option<&Value>) -> Vec<[f64; 2]> {
    seq(v).iter().filter_map(vec2).collect()
}

/// The edge strokes, arrowheads and collapsed dependency indicators.
pub fn edge_commands(
    scene: &Value,
    selected: Option<&Value>,
    hover: Option<&Value>,
) -> Vec<EdgeCommand> {
    let h = Highlight::new(scene, selected, hover);
    let mut out = Vec::new();
    let hide = get(scene, "diagram").is_some_and(|d| flag(d, "hide-edges"));
    if hide {
        for ind in seq(get(scene, "dep-indicators")) {
            let points = points_of(get(ind, "triangle"));
            if points.len() != 3 {
                continue;
            }
            let violating = flag(ind, "violating");
            out.push(EdgeCommand::Indicator {
                id: get(ind, "id").cloned(),
                points: [points[0], points[1], points[2]],
                violating,
                dimmed: get(ind, "id").is_some_and(|id| h.dimmed(id)),
            });
        }
        return out;
    }
    for e in seq(get(scene, "edges")) {
        let hot = h.hot(e, hover);
        let warm = !hot && h.warm(e);
        let dimmed = h.focus.is_some() && !hot;
        let violating = flag(e, "violating");
        let (color, width) = edge_style(violating, hot, warm);
        let from = get(e, "from").cloned();
        let to = get(e, "to").cloned();
        for ps in seq(get(e, "strokes")) {
            out.push(EdgeCommand::Stroke {
                from: from.clone(),
                to: to.clone(),
                points: points_of(Some(ps)),
                color,
                width,
                hot,
                warm,
                dimmed,
            });
        }
        if let (Some(head), Some(tip), Some(behind)) = (
            get(e, "head"),
            vec2(get(e, "tip").unwrap_or(&Value::Nil)),
            vec2(get(e, "behind").unwrap_or(&Value::Nil)),
        ) {
            out.push(EdgeCommand::Head {
                from,
                to,
                head: local_name(head).unwrap_or_default().to_owned(),
                tip,
                behind,
                color,
                width,
                hot,
                warm,
                dimmed,
            });
        }
    }
    out
}

/// Natural advance fallback: the layout's 8px/char monospace estimate at
/// 16px, scaled. The real measure comes from egui at paint time.
pub fn estimate_w(text: &str, size: f64) -> f64 {
    scene::text_w(Some(text)) / 16.0 * size
}

/// The layout's 8px/char estimate fixes box widths (JVM goldens); a bold or
/// italic name may be wider, so the drawn size shrinks until the text, bold offset
/// and italic shear fit inside `max_w`. Never grows past the nominal size.
pub fn fit(
    measure: impl Fn(f64, &str) -> f64,
    size: f64,
    text: &str,
    max_w: f64,
    bold: bool,
    italic: bool,
) -> f64 {
    let width = |s: f64| {
        measure(s, text) + if bold { s * 0.05 } else { 0.0 } + if italic { s * 0.2 } else { 0.0 }
    };
    let natural = width(size);
    if natural <= max_w || natural <= 0.0 {
        return size;
    }
    let budget = max_w.max(0.0);
    let mut fitted = size * budget / natural;
    // Real glyph advances snap to the pixel grid, so width is not linear in
    // size: shrink until the measured text is inside the budget (egui would
    // otherwise wrap the label).
    for _ in 0..8 {
        let w = width(fitted);
        if w <= budget || w <= 0.0 {
            break;
        }
        fitted *= budget / w * 0.999;
    }
    fitted
}

/// Text ink by role: crap lines gold, stereotypes muted, everything else ink.
pub fn text_color(role: &str) -> Rgb {
    match role {
        "crap" => theme::GOLD,
        "stereo" => theme::MUTED,
        _ => theme::INK,
    }
}

/// A centered canvas hint command (waiting for :display, missing file).
pub fn hint(text: &str) -> Command {
    Command::Hint { text: text.into() }
}

/// Screen-space canvas chrome (port of `class_boxes.chrome`): the waiting
/// hint, the diagram banner, return scope and the leaf-dependency popup
/// (Declutter lives in the inspector, #141).
pub fn chrome(state: &ViewState, width: f64, height: f64) -> Vec<Command> {
    let mut out = Vec::new();
    if state.waiting {
        out.push(hint(strings::WAITING_DISPLAY));
        return out;
    }
    let diagram = get(&state.scene, "diagram")
        .cloned()
        .unwrap_or(Value::Map(vec![]));
    let title = get(&diagram, "title");
    if flag(&diagram, "proposal") {
        let mut proposal_name: Option<String> = None;
        if let Some(doc) = &state.document {
            for p in &scene::proposals(doc) {
                if get(p, "id") == get(&state.extra, "proposal-id") {
                    proposal_name = Some(match get(p, "name") {
                        Some(Value::String(s)) => s.clone(),
                        _ => local_name(get(p, "id").unwrap_or(&Value::Nil))
                            .unwrap_or_default()
                            .to_owned(),
                    });
                }
            }
        }
        out.push(Command::ProposalBanner {
            text: format!(
                "{}{}",
                strings::PROPOSAL_BANNER,
                proposal_name.map(|n| format!(" · {n}")).unwrap_or_default()
            ),
            x: 12.0,
            y: 8.0,
            color: theme::GOLD.color32(),
        });
    } else if let Some(Value::String(title)) = title {
        out.push(Command::Title {
            text: title.clone(),
            x: 12.0,
            y: 8.0,
            color: theme::MUTED.color32(),
        });
    }
    if !state.focus.is_empty() || get(&state.extra, "open-layer").is_some() {
        let parts: Vec<String> = state
            .focus
            .iter()
            .map(|v| local_name(v).unwrap_or_default().to_owned())
            .collect();
        let scope = if !parts.is_empty() {
            parts.join(".")
        } else {
            title
                .map(|t| match t {
                    Value::String(s) => s.clone(),
                    other => local_name(other).unwrap_or_default().to_owned(),
                })
                .unwrap_or_else(|| {
                    local_name(get(&state.extra, "open-layer").unwrap_or(&Value::Nil))
                        .unwrap_or_default()
                        .to_owned()
                })
        };
        out.push(Command::Back {
            text: format!("{} · {scope}", strings::BACK),
            x: 12.0,
            y: 28.0,
            color: theme::GOLD.color32(),
        });
    }
    let hover_kind = state
        .hover
        .as_ref()
        .map(|hv| {
            local_name(get(hv, "kind").unwrap_or(&Value::Nil))
                .unwrap_or_default()
                .to_owned()
        })
        .unwrap_or_default();
    let deps = state.hover.as_ref().and_then(|hv| get(hv, "deps"));
    if (hover_kind == "edge" || hover_kind == "dep")
        && let Some(Value::Vector(deps)) = deps
        && !deps.is_empty()
        && let Some((mx, my)) = state.pointer
    {
        let mut rows: Vec<PopupRow> = deps
            .iter()
            .map(|d| {
                let from = local_name(get(d, "from").unwrap_or(&Value::Nil))
                    .unwrap_or_default()
                    .to_owned();
                let to = local_name(get(d, "to").unwrap_or(&Value::Nil))
                    .unwrap_or_default()
                    .to_owned();
                let violating = flag(d, "violating");
                PopupRow {
                    text: format!("{from} -> {to}"),
                    violating,
                    color: if violating {
                        theme::VIOLATION.color32()
                    } else {
                        theme::INK.color32()
                    },
                }
            })
            .collect();
        rows.sort_by(|a, b| b.violating.cmp(&a.violating).then(a.text.cmp(&b.text)));
        let mut tw = 40.0f64;
        for row in &rows {
            tw = tw.max(estimate_w(&row.text, fonts::BODY as f64));
        }
        let w = (tw + 16.0).min((16.0f64).max(width - 16.0));
        let h = 16.0 + rows.len() as f64 * (fonts::BODY as f64 + 2.0);
        let x = (8.0f64).max(mx.min(width - w - 8.0));
        let mut y = my + 16.0;
        if y + h > height {
            y = (8.0f64).max(my - h - 8.0);
        }
        out.push(Command::DependencyPopup { x, y, w, h, rows });
    }
    out
}
