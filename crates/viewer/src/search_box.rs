//! Ctrl+F search box state (port of the search part of
//! `love/uml/adapters/inspector.lua`). Matching itself is
//! `scene::search::search`; this module owns what the viewer adds around it:
//! open/close, explicit Down/Up selection, forgetting the selection when a new
//! document loads, and the view-unit area that captures canvas input while
//! the box is open. egui draws it (`app.rs`); nothing here touches egui.

use edn::Value;
use scene::geom::{Rect, name};
use scene::search::{self, Match};

use crate::driver::SearchRecord;

#[derive(Debug, Clone, Default)]
pub struct SearchBox {
    pub active: bool,
    pub query: String,
    /// Explicit selection; None until Down/Up is pressed.
    pub selection: Option<Value>,
    /// Ask the text field for keyboard focus on its next draw.
    pub focus_request: bool,
    /// Drawn area and result rows of the last frame, in view units.
    pub area: Option<Rect>,
    pub rows: Vec<(Rect, Value)>,
    revision: Option<u64>,
}

impl SearchBox {
    pub fn open(&mut self) {
        self.active = true;
        self.focus_request = true;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.focus_request = false;
        self.area = None;
        self.rows.clear();
    }

    pub fn results(&self, doc: Option<&Value>) -> Vec<Match> {
        doc.map(|d| search::search(d, &self.query))
            .unwrap_or_default()
    }

    /// A different document revision forgets the selection (Lua
    /// `search_document ~= state.document`).
    pub fn sync(&mut self, revision: u64) {
        if self.revision != Some(revision) {
            self.revision = Some(revision);
            self.selection = None;
        }
    }

    /// Down: `min(n, (index or 0) + 1)`; Up: `max(1, (index or 2) - 1)`.
    pub fn step(&mut self, down: bool, results: &[Match]) {
        if results.is_empty() {
            return;
        }
        let index = self
            .selection
            .as_ref()
            .and_then(|id| results.iter().position(|m| &m.id == id))
            .map(|i| i + 1);
        let next = if down {
            results.len().min(index.unwrap_or(0) + 1)
        } else {
            1.max(index.unwrap_or(2) - 1)
        };
        self.selection = Some(results[next - 1].id.clone());
    }

    /// While open, its drawn area takes pointer input from the canvas.
    pub fn captures(&self, x: f64, y: f64) -> bool {
        self.active && self.area.is_some_and(|r| contains(r, x, y))
    }

    pub fn row_at(&self, x: f64, y: f64) -> Option<Value> {
        if !self.active {
            return None;
        }
        self.rows
            .iter()
            .find(|(r, _)| contains(*r, x, y))
            .map(|(_, id)| id.clone())
    }

    pub fn record(&self, results: &[Match]) -> SearchRecord {
        SearchRecord {
            active: self.active,
            query: self.query.clone(),
            results: results.iter().map(|m| name(&m.id)).collect(),
            selection: self.selection.as_ref().map(name),
        }
    }
}

fn contains(r: Rect, x: f64, y: f64) -> bool {
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
}
