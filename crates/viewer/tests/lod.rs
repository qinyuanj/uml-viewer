//! Three-tier zoom LOD and viewport culling (port of the #131 egui
//! prototype's rules, ADR 0008): far zoom draws block silhouettes, mid zoom
//! draws frames with title text and decimated edges, near zoom is the full
//! class-box detail.

use viewer::lod::{self, Tier};

#[test]
fn zoom_selects_one_of_three_tiers() {
    assert_eq!(Tier::Blocks, lod::tier(0.1));
    assert_eq!(Tier::Blocks, lod::tier(0.149));
    assert_eq!(Tier::Frames, lod::tier(0.15));
    assert_eq!(Tier::Frames, lod::tier(0.399));
    assert_eq!(Tier::Full, lod::tier(0.4));
    assert_eq!(Tier::Full, lod::tier(1.0));
    assert_eq!(Tier::Full, lod::tier(10.0));
}

#[test]
fn culling_keeps_only_rects_overlapping_the_viewport() {
    let view = scene::geom::Rect::new(0.0, 0.0, 100.0, 100.0);
    assert!(lod::visible(
        scene::geom::Rect::new(50.0, 50.0, 10.0, 10.0),
        view
    ));
    assert!(lod::visible(
        scene::geom::Rect::new(-5.0, -5.0, 10.0, 10.0),
        view
    ));
    assert!(!lod::visible(
        scene::geom::Rect::new(200.0, 50.0, 10.0, 10.0),
        view
    ));
    assert!(!lod::visible(
        scene::geom::Rect::new(50.0, -50.0, 10.0, 10.0),
        view
    ));
}

#[test]
fn decimation_drops_points_closer_than_the_budget_but_keeps_the_ends() {
    let line = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [20.0, 0.0], [21.0, 0.0]];
    let out = lod::decimate(&line, 8.0);
    assert_eq!(vec![[0.0, 0.0], [20.0, 0.0], [21.0, 0.0]], out);
    // A polyline already sparser than the budget survives untouched.
    let sparse = vec![[0.0, 0.0], [10.0, 0.0], [20.0, 5.0]];
    assert_eq!(sparse, lod::decimate(&sparse, 8.0));
    // Degenerate inputs are safe.
    assert_eq!(Vec::<[f64; 2]>::new(), lod::decimate(&[], 8.0));
    assert_eq!(vec![[3.0, 4.0]], lod::decimate(&[[3.0, 4.0]], 8.0));
}

#[test]
fn font_px_quantizes_to_whole_points() {
    assert_eq!(9.0, lod::font_px(9.0, 1.0));
    assert_eq!(14.0, lod::font_px(9.4, 1.5));
    assert_eq!(1.0, lod::font_px(0.01, 0.5));
}
