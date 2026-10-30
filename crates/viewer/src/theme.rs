//! Single viewer theme (port of `love/uml/domain/theme.lua`): one dark
//! palette, screen-px line widths and the continuous quality ramps shared by
//! the canvas, edges and chrome. egui works in points; `pixels_per_point`
//! plays the role of the Lua `ui` DPI factor, applied at paint time.

use egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn color32(self) -> Color32 {
        Color32::from_rgb(self.0, self.1, self.2)
    }

    pub fn with_alpha(self, a: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.0, self.1, self.2, a)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowMarker {
    pub wash: Color32,
    pub bar: Color32,
    pub bar_width: f32,
}

/// Shared target/member/hover row marker: a translucent gold wash and an
/// opaque gold rule at the row's left edge.
pub fn row_marker() -> RowMarker {
    RowMarker {
        wash: GOLD.with_alpha(ALPHA_ROW_WASH),
        bar: GOLD.color32(),
        bar_width: 3.0,
    }
}

pub mod fonts {
    pub const BODY: f32 = 18.0;
    pub const TITLE: f32 = 18.0;
    /// Canvas class, member and package text: the size the layout's 8px a
    /// char estimate sizes boxes for, so labels draw at one size.
    pub const CANVAS: f32 = 16.0;
    pub const BADGE: f32 = 9.0;
    pub const LEVEL: f32 = 11.0;
    pub const MARKER: f32 = 12.0;
    pub const CODE: f32 = 13.0;
    pub const PORT: f32 = 10.0;
}

pub mod radius {
    pub const CLASS: f32 = 4.0;
    pub const PACKAGE: f32 = 8.0;
}

/// Baseline point widths at DPI scale 1: never multiplied by canvas zoom.
pub mod widths {
    pub const CLASS: f32 = 2.5;
    pub const PACKAGE: f32 = 2.0;
    pub const SELECTED: f32 = 3.5;
    pub const EDGE: f32 = 1.6;
    pub const EDGE_HOT: f32 = 2.8;
    pub const EDGE_VIOLATING: f32 = 2.4;
    pub const EDGE_VIOLATING_HOT: f32 = 3.2;
    pub const EDGE_WARM: f32 = 2.1;
    pub const BAND: f32 = 2.0;
    pub const RULE: f32 = 1.0;
    pub const DOT: f32 = 1.0;
    pub const DASH: f32 = 6.0;
    pub const DASH_GAP: f32 = 4.0;
    pub const PORT: f32 = 1.0;
    pub const PORT_ON: f32 = 2.2;
    pub const PORT_LINK: f32 = 1.3;
}

pub const CANVAS_BG: Rgb = Rgb(22, 28, 32);
pub const PANEL: Rgb = Rgb(26, 36, 40);
pub const SIDEBAR: Rgb = Rgb(18, 22, 24);
pub const LINE: Rgb = Rgb(42, 61, 54);
pub const INK: Rgb = Rgb(236, 236, 228);
pub const WHITE: Rgb = Rgb(255, 255, 255);
pub const MUTED: Rgb = Rgb(157, 184, 168);
pub const GOLD: Rgb = Rgb(232, 196, 72);
pub const GOOD: Rgb = Rgb(95, 181, 138);
pub const VIOLATION: Rgb = Rgb(196, 42, 36);
pub const VIOLATION_HOT: Rgb = Rgb(255, 64, 48);
pub const WARN: Rgb = Rgb(224, 122, 74);
pub const BAND_RED: Rgb = Rgb(196, 42, 36);
pub const BAND_YELLOW: Rgb = Rgb(232, 196, 72);
pub const BAND_GREEN: Rgb = Rgb(95, 181, 138);
pub const EDGE: Rgb = Rgb(157, 184, 168);
pub const EDGE_VIOLATING: Rgb = Rgb(196, 42, 36);
pub const EDGE_VIOLATING_HOT: Rgb = Rgb(255, 64, 48);
pub const EDGE_WARM: Rgb = Rgb(200, 214, 204);
pub const EDGE_VIOLATING_WARM: Rgb = Rgb(220, 56, 44);
pub const INDICATOR: Rgb = Rgb(157, 184, 168);
/// The dependency chain: a blue no quality grade or plain edge uses.
pub const RELATED: Rgb = Rgb(62, 152, 236);
pub const CHILD_WASH: Rgb = Rgb(232, 196, 72);
pub const PORT_FILL: Rgb = Rgb(42, 56, 52);
pub const PORT_FILL_ON: Rgb = Rgb(74, 96, 78);
pub const POPUP_BG: Rgb = Rgb(18, 22, 24);
// Sidebar buttons and the dark source window. Its target line is a GOLD
// wash (ALPHA_ROW_WASH); tokens avoid the red/green quality hues.
pub const BUTTON_HOVER: Rgb = Rgb(54, 76, 66);
pub const BUTTON_ACTIVE: Rgb = Rgb(66, 90, 76);
pub const SOURCE_BG: Rgb = PANEL;
pub const SOURCE_INK: Rgb = INK;
pub const SOURCE_GUTTER_BG: Rgb = SIDEBAR;
pub const SOURCE_GUTTER: Rgb = MUTED;
pub const SOURCE_SELECTION: Rgb = Rgb(38, 54, 70);
pub const SOURCE_COMMENT: Rgb = Rgb(120, 140, 130);
pub const SOURCE_STRING: Rgb = Rgb(230, 160, 110);
pub const SOURCE_KEYWORD: Rgb = Rgb(122, 162, 247);

pub const ALPHA_PACKAGE_FILL: u8 = 80;
pub const ALPHA_PACKAGE_STROKE: u8 = 170;
pub const ALPHA_DIMMED: u8 = 70;
pub const ALPHA_CHILD_WASH: u8 = 55;
pub const ALPHA_POPUP: u8 = 240;
/// Gold wash over the hovered class-card row.
pub const ALPHA_ROW_WASH: u8 = 32;

/// Dim all premultiplied channels together; changing alpha alone would
/// make an opaque color brighter when blended.
pub fn fade(color: Color32) -> Color32 {
    color.gamma_multiply_u8(ALPHA_DIMMED)
}

pub fn band_color(band: &str) -> Rgb {
    match band {
        "yellow" => BAND_YELLOW,
        "green" => BAND_GREEN,
        _ => BAND_RED,
    }
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let channel = |x: u8, y: u8| (x as f64 + t * (y as f64 - x as f64) + 0.5).floor() as u8;
    Rgb(channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

fn ramp(g: Option<f64>, none: Rgb, red: Rgb, mid: Rgb, green: Rgb) -> Rgb {
    match g {
        None => none,
        Some(g) if g <= 5.0 => mix(red, mid, g / 5.0),
        Some(g) => mix(mid, green, ((g - 5.0) / 5.0).min(1.0)),
    }
}

/// Continuous 0-10 grade ramps (draw.clj fill-for / stroke-for).
pub fn fill(grade: Option<f64>) -> Rgb {
    ramp(
        grade,
        Rgb(36, 52, 48),
        Rgb(74, 40, 24),
        Rgb(61, 58, 24),
        Rgb(30, 74, 56),
    )
}

pub fn stroke(grade: Option<f64>) -> Rgb {
    ramp(
        grade,
        Rgb(90, 110, 100),
        Rgb(224, 122, 74),
        Rgb(212, 192, 90),
        Rgb(95, 181, 138),
    )
}

/// Line coverage ink: >=80% good, >=50% gold, else warn; unknown muted.
pub fn coverage(p: Option<f64>) -> Rgb {
    match p {
        None => MUTED,
        Some(p) if p >= 0.8 => GOOD,
        Some(p) if p >= 0.5 => GOLD,
        _ => WARN,
    }
}
