//! egui painter for the pure command model: LOD selection, viewport culling,
//! the zoom transform and the canvas chrome. Strokes are fixed screen points;
//! only world geometry scales with zoom (the LOVE adapter's contract).

use egui::{Align, Color32, FontFamily, FontId, Pos2, Rect as ERect, Stroke, StrokeKind};
use scene::document::ViewState;
use scene::geom::Rect;

use crate::commands::{self, Command, EdgeCommand};
use crate::{lod, strings, theme};

pub fn canvas_bg() -> Color32 {
    theme::CANVAS_BG.color32()
}

/// World -> screen (points): `screen = origin + world * scale`.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub origin: Pos2,
    pub scale: f32,
}

impl Transform {
    pub fn new(rect: ERect, state: &ViewState) -> Transform {
        let scale = (if state.zoom > 0.0 { state.zoom } else { 1.0 }) as f32;
        Transform {
            origin: Pos2::new(
                rect.min.x - state.cam_x as f32 * scale,
                rect.min.y - state.cam_y as f32 * scale,
            ),
            scale,
        }
    }

    pub fn pos(&self, p: [f64; 2]) -> Pos2 {
        Pos2::new(
            self.origin.x + p[0] as f32 * self.scale,
            self.origin.y + p[1] as f32 * self.scale,
        )
    }

    pub fn rect(&self, r: Rect) -> ERect {
        ERect::from_min_max(self.pos([r.x, r.y]), self.pos([r.right(), r.bottom()]))
    }

    /// The world rect currently visible inside `screen`.
    pub fn world_viewport(&self, screen: ERect) -> Rect {
        let scale = self.scale.max(1e-6);
        Rect::new(
            ((screen.min.x - self.origin.x) / scale) as f64,
            ((screen.min.y - self.origin.y) / scale) as f64,
            (screen.width() / scale) as f64,
            (screen.height() / scale) as f64,
        )
    }

    /// Screen point back to world coordinates (event input).
    pub fn world(&self, p: Pos2) -> (f64, f64) {
        let scale = self.scale.max(1e-6);
        (
            ((p.x - self.origin.x) / scale) as f64,
            ((p.y - self.origin.y) / scale) as f64,
        )
    }
}

fn ink(color: Color32, dimmed: bool) -> Color32 {
    if dimmed { theme::fade(color) } else { color }
}

fn stroke_of(color: Color32, width: f32) -> Stroke {
    Stroke::new(width, color)
}

/// Measure a string at a font size through the live egui fonts.
pub fn measure(ctx: &egui::Context, size: f32, text: &str) -> f64 {
    ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), FontId::monospace(size), Color32::WHITE)
            .size()
            .x as f64
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    ctx: &egui::Context,
    painter: &egui::Painter,
    x: f32,
    y: f32,
    label: &str,
    size: f32,
    color: Color32,
    italic: bool,
    bold: bool,
    center: bool,
    max_w: Option<f32>,
) {
    if label.is_empty() || size < 1.0 {
        return;
    }
    let size = max_w.map_or(size, |budget| {
        commands::fit(
            |s, t| measure(ctx, s as f32, t),
            size as f64,
            label,
            budget as f64,
            bold,
            italic,
        ) as f32
    });
    if size < 1.0 {
        return;
    }
    let mut job = egui::text::LayoutJob::default();
    job.append(
        label,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(size, FontFamily::Monospace),
            color,
            italics: italic,
            ..Default::default()
        },
    );
    job.halign = if center { Align::Center } else { Align::LEFT };
    if let Some(budget) = max_w {
        job.wrap.max_width = budget;
    }
    let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
    // Bold is a second pass with the adapter's 5% offset.
    for pass in 0..if bold { 2 } else { 1 } {
        let offset = if pass == 0 { 0.0 } else { size * 0.05 };
        painter.galley(Pos2::new(x + offset, y), galley.clone(), color);
    }
}

fn dashed_rect(painter: &egui::Painter, rect: ERect, stroke: Stroke) {
    let dash = theme::widths::DASH;
    let gap = theme::widths::DASH_GAP;
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    for pair in corners.windows(2) {
        for shape in egui::Shape::dashed_line(pair, stroke, dash, gap) {
            painter.add(shape);
        }
    }
}

fn ellipse(painter: &egui::Painter, rect: ERect, fill: Color32, stroke: Stroke) {
    let center = rect.center();
    let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
    let points: Vec<Pos2> = (0..32)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 32.0;
            Pos2::new(center.x + rx * a.cos(), center.y + ry * a.sin())
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(points, fill, stroke));
}

fn paint_head(
    painter: &egui::Painter,
    transform: &Transform,
    head: &str,
    tip: [f64; 2],
    behind: [f64; 2],
    color: Color32,
    width: f32,
) {
    let p = transform.pos(tip);
    let b = transform.pos(behind);
    let (mut dx, mut dy) = (p.x - b.x, p.y - b.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        dx = 1.0;
        dy = 0.0;
    } else {
        dx /= len;
        dy /= len;
    }
    let (sin, cos) = (std::f32::consts::PI / 9.0).sin_cos();
    let l = 16.0 * transform.scale;
    let a = Pos2::new(
        p.x - l * (dx * cos - dy * sin),
        p.y - l * (dy * cos + dx * sin),
    );
    let c = Pos2::new(
        p.x - l * (dx * cos + dy * sin),
        p.y - l * (dy * cos - dx * sin),
    );
    let stroke = stroke_of(color, width);
    let bg = canvas_bg();
    match head {
        "triangle" => {
            painter.add(egui::Shape::convex_polygon(vec![p, a, c], bg, stroke));
        }
        "diamond" | "diamond-fill" => {
            let back = Pos2::new(p.x - l * dx, p.y - l * dy);
            let fill = if head == "diamond-fill" {
                theme::INK.color32()
            } else {
                bg
            };
            painter.add(egui::Shape::convex_polygon(
                vec![p, a, back, c],
                fill,
                stroke,
            ));
        }
        _ => {
            painter.line_segment([p, a], stroke);
            painter.line_segment([p, c], stroke);
        }
    }
}

fn polyline_visible(points: &[[f64; 2]], viewport: Rect) -> bool {
    let Some(first) = points.first() else {
        return false;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first[0], first[1], first[0], first[1]);
    for p in points {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y).overlaps(viewport)
}

fn point_visible(p: [f64; 2], viewport: Rect) -> bool {
    Rect::new(p[0], p[1], 0.0, 0.0).overlaps(viewport)
}

fn paint_edges(
    painter: &egui::Painter,
    transform: &Transform,
    viewport: Rect,
    commands: &[EdgeCommand],
    tier: lod::Tier,
) {
    for command in commands {
        match command {
            EdgeCommand::Stroke {
                points,
                color,
                width,
                dimmed,
                ..
            } => {
                if points.len() < 2 || !polyline_visible(points, viewport) {
                    continue;
                }
                let simplified = if tier == lod::Tier::Frames {
                    let budget = (lod::DECIMATE_PX / transform.scale.max(1e-6) as f64).max(1e-6);
                    lod::decimate(points, budget)
                } else {
                    points.clone()
                };
                let screen: Vec<Pos2> = simplified.iter().map(|p| transform.pos(*p)).collect();
                painter.add(egui::Shape::line(
                    screen,
                    stroke_of(ink(color.color32(), *dimmed), *width),
                ));
            }
            EdgeCommand::Head {
                head,
                tip,
                behind,
                color,
                width,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                if !point_visible(*tip, viewport) && !point_visible(*behind, viewport) {
                    continue;
                }
                paint_head(
                    painter,
                    transform,
                    head,
                    *tip,
                    *behind,
                    ink(color.color32(), *dimmed),
                    *width,
                );
            }
            EdgeCommand::Indicator {
                points,
                violating,
                dimmed,
                ..
            } => {
                if tier == lod::Tier::Blocks {
                    continue;
                }
                let fill = ink(
                    if *violating {
                        theme::band_color("red").color32()
                    } else {
                        theme::INDICATOR.color32()
                    },
                    *dimmed,
                );
                let screen: Vec<Pos2> = points.iter().map(|p| transform.pos(*p)).collect();
                painter.add(egui::Shape::convex_polygon(
                    screen,
                    fill,
                    stroke_of(fill, 1.0),
                ));
            }
        }
    }
}

/// Paint one frame of the canvas: background, geometry and chrome.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    ctx: &egui::Context,
    painter: &egui::Painter,
    state: &ViewState,
    screen: ERect,
    hint: Option<&str>,
    status: Option<&str>,
    performance: Option<String>,
    show_performance: bool,
) {
    painter.rect_filled(screen, 0.0, canvas_bg());
    let transform = Transform::new(screen, state);
    let tier = lod::tier(if state.zoom > 0.0 { state.zoom } else { 1.0 });
    let viewport = transform.world_viewport(screen);
    let clipped = painter.with_clip_rect(screen);

    let commands = commands::commands(&state.scene, state.selected.as_ref(), state.hover.as_ref());
    for command in &commands {
        if command.bounds().is_some_and(|b| !lod::visible(b, viewport)) {
            continue;
        }
        match command {
            Command::Package {
                rect,
                text: label,
                fill,
                stroke,
                missing,
                selected,
                max_w,
                ..
            } => {
                let r = transform.rect(*rect);
                if r.width() < 1.0 || r.height() < 1.0 {
                    continue;
                }
                let corner = (theme::radius::PACKAGE * transform.scale).max(0.0);
                painter.rect_filled(r, corner, fill.with_alpha(theme::ALPHA_PACKAGE_FILL));
                let line = if *selected {
                    stroke_of(theme::GOLD.color32(), theme::widths::SELECTED)
                } else {
                    stroke_of(
                        stroke.with_alpha(theme::ALPHA_PACKAGE_STROKE),
                        theme::widths::PACKAGE,
                    )
                };
                if *missing {
                    dashed_rect(&clipped, r, line);
                } else {
                    painter.rect_stroke(r, corner, line, StrokeKind::Inside);
                }
                let budget = (*max_w * transform.scale as f64) as f32;
                draw_text(
                    ctx,
                    &clipped,
                    r.min.x + commands::PAD as f32 * transform.scale,
                    r.min.y + 8.0 * transform.scale,
                    label,
                    lod::font_px(theme::fonts::TITLE as f64 * transform.scale as f64, 1.0),
                    theme::GOLD.color32(),
                    true,
                    false,
                    false,
                    Some(budget),
                );
            }
            Command::Class {
                rect,
                shape,
                fill,
                stroke,
                missing,
                selected,
                hovered,
                related,
                dimmed,
                ..
            } => {
                let r = transform.rect(*rect);
                if r.width() < 1.0 || r.height() < 1.0 {
                    continue;
                }
                let corner = (theme::radius::CLASS * transform.scale).max(0.0);
                let line_color = if *selected {
                    ink(theme::GOLD.color32(), *dimmed)
                } else if *hovered {
                    ink(Color32::WHITE, *dimmed)
                } else if *related {
                    ink(theme::RELATED.color32(), *dimmed)
                } else {
                    ink(stroke.color32(), *dimmed)
                };
                let width = if *selected {
                    theme::widths::SELECTED
                } else {
                    theme::widths::CLASS
                };
                if shape == "oval" {
                    ellipse(
                        &clipped,
                        r,
                        ink(fill.color32(), *dimmed),
                        stroke_of(line_color, width),
                    );
                } else {
                    painter.rect_filled(r, corner, ink(fill.color32(), *dimmed));
                    if *missing {
                        dashed_rect(&clipped, r, stroke_of(line_color, width));
                    } else {
                        painter.rect_stroke(
                            r,
                            corner,
                            stroke_of(line_color, width),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            Command::Quality {
                x,
                y,
                radius,
                grade,
                text: label,
                dimmed,
                ..
            } => {
                if tier == lod::Tier::Blocks {
                    continue;
                }
                let center = transform.pos([*x, *y]);
                let r = *radius as f32 * transform.scale;
                painter.circle_filled(center, r, ink(theme::fill(*grade).color32(), *dimmed));
                painter.circle_stroke(
                    center,
                    r,
                    stroke_of(
                        ink(theme::stroke(*grade).color32(), *dimmed),
                        theme::widths::DOT,
                    ),
                );
                if tier == lod::Tier::Full {
                    draw_text(
                        ctx,
                        &clipped,
                        center.x,
                        center.y - 4.5,
                        label,
                        lod::font_px(9.0 * transform.scale as f64, 1.0),
                        ink(Color32::WHITE, *dimmed),
                        false,
                        false,
                        true,
                        None,
                    );
                }
            }
            Command::Marker {
                x,
                y,
                text: label,
                size,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let p = transform.pos([*x, *y]);
                draw_text(
                    ctx,
                    &clipped,
                    p.x,
                    p.y,
                    label,
                    lod::font_px(*size * transform.scale as f64, 1.0),
                    ink(*color, *dimmed),
                    false,
                    false,
                    false,
                    None,
                );
            }
            Command::Level {
                x,
                y,
                text: label,
                size,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let p = transform.pos([*x, *y]);
                draw_text(
                    ctx,
                    &clipped,
                    p.x,
                    p.y,
                    label,
                    lod::font_px(*size * transform.scale as f64, 1.0),
                    ink(*color, *dimmed),
                    false,
                    false,
                    false,
                    None,
                );
            }
            Command::Band {
                x,
                w,
                y,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let a = transform.pos([*x, *y]);
                let b = transform.pos([x + w, *y]);
                painter.line_segment(
                    [a, b],
                    stroke_of(ink(color.color32(), *dimmed), theme::widths::BAND),
                );
            }
            Command::Rule {
                x,
                y,
                w,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let a = transform.pos([x + 6.0, *y]);
                let b = transform.pos([x + w - 6.0, *y]);
                painter.line_segment(
                    [a, b],
                    stroke_of(ink(color.color32(), *dimmed), theme::widths::RULE),
                );
            }
            Command::Text {
                role,
                italic,
                bold,
                center,
                x,
                y,
                text: label,
                size,
                top,
                h,
                max_w,
                dimmed,
                ..
            } => {
                // Blocks show silhouettes only; Frames add the name line.
                if tier == lod::Tier::Blocks || (tier == lod::Tier::Frames && role != "name") {
                    continue;
                }
                let px = lod::font_px(*size * transform.scale as f64, 1.0);
                let top_px = transform.pos([*x, top.unwrap_or(*y)]).y;
                let slot_px = h.unwrap_or(*size) as f32 * transform.scale;
                let base_y = top_px + ((slot_px - px) / 2.0).max(0.0);
                let cx = if *center {
                    transform.pos([x + max_w / 2.0, 0.0]).x
                } else {
                    transform.pos([*x, 0.0]).x
                };
                draw_text(
                    ctx,
                    &clipped,
                    cx,
                    base_y,
                    label,
                    px,
                    ink(commands::text_color(role).color32(), *dimmed),
                    *italic,
                    *bold,
                    *center,
                    Some((*max_w * transform.scale as f64) as f32),
                );
            }
            Command::NoData {
                right,
                y,
                text: label,
                size,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let px = lod::font_px(*size * transform.scale as f64, 1.0);
                let p = transform.pos([*right, *y]);
                let width = measure(ctx, px, label) as f32;
                draw_text(
                    ctx,
                    &clipped,
                    p.x - width,
                    p.y,
                    label,
                    px,
                    ink(*color, *dimmed),
                    false,
                    false,
                    false,
                    None,
                );
            }
            Command::ChildWash {
                rect,
                color,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let r = transform.rect(*rect);
                painter.rect_filled(r, 2.0 * transform.scale, ink(*color, *dimmed));
            }
            Command::PortLink {
                behind,
                tip,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let color = ink(theme::MUTED.color32(), *dimmed);
                let a = transform.pos(*behind);
                let b = transform.pos(*tip);
                painter.line_segment([a, b], stroke_of(color, theme::widths::PORT_LINK));
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0 {
                    let (dx, dy) = (dx / len, dy / len);
                    let (si, co) = (std::f32::consts::PI / 9.0).sin_cos();
                    for sign in [-1.0f32, 1.0] {
                        let end = Pos2::new(
                            b.x - 10.0 * (dx * co - sign * dy * si),
                            b.y - 10.0 * (dy * co + sign * dx * si),
                        );
                        painter.line_segment([b, end], stroke_of(color, theme::widths::PORT_LINK));
                    }
                }
            }
            Command::Port {
                rect,
                text: label,
                marked,
                dimmed,
                ..
            } => {
                if tier != lod::Tier::Full {
                    continue;
                }
                let r = transform.rect(*rect);
                let corner = 3.0 * transform.scale;
                painter.rect_filled(
                    r,
                    corner,
                    ink(
                        if *marked {
                            theme::PORT_FILL_ON.color32()
                        } else {
                            theme::PORT_FILL.color32()
                        },
                        *dimmed,
                    ),
                );
                painter.rect_stroke(
                    r,
                    corner,
                    stroke_of(
                        ink(
                            if *marked {
                                theme::GOLD.color32()
                            } else {
                                theme::MUTED.color32()
                            },
                            *dimmed,
                        ),
                        if *marked {
                            theme::widths::PORT_ON
                        } else {
                            theme::widths::PORT
                        },
                    ),
                    StrokeKind::Inside,
                );
                draw_text(
                    ctx,
                    &clipped,
                    r.center().x,
                    r.center().y - 5.0,
                    label,
                    10.0,
                    ink(
                        if *marked {
                            theme::INK.color32()
                        } else {
                            theme::MUTED.color32()
                        },
                        *dimmed,
                    ),
                    false,
                    false,
                    true,
                    Some(r.width() - 6.0),
                );
            }
            _ => {}
        }
    }

    let edges =
        commands::edge_commands(&state.scene, state.selected.as_ref(), state.hover.as_ref());
    paint_edges(&clipped, &transform, viewport, &edges, tier);

    // Screen-space chrome: banner, return scope, popup (view units = points).
    for command in commands::chrome(state, screen.width() as f64, screen.height() as f64) {
        match command {
            Command::Title {
                text: label,
                x,
                y,
                color,
            }
            | Command::ProposalBanner {
                text: label,
                x,
                y,
                color,
            }
            | Command::Back {
                text: label,
                x,
                y,
                color,
            } => draw_text(
                ctx,
                painter,
                screen.min.x + x as f32,
                screen.min.y + y as f32,
                &label,
                18.0,
                color,
                false,
                false,
                false,
                None,
            ),
            Command::Hint { text: label } => draw_text(
                ctx,
                painter,
                screen.center().x,
                screen.center().y,
                &label,
                18.0,
                theme::MUTED.color32(),
                false,
                false,
                true,
                None,
            ),
            Command::DependencyPopup { x, y, w, h, rows } => {
                let box_rect = ERect::from_min_size(
                    Pos2::new(screen.min.x + x as f32, screen.min.y + y as f32),
                    egui::vec2(w as f32, h as f32),
                );
                painter.rect_filled(
                    box_rect,
                    4.0,
                    theme::POPUP_BG.with_alpha(theme::ALPHA_POPUP),
                );
                painter.rect_stroke(
                    box_rect,
                    4.0,
                    stroke_of(theme::LINE.color32(), 1.0),
                    StrokeKind::Inside,
                );
                for (i, row) in rows.iter().enumerate() {
                    draw_text(
                        ctx,
                        painter,
                        box_rect.min.x + 8.0,
                        box_rect.min.y + 8.0 + i as f32 * 20.0,
                        &row.text,
                        18.0,
                        row.color,
                        false,
                        false,
                        false,
                        None,
                    );
                }
            }
            Command::Declutter { rect, text: label } => {
                let r = ERect::from_min_size(
                    Pos2::new(screen.min.x + rect.x as f32, screen.min.y + rect.y as f32),
                    egui::vec2(rect.w as f32, rect.h as f32),
                );
                painter.rect_filled(r, 4.0, theme::PANEL.color32());
                painter.rect_stroke(
                    r,
                    4.0,
                    stroke_of(theme::LINE.color32(), 1.0),
                    StrokeKind::Inside,
                );
                draw_text(
                    ctx,
                    painter,
                    r.center().x,
                    r.min.y + 5.0,
                    &label,
                    16.0,
                    theme::INK.color32(),
                    false,
                    false,
                    true,
                    Some(r.width() - 8.0),
                );
            }
            _ => {}
        }
    }
    if let Some(label) = hint {
        draw_text(
            ctx,
            painter,
            screen.center().x,
            screen.center().y,
            label,
            18.0,
            theme::MUTED.color32(),
            false,
            false,
            true,
            None,
        );
    }
    if let Some(label) = status {
        draw_text(
            ctx,
            painter,
            screen.min.x + 12.0,
            screen.max.y - 30.0,
            label,
            18.0,
            theme::MUTED.color32(),
            false,
            false,
            false,
            None,
        );
    }
    if show_performance && let Some(line) = performance {
        draw_text(
            ctx,
            painter,
            screen.min.x + 12.0,
            screen.min.y + 48.0,
            &line,
            18.0,
            theme::MUTED.color32(),
            false,
            false,
            false,
            None,
        );
    }
}

/// Lay out the probe upright and slanted through the live font atlas and
/// compare the tessellated glyph vertices: identical topology, identical
/// baselines, shifted x. This is the egui equivalent of the cimgui
/// vertex-slant assertion in `scripts/test_love_class_boxes.lua`.
pub fn italic_glyph_probe(ctx: &egui::Context) -> Result<(), String> {
    italic_glyph_probe_text(ctx, "Abstract")
}

pub fn italic_glyph_probe_text(ctx: &egui::Context, probe: &str) -> Result<(), String> {
    if probe.trim().is_empty() {
        return Err("italic probe needs a non-empty text".into());
    }
    let layout = |italics: bool| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            probe,
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(18.0),
                color: Color32::WHITE,
                italics,
                ..Default::default()
            },
        );
        ctx.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let plain = layout(false);
    let slanted = layout(true);
    let vertices = |galley: &egui::Galley| -> Vec<Pos2> {
        galley
            .rows
            .iter()
            .flat_map(|row| row.row.visuals.mesh.vertices.iter().map(|v| v.pos))
            .collect()
    };
    let before = vertices(&plain);
    let after = vertices(&slanted);
    if before.len() != after.len() {
        return Err("italic must keep the same glyph topology".into());
    }
    if before.is_empty() {
        return Err("italic probe text produced no glyph vertices".into());
    }
    let mut slanted_x = false;
    for (a, b) in before.iter().zip(after.iter()) {
        if (a.y - b.y).abs() > 0.01 {
            return Err("italic must preserve glyph baselines".into());
        }
        if (a.x - b.x).abs() > 0.1 {
            slanted_x = true;
        }
    }
    if !slanted_x {
        return Err("italic glyphs must slant".into());
    }
    Ok(())
}

/// The chrome hint for an unreadable diagram path.
pub fn missing_hint(path: &str) -> String {
    strings::missing_file(path)
}
