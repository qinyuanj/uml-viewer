//! Driver evidence files (port of the `publish()` in
//! `love/uml/adapters/viewer.lua`): `scene-<revision>.edn` holds the encoded
//! scene with no trailing newline; `render.png` is captured at frame 4 and
//! `renderer.txt` records the wgpu adapter (the D3D12/llvmpipe evidence).

use edn::Value;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Evidence {
    pub dir: PathBuf,
}

impl Evidence {
    pub fn new(dir: PathBuf) -> Evidence {
        Evidence { dir }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// `scene-<revision>.edn`: the encoded scene, exactly `E.encode` bytes.
    pub fn publish(&self, scene: &Value, revision: u64) -> io::Result<PathBuf> {
        let target = self.path(&format!("scene-{revision}.edn"));
        fs::write(&target, edn::encode(scene).unwrap_or_default())?;
        Ok(target)
    }

    pub fn write_text(&self, name: &str, text: &str) -> io::Result<PathBuf> {
        let target = self.path(name);
        fs::write(&target, text)?;
        Ok(target)
    }
}
