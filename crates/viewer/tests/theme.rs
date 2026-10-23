//! Theme parity with `love/uml/domain/theme.lua`: palette, ramps and the
//! missing-means-grade-1 rule that paints unmeasured classes red.

use viewer::theme::{self, Rgb};

#[test]
fn quality_ramps_match_the_lua_breakpoints() {
    // Missing metrics enter the ramps as grade 1 (red family).
    assert_eq!(Rgb(36, 52, 48), theme::fill(None));
    assert_eq!(Rgb(90, 110, 100), theme::stroke(None));
    // grade 1 -> 1/5 of the way from the red end to the midpoint
    assert_eq!(Rgb(71, 44, 24), theme::fill(Some(1.0)));
    assert_eq!(Rgb(222, 136, 77), theme::stroke(Some(1.0)));
    // grade 5 -> midpoint
    assert_eq!(Rgb(61, 58, 24), theme::fill(Some(5.0)));
    assert_eq!(Rgb(212, 192, 90), theme::stroke(Some(5.0)));
    // grade 10 -> green end
    assert_eq!(Rgb(30, 74, 56), theme::fill(Some(10.0)));
    assert_eq!(Rgb(95, 181, 138), theme::stroke(Some(10.0)));
}

#[test]
fn band_colors_are_the_traffic_light() {
    assert_eq!(Rgb(196, 42, 36), theme::band_color("red"));
    assert_eq!(Rgb(232, 196, 72), theme::band_color("yellow"));
    assert_eq!(Rgb(95, 181, 138), theme::band_color("green"));
    // Unknown bands are red, matching the Lua fallback.
    assert_eq!(Rgb(196, 42, 36), theme::band_color("bogus"));
}

#[test]
fn fade_scales_alpha_but_keeps_the_hue() {
    let dim = egui::Color32::from_rgb(200, 214, 204);
    let faded = theme::fade(dim);
    assert_eq!((200, 214, 204), (faded.r(), faded.g(), faded.b()));
    assert_eq!(theme::ALPHA_DIMMED, faded.a());
}

/// WCAG 2 contrast ratio between two opaque colors.
fn contrast(a: Rgb, b: Rgb) -> f64 {
    let lum = |c: Rgb| {
        let ch = |v: u8| {
            let s = v as f64 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn source_window_is_dark_and_readable() {
    // The source window joins the dark theme instead of a light island.
    assert_eq!(theme::PANEL, theme::SOURCE_BG);
    for ink in [
        theme::SOURCE_INK,
        theme::SOURCE_KEYWORD,
        theme::SOURCE_STRING,
    ] {
        assert!(contrast(ink, theme::SOURCE_BG) >= 4.5, "{ink:?}");
        assert!(
            contrast(ink, theme::SOURCE_SELECTION) >= 4.5,
            "{ink:?} on selection"
        );
    }
    assert!(contrast(theme::SOURCE_COMMENT, theme::SOURCE_BG) >= 3.0);
    assert!(contrast(theme::SOURCE_FILE, theme::SOURCE_BG) >= 4.5);
    assert!(contrast(theme::SOURCE_GUTTER, theme::SOURCE_GUTTER_BG) >= 4.5);
}
