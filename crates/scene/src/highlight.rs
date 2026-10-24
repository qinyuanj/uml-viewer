//! Relationship highlighting (port of `love/uml/application/highlight.lua`):
//! hit maps and scene edges in, ids and predicates out.

use edn::Value;
use edn::record::{get, local_name};
use std::collections::HashSet;

/// The class a hit stands for: the class itself, or the parent of a child
/// row or port. Packages, edges and dependency triangles focus no class.
pub fn box_id(hit: Option<&Value>) -> Option<Value> {
    let hit = hit?;
    match get(hit, "kind").map(crate::geom::name).as_deref() {
        Some("class") => get(hit, "id").cloned(),
        Some("child") | Some("port") => get(hit, "parent").cloned(),
        _ => None,
    }
}

pub fn touches(e: &Value, id: Option<&Value>) -> bool {
    let Some(id) = id else {
        return false;
    };
    if get(e, "from") == Some(id) || get(e, "to") == Some(id) {
        return true;
    }
    match get(e, "via-ids") {
        Some(Value::Vector(via)) => via.iter().any(|v| v == id),
        _ => false,
    }
}

fn same_edge(e: &Value, hit: Option<&Value>) -> bool {
    let Some(hit) = hit else {
        return false;
    };
    get(hit, "kind").and_then(local_name) == Some("edge")
        && get(e, "from") == get(hit, "from")
        && get(e, "to") == get(hit, "to")
}

/// Focus state derived from selection and hover.
pub struct Highlight {
    pub focus: Option<Value>,
    pub hover: Option<Value>,
    related: HashSet<String>,
}

fn key(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| edn::encode(v).ok())
}

impl Highlight {
    /// Build from a scene and a `{selected, hover}` view record.
    pub fn new(scene: &Value, selected: Option<&Value>, hover: Option<&Value>) -> Highlight {
        let focus = box_id(selected);
        let hover = box_id(hover);
        let mut related = HashSet::new();
        if focus.is_some() {
            for e in edn::record::seq(get(scene, "edges")) {
                if touches(e, focus.as_ref()) {
                    if let Some(k) = key(get(e, "from")) {
                        related.insert(k);
                    }
                    if let Some(k) = key(get(e, "to")) {
                        related.insert(k);
                    }
                }
            }
        }
        Highlight {
            focus,
            hover,
            related,
        }
    }

    /// A class connected to the focus, but not the focus itself.
    pub fn related(&self, id: &Value) -> bool {
        self.focus.as_ref().is_some_and(|f| f != id)
            && key(Some(id)).is_some_and(|k| self.related.contains(&k))
    }

    /// A class unrelated to the focus (dimmed).
    pub fn dimmed(&self, id: &Value) -> bool {
        self.focus.is_some()
            && self.focus.as_ref() != Some(id)
            && !key(Some(id)).is_some_and(|k| self.related.contains(&k))
    }

    /// An edge on the selection or the hovered edge.
    pub fn hot(&self, e: &Value, hovered: Option<&Value>) -> bool {
        touches(e, self.focus.as_ref()) || same_edge(e, hovered)
    }

    /// An edge touching the hovered class.
    pub fn warm(&self, e: &Value) -> bool {
        touches(e, self.hover.as_ref())
    }
}
