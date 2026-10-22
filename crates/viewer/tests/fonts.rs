//! CJK font lookup order and the egui italic glyph probe (the honest egui
//! equivalent of the cimgui vertex-slant assertion in
//! `scripts/test_love_class_boxes.lua`).

use std::path::Path;

#[test]
fn an_explicit_env_font_wins_when_it_exists() {
    let dir = std::env::temp_dir().join(format!("viewer-fonts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let font = dir.join("explicit.ttf");
    std::fs::write(&font, b"not a real font").unwrap();
    let (path, source) =
        viewer::fonts::resolve_with(Some(font.to_str().unwrap()), &dir, &["/does/not/exist.ttf"])
            .unwrap();
    assert_eq!(font, path);
    assert_eq!("env", source);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_provisioned_sarasa_beats_the_system_fallback() {
    let dir = std::env::temp_dir().join(format!("viewer-fonts-sarasa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".uml-viewer/fonts")).unwrap();
    let sarasa = dir.join(".uml-viewer/fonts/SarasaMonoSC-Regular.ttf");
    std::fs::write(&sarasa, b"x").unwrap();
    let system = dir.join("system.ttc");
    std::fs::write(&system, b"y").unwrap();
    let (path, source) =
        viewer::fonts::resolve_with(None, &dir, &[system.to_str().unwrap()]).unwrap();
    assert_eq!(sarasa, path);
    assert_eq!("sarasa", source);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn egui_italics_shear_real_glyph_vertices_and_preserve_baselines() {
    let ctx = egui::Context::default();
    // Fonts are installed on the first pass, as in the running window.
    ctx.begin_pass(egui::RawInput::default());
    ctx.end_pass().textures_delta.clear();
    viewer::paint::italic_glyph_probe(&ctx).expect("default fonts must provide slanted glyphs");
}

#[test]
fn the_probe_reports_the_reason_when_the_text_is_empty() {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    ctx.end_pass().textures_delta.clear();
    assert!(viewer::paint::italic_glyph_probe_text(&ctx, "").is_err());
    let _ = Path::new("/");
}
