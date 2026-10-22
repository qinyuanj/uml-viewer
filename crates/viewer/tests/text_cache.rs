//! Canvas text layout cache (#143): integer font sizes repeat while zooming,
//! so a label laid out at a size is reused on later frames instead of being
//! laid out again whenever egui's per-frame galley cache drops it.

use egui::Color32;
use viewer::paint::TextCache;

fn frame(ctx: &egui::Context, run: impl FnMut(&egui::Context)) {
    let mut run = run;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| run(ui.ctx()));
    // Headless: no renderer applies the font texture.
    out.textures_delta.clear();
}

#[test]
fn a_label_at_a_size_is_laid_out_once_across_frames() {
    let ctx = egui::Context::default();
    let mut cache = TextCache::default();
    for _ in 0..3 {
        frame(&ctx, |ctx| {
            cache.galley(ctx, "Alpha", 14.0, Color32::WHITE, false, false);
            cache.width(ctx, "Alpha", 14.0);
        });
    }
    assert_eq!(2, cache.layouts());
}

#[test]
fn a_new_size_or_style_is_a_new_layout() {
    let ctx = egui::Context::default();
    let mut cache = TextCache::default();
    frame(&ctx, |ctx| {
        cache.galley(ctx, "Alpha", 14.0, Color32::WHITE, false, false);
        cache.galley(ctx, "Alpha", 15.0, Color32::WHITE, false, false);
        cache.galley(ctx, "Alpha", 15.0, Color32::WHITE, true, false);
        cache.galley(ctx, "Alpha", 15.0, Color32::GOLD, true, false);
    });
    assert_eq!(4, cache.layouts());
}
