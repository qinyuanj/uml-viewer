//! Theme parity with `love/uml/domain/theme.lua`: palette, ramps and the
//! missing-means-grade-1 rule that paints unmeasured classes red.

mod common;

use common::{composite, contrast};
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
fn translucent_colors_composite_at_their_nominal_alpha() {
    let background = theme::SOURCE_BG;
    let original = Rgb(200, 214, 204);
    let dimmed = composite(background, theme::fade(original.color32()));
    let washed = composite(background, theme::GOLD.with_alpha(theme::ALPHA_ROW_WASH));

    assert!(
        dimmed.0 < original.0 && dimmed.1 < original.1 && dimmed.2 < original.2,
        "dimming must darken, got {dimmed:?}"
    );
    assert!(
        washed.0 < theme::GOLD.0 && washed.1 < theme::GOLD.1 && washed.2 < theme::GOLD.2,
        "a wash must not become solid gold, got {washed:?}"
    );
    assert_eq!(
        composite(background, theme::fade(original.color32())),
        composite(background, original.with_alpha(theme::ALPHA_DIMMED)),
        "fade and an equally alpha'd opaque color must composite alike"
    );
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
