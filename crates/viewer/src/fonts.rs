//! CJK font provisioning for egui (port of the lookup in
//! `love/uml/adapters/fonts.lua`): `UML_CJK_FONT` when non-empty, then the
//! project-provisioned Sarasa Mono SC, then the Ubuntu Noto CJK fallback.

use std::path::{Path, PathBuf};

pub const SARASA: &str = "SarasaMonoSC-Regular.ttf";
pub const SYSTEM_FONTS: [&str; 2] = [
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
];

/// Resolve the font with an injected environment value and candidate list.
pub fn resolve_with(
    env: Option<&str>,
    root: &Path,
    candidates: &[&str],
) -> Option<(PathBuf, &'static str)> {
    if let Some(path) = env.filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some((path, "env"));
        }
        eprintln!("uml viewer: UML_CJK_FONT not found: {}", path.display());
    }
    let provisioned = root.join(".uml-viewer/fonts").join(SARASA);
    if provisioned.is_file() {
        return Some((provisioned, "sarasa"));
    }
    for candidate in candidates {
        let path = Path::new(candidate);
        if path.is_file() {
            eprintln!(
                "uml viewer: Sarasa Mono SC not found at {}; falling back to {}. Run get-uml-viewer to provision the font.",
                provisioned.display(),
                path.display()
            );
            return Some((path.to_path_buf(), "system"));
        }
    }
    None
}

pub fn resolve(root: &Path) -> Option<(PathBuf, &'static str)> {
    let env = std::env::var("UML_CJK_FONT").ok();
    resolve_with(env.as_deref(), root, &SYSTEM_FONTS)
}

/// Install the resolved font into egui. Glyphs rasterise into the atlas on
/// demand, at the size and pixels-per-point they are drawn with, so CJK text
/// needs no prebuilt atlas. Returns the chosen path and its source (`env`,
/// `sarasa`, `system`) so the viewer can report it.
pub fn install(ctx: &egui::Context, root: &Path) -> Option<(PathBuf, &'static str)> {
    let (path, source) = resolve(root)?;
    let bytes = std::fs::read(&path).ok()?;
    let mut font = egui::FontData::from_owned(bytes);
    // .ttc collections: face 2 is Simplified Chinese in the Ubuntu Noto
    // collections (0 JP, 1 KR, 2 SC, 3 TC, 4 HK).
    if path.extension().is_some_and(|e| e == "ttc") {
        font.index = 2;
    }
    ctx.set_fonts(definitions(font));
    Some((path, source))
}

/// egui's fonts with `font` first for monospace text and as a fallback for
/// proportional text. Canvas text is monospace and the layout sizes boxes
/// for Sarasa's half-width Latin, so egui's wider built-in monospace face
/// must not draw it.
pub fn definitions(font: egui::FontData) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("uml_cjk".into(), std::sync::Arc::new(font));
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "uml_cjk".into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("uml_cjk".into());
    fonts
}
