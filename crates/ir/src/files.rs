//! Port of `love/uml/adapters/project_files.lua` over POSIX paths
//! (`posix_path.lua`) and atomic writes (`atomic_file.lua`): canonicalize
//! with `realpath` semantics, enumerate sorted entries, refuse source
//! symlinks, and replace outputs atomically via a same-directory rename.

use std::path::Path;

use edn::Value;

use crate::graph::Scanner;

/// A canonical source file and its raw bytes.
pub struct SourceFile {
    pub path: String,
    pub source: Vec<u8>,
}

/// The IO capability handed to IR orchestration; rooted at the canonical
/// project root.
pub struct Adapter {
    root: String,
}

fn canonical(path: &str) -> Result<String, String> {
    if path.contains('\0') {
        return Err(format!("invalid path: {path}"));
    }
    std::fs::canonicalize(path)
        .map_err(|e| format!("cannot canonicalize path: {path}: {e}"))
        .map(|p| p.to_string_lossy().into_owned())
}

fn absolute(root: &str, path: &str) -> String {
    if path.starts_with('/') {
        return path.to_owned();
    }
    format!("{root}/{path}")
}

/// Sorted directory entries; symlinks are refused like `lfs` mode `link`.
fn entries(path: &str) -> Result<Vec<(String, bool)>, String> {
    let mut result = Vec::new();
    let read = std::fs::read_dir(path).map_err(|e| format!("{path}: {e}"))?;
    for entry in read {
        let entry = entry.map_err(|e| format!("{path}: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let full = format!("{path}/{name}");
        let meta = std::fs::symlink_metadata(&full).map_err(|e| format!("{full}: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!("source symbolic link unsupported: {full}"));
        }
        result.push((name, meta.is_dir()));
    }
    result.sort();
    Ok(result)
}

/// `atomic_file.write`: temp file next to the target, then rename. The
/// destination is never deleted first; a failure keeps the old contents.
fn atomic_write(path: &str, bytes: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent()
        && parent != Path::new("")
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temporary = format!("{path}.uml{}.tmp", std::process::id());
    let outcome = std::fs::write(&temporary, bytes)
        .map_err(|e| e.to_string())
        .and_then(|()| std::fs::rename(&temporary, path).map_err(|e| e.to_string()));
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    outcome
}

/// `snapshot`'s walk: sorted entries, descending into accepted directories
/// inline, matching the Lua recursion order.
fn walk(
    root: &str,
    relative: &str,
    scanner: &dyn Scanner,
    opts: &Value,
    files: &mut Vec<SourceFile>,
) -> Result<(), String> {
    let dir = if relative.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{relative}")
    };
    for (name, directory) in entries(&dir)? {
        let rel = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if directory {
            if scanner.accepts_directory(&rel, opts) {
                walk(root, &rel, scanner, opts, files)?;
            }
        } else if scanner.accepts(&rel, opts) {
            let real = canonical(&format!("{root}/{rel}"))?;
            files.push(SourceFile {
                path: real.clone(),
                source: std::fs::read(&real).map_err(|e| format!("{real}: {e}"))?,
            });
        }
    }
    Ok(())
}

impl Adapter {
    /// `new(root, host)`: the root is canonicalized once.
    pub fn new(root: &str) -> Result<Adapter, String> {
        Ok(Adapter {
            root: canonical(root)?,
        })
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let target = absolute(&self.root, path);
        std::fs::read(&target).map_err(|e| format!("{target}: {e}"))
    }

    pub fn write(&self, path: &str, text: &str) -> Result<(), String> {
        atomic_write(&absolute(&self.root, path), text)
    }

    /// `snapshot(path, scanner, opts)`: walks the canonical source root,
    /// collecting accepted files, then explicitly named metadata paths.
    pub fn snapshot(
        &self,
        path: &str,
        scanner: &dyn Scanner,
        opts: &Value,
    ) -> Result<(String, Vec<SourceFile>), String> {
        let canonical_root = canonical(&absolute(&self.root, path))?;
        let mut files = Vec::new();
        walk(&canonical_root, "", scanner, opts, &mut files)?;
        let mut seen: std::collections::HashSet<String> =
            files.iter().map(|f| f.path.clone()).collect();
        for path in scanner.metadata_paths(&canonical_root) {
            if let Ok(source) = std::fs::read(&path) {
                let real = canonical(&path)?;
                if seen.insert(real.clone()) {
                    files.push(SourceFile { path: real, source });
                }
            }
        }
        Ok((canonical_root, files))
    }

    /// `filenames()`: project-relative paths under the root, skipping VCS and
    /// tool directories; only membership is used (language detection).
    pub fn filenames(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut stack = vec![String::new()];
        while let Some(path) = stack.pop() {
            let dir = if path.is_empty() {
                self.root.clone()
            } else {
                format!("{}/{}", self.root, path)
            };
            for (name, directory) in entries(&dir)? {
                let rel = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}/{name}")
                };
                if directory {
                    if !matches!(
                        name.as_str(),
                        ".git" | "node_modules" | ".uml-viewer" | ".claude"
                    ) {
                        stack.push(rel);
                    }
                } else {
                    names.push(rel);
                }
            }
        }
        Ok(names)
    }
}
