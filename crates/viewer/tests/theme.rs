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
