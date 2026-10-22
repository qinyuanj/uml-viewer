//! Namespace and proposal views (port of `love/uml/domain/hierarchy.lua`).
//! Functions consume and return tagged EDN; inputs are never mutated.

use crate::config;
use crate::geom::{self, name};
use crate::policy;
use edn::Value;
use std::collections::HashMap;

fn get<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    geom::get(m, key)
}

fn seq(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Vector(items)) => items,
        _ => &[],
    }
}

fn id(x: &Value) -> Value {
    Value::keyword(&name(x))
}

fn contains(xs: &[Value], x: &Value) -> bool {
    xs.iter().any(|v| v == x)
}

fn path_parts(x: &Value) -> Vec<String> {
    name(x)
        .split('.')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn path_name(path: &[Value]) -> String {
    path.iter().map(name).collect::<Vec<_>>().join(".")
}

fn under(x: &Value, parent: &Value) -> bool {
    let (x, parent) = (name(x), name(parent));
    x == parent || x.starts_with(&format!("{parent}."))
}

/// The node `x` presents as at `path`: itself when it is exactly the focus,
/// its next segment when deeper, None when outside the focus subtree.
fn child(x: &Value, path: &[Value]) -> Option<Value> {
    let ss = path_parts(x);
    if ss.len() < path.len() {
        return None;
    }
    for (s, p) in ss.iter().zip(path) {
        if *s != name(p) {
            return None;
        }
    }
    if ss.len() == path.len() {
        return Some(x.clone());
    }
    Some(Value::keyword(&ss[..path.len() + 1].join(".")))
}

/// Display label: CamelCase of the last dot segment, preserving existing
/// capitals (byte-level ASCII case ops, as in Lua).
fn label(x: &Value) -> String {
    let last = name(x);
    let last = last.rsplit('.').next().unwrap_or("");
    let mut out = String::new();
    for part in last.split('-').filter(|s| !s.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            let rest: String = chars.collect();
            if part.bytes().any(|b| b.is_ascii_uppercase()) {
                out.push_str(&rest);
            } else {
                out.push_str(&rest.to_ascii_lowercase());
            }
        }
    }
    out
}

fn module_name(c: &Value) -> String {
    if get(c, "proposal-group?").is_some() {
        return match get(c, "name") {
            Some(v @ Value::String(_)) => name(v),
            _ => label(get(c, "id").unwrap()),
        };
    }
    label(get(c, "id").unwrap())
}

/// `G.file_base`: trailing path segment after `/` or `\`.
fn file_base(path: &Value) -> Option<String> {
    match path {
        Value::String(s) => {
            let norm = s.replace('\\', "/");
            let trimmed = norm.trim_end_matches('/');
            Some(trimmed.rsplit('/').next().unwrap_or(trimmed).to_owned())
        }
        _ => None,
    }
}

fn box_name(leaf: Option<&Value>, cid: &Value, directory: bool) -> String {
    let base = leaf.and_then(|l| get(l, "file")).and_then(file_base);
    if !directory
        && let Some(base) = &base
        && base.to_ascii_lowercase().ends_with(".rs")
    {
        return base.clone();
    }
    label(cid)
}

fn index(classes: &[Value]) -> HashMap<String, &Value> {
    classes
        .iter()
        .map(|c| (name(get(c, "id").unwrap()), c))
        .collect()
}

fn descendants(cid: &Value, classes: &[Value]) -> bool {
    classes.iter().any(|c| {
        let x = get(c, "id").unwrap();
        x != cid && under(x, cid)
    })
}

/// Distinct child nodes of `path` across the classes (foreign excluded).
pub fn nodes_at(classes: &[Value], path: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for c in classes {
        if get(c, "foreign").is_some() {
            continue;
        }
        if let Some(x) = child(get(c, "id").unwrap(), path)
            && !contains(&out, &x)
        {
            out.push(x);
        }
    }
    out
}

fn contents(
    classes: &[Value],
    path: &[Value],
    cid: &Value,
    idx: &HashMap<String, &Value>,
) -> Option<Vec<Value>> {
    let mut next_path: Vec<Value> = path.to_vec();
    let last = name(cid);
    let last = last.rsplit('.').next().unwrap_or("");
    next_path.push(Value::keyword(last));
    let kids = nodes_at(classes, &next_path);
    let others = kids.iter().any(|x| x != cid);
    if !others {
        return None;
    }
    let mut out = Vec::new();
    for x in &kids {
        let drill = x != cid && descendants(x, classes);
        out.push(Value::Map(vec![
            (Value::keyword("id"), x.clone()),
            (
                Value::keyword("name"),
                Value::String(box_name(idx.get(&name(x)).copied(), x, drill)),
            ),
            (Value::keyword("drill?"), Value::Bool(drill)),
        ]));
    }
    Some(out)
}

fn metrics(out: &mut Value, classes: &[Value], rolled: bool) {
    if let Some(crap) = config::pool_crap(classes, rolled) {
        geom::put(out, "crap", crap);
    }
    if let Some(Value::Map(entries)) = config::pool_mutants(classes) {
        for (k, v) in entries {
            out.assoc(k, v);
        }
    }
    let mut lv: Option<f64> = None;
    for c in classes {
        if let Some(n) = get(c, "level").and_then(geom::num) {
            lv = Some(lv.map_or(n, |m: f64| m.max(n)));
        }
    }
    if let Some(lv) = lv {
        geom::put(out, "level", geom::numval(lv));
    }
}

fn view_class(
    classes: &[Value],
    path: &[Value],
    cid: &Value,
    idx: &HashMap<String, &Value>,
) -> Value {
    let leaf = idx.get(&name(cid)).copied();
    let kids = contents(classes, path, cid, idx);
    let drill = kids.is_some();
    let mut r = Value::Map(vec![
        (Value::keyword("id"), cid.clone()),
        (
            Value::keyword("name"),
            Value::String(box_name(leaf, cid, drill)),
        ),
        (Value::keyword("drill?"), Value::Bool(drill)),
    ]);
    let mut subtree: Vec<Value> = Vec::new();
    let mut lv: Option<f64> = None;
    for c in classes {
        if under(get(c, "id").unwrap(), cid) {
            subtree.push(c.clone());
            if let Some(n) = get(c, "level").and_then(geom::num) {
                lv = Some(lv.map_or(n, |m: f64| m.max(n)));
            }
        }
    }
    for k in [
        "ns",
        "lang",
        "file",
        "stereotype",
        "coverage",
        "ops",
        "fields",
    ] {
        if let Some(leaf) = leaf
            && let Some(v) = get(leaf, k)
        {
            geom::put(&mut r, k, v.clone());
        }
    }
    if let Some(lv) = lv {
        geom::put(&mut r, "level", geom::numval(lv));
    }
    if drill {
        metrics(&mut r, &subtree, false);
        geom::put(&mut r, "contents", Value::Vector(kids.unwrap()));
        geom::put(&mut r, "hide-members", Value::Bool(true));
    } else if let Some(leaf) = leaf {
        if let Some(crap) = get(leaf, "crap")
            && get(crap, "mu").is_some()
        {
            geom::put(&mut r, "crap", crap.clone());
        }
        if get(leaf, "killed").is_some() || get(leaf, "survived").is_some() {
            for k in ["killed", "survived", "uncovered"] {
                if let Some(v) = get(leaf, k) {
                    geom::put(&mut r, k, v.clone());
                }
            }
        }
    }
    r
}

fn external(cid: &Value, path: &[Value], idx: &HashMap<String, &Value>) -> Option<Value> {
    let c = idx.get(&name(cid)).copied();
    if let Some(c) = c
        && get(c, "foreign").is_some()
    {
        return Some(Value::Map(vec![
            (Value::keyword("id"), cid.clone()),
            (
                Value::keyword("name"),
                get(c, "name").cloned().unwrap_or(Value::String(name(cid))),
            ),
            (Value::keyword("foreign"), Value::Bool(true)),
        ]));
    }
    if child(cid, path).is_some() {
        return None;
    }
    let ss = path_parts(cid);
    let keep = ss.len().min(path.len().max(1));
    let x = Value::keyword(&ss[..keep].join("."));
    Some(Value::Map(vec![
        (Value::keyword("id"), x.clone()),
        (Value::keyword("name"), Value::String(label(&x))),
        (Value::keyword("external"), Value::Bool(true)),
    ]))
}

fn collapse(cid: &Value, path: &[Value], idx: &HashMap<String, &Value>) -> Option<Value> {
    if let Some(c) = idx.get(&name(cid))
        && get(c, "foreign").is_some()
    {
        return Some(cid.clone());
    }
    child(cid, path)
}

fn port(ports: &mut HashMap<String, Vec<Value>>, cid: &Value, dep: &Value) {
    let entry = ports.entry(name(cid)).or_default();
    let dep_id = get(dep, "id");
    if entry.iter().any(|p| get(p, "id") == dep_id) {
        return;
    }
    entry.push(Value::Map(vec![
        (Value::keyword("id"), dep_id.cloned().unwrap_or(Value::Nil)),
        (
            Value::keyword("name"),
            get(dep, "name").cloned().unwrap_or(Value::Nil),
        ),
    ]));
}

/// The source-tree view at a namespace focus.
pub fn view_at(doc: &Value, path: &[Value]) -> Value {
    let classes: Vec<Value> = seq(get(doc, "classes"))
        .iter()
        .filter(|c| !policy::omitted_id(get(c, "id").unwrap(), get(doc, "omit")))
        .cloned()
        .collect();
    let idx = index(&classes);
    let mut nodes = nodes_at(&classes, path);
    let order = get(doc, "order");
    let specific = match order {
        Some(Value::Map(_)) => order.and_then(|o| o.get(&Value::String(path_name(path)))),
        _ => None,
    };
    let order = match specific {
        Some(s) => Some(s),
        None => {
            if path.is_empty() {
                order
            } else {
                None
            }
        }
    };
    let mut ranks: HashMap<String, usize> = HashMap::new();
    for (i, x) in seq(order).iter().enumerate() {
        ranks.insert(name(x), i + 1);
    }
    nodes.sort_by(|a, b| {
        let ar = ranks.get(&name(a)).copied().unwrap_or(usize::MAX);
        let br = ranks.get(&name(b)).copied().unwrap_or(usize::MAX);
        if ar != br {
            ar.cmp(&br)
        } else {
            name(a).cmp(&name(b))
        }
    });
    let boxes: Vec<Value> = nodes
        .iter()
        .map(|cid| view_class(&classes, path, cid, &idx))
        .collect();
    let (mut internal, mut foreign, mut foreign_edges) = (Vec::new(), Vec::new(), Vec::new());
    let (mut incoming, mut outgoing): (HashMap<String, Vec<Value>>, HashMap<String, Vec<Value>>) =
        (HashMap::new(), HashMap::new());
    for e in seq(get(doc, "edges")) {
        let f = collapse(get(e, "from").unwrap(), path, &idx);
        let t = collapse(get(e, "to").unwrap(), path, &idx);
        let fh = f.as_ref().is_some_and(|f| contains(&nodes, f));
        let th = t.as_ref().is_some_and(|t| contains(&nodes, t));
        let te = if th {
            None
        } else {
            external(get(e, "to").unwrap(), path, &idx)
        };
        let fe = if fh {
            None
        } else {
            external(get(e, "from").unwrap(), path, &idx)
        };
        if fh && th && f != t {
            let mut out = e.clone();
            geom::put(&mut out, "from", f.unwrap());
            geom::put(&mut out, "to", t.unwrap());
            internal.push(out);
        } else if fh && te.as_ref().is_some_and(|te| get(te, "foreign").is_some()) {
            let te = te.unwrap();
            let fid = get(&te, "id").cloned().unwrap();
            let mut out = e.clone();
            geom::put(&mut out, "from", f.clone().unwrap());
            geom::put(&mut out, "to", fid.clone());
            foreign_edges.push(out);
            if !foreign.iter().any(|c| get(c, "id") == Some(&fid)) {
                foreign.push(Value::Map(vec![
                    (Value::keyword("id"), fid),
                    (
                        Value::keyword("name"),
                        get(&te, "name").cloned().unwrap_or(Value::Nil),
                    ),
                    (Value::keyword("foreign"), Value::Bool(true)),
                    (Value::keyword("shape"), Value::keyword("oval")),
                ]));
            }
        } else if fh && let Some(te) = te {
            port(&mut outgoing, &f.unwrap(), &te);
        } else if th && fe.as_ref().is_some_and(|fe| get(fe, "foreign").is_some()) {
            let fe = fe.unwrap();
            let fid = get(&fe, "id").cloned().unwrap();
            let mut out = e.clone();
            geom::put(&mut out, "from", fid.clone());
            geom::put(&mut out, "to", t.clone().unwrap());
            foreign_edges.push(out);
            if !foreign.iter().any(|c| get(c, "id") == Some(&fid)) {
                foreign.push(Value::Map(vec![
                    (Value::keyword("id"), fid),
                    (
                        Value::keyword("name"),
                        get(&fe, "name").cloned().unwrap_or(Value::Nil),
                    ),
                    (Value::keyword("foreign"), Value::Bool(true)),
                    (Value::keyword("shape"), Value::keyword("oval")),
                ]));
            }
        } else if th && let Some(fe) = fe {
            port(&mut incoming, &t.unwrap(), &fe);
        }
    }
    let mut boxes = boxes;
    for c in boxes.iter_mut() {
        let n = name(get(c, "id").unwrap());
        if let Some(v) = incoming.get(&n) {
            geom::put(c, "in-deps", Value::Vector(v.clone()));
        }
        if let Some(v) = outgoing.get(&n) {
            geom::put(c, "out-deps", Value::Vector(v.clone()));
        }
    }
    let merged = policy::merge_edges(&[internal, foreign_edges].concat());
    let edges = policy::apply_edge_kinds(
        match &merged {
            Value::Vector(v) => v,
            _ => &[],
        },
        get(doc, "edge-kinds"),
        get(doc, "omit-edges"),
    );
    let mut visible: Vec<Value> = nodes.clone();
    for c in &foreign {
        visible.push(get(c, "id").cloned().unwrap());
    }
    let filtered: Vec<Value> = seq(Some(&edges))
        .iter()
        .filter(|e| {
            contains(&visible, get(e, "from").unwrap()) && contains(&visible, get(e, "to").unwrap())
        })
        .cloned()
        .collect();
    let title = if !path.is_empty() {
        Value::String(path_name(path))
    } else {
        get(doc, "title")
            .cloned()
            .unwrap_or(Value::String("UML".into()))
    };
    let mut r = Value::Map(vec![
        (Value::keyword("title"), title.clone()),
        (Value::keyword("direction"), Value::keyword("tb")),
        (
            Value::keyword("packages"),
            Value::Vector(vec![Value::Map(vec![
                (Value::keyword("id"), Value::keyword("view")),
                (Value::keyword("label"), title),
                (Value::keyword("classes"), Value::Vector(boxes)),
            ])]),
        ),
        (Value::keyword("edges"), Value::Vector(filtered)),
    ]);
    if !foreign.is_empty() {
        geom::put(&mut r, "foreign", Value::Vector(foreign));
    }
    r
}

pub fn proposal_layers(x: &Value) -> Vec<Value> {
    policy::proposal_layers(x)
}

pub fn named_proposals(x: &Value) -> Vec<Value> {
    policy::named_proposals(x)
}

fn chosen(doc: &Value, which: Option<&Value>) -> (Option<Value>, Option<Value>) {
    let named = match which {
        Some(w @ Value::Map(_)) if get(w, "layers").is_some() => Some(w.clone()),
        Some(w) if !matches!(w, Value::Nil) => policy::proposal_by_id(doc, Some(w)),
        _ => named_proposals(doc).into_iter().next(),
    };
    (
        named
            .as_ref()
            .and_then(|n| policy::normalize_proposal(Some(n)))
            .or_else(|| policy::normalize_proposal(get(doc, "proposal"))),
        named,
    )
}

fn covering(cid: &Value, visible: &[Value], owners: &HashMap<String, Value>) -> Option<Value> {
    if contains(visible, cid) {
        return Some(cid.clone());
    }
    if let Some(owner) = owners.get(&name(cid))
        && contains(visible, owner)
    {
        return Some(owner.clone());
    }
    let mut s = name(cid).rsplit_once('.').map(|(p, _)| p.to_owned());
    while let Some(cur) = s {
        let k = Value::keyword(&cur);
        if contains(visible, &k) {
            return Some(k);
        }
        s = cur.rsplit_once('.').map(|(p, _)| p.to_owned());
    }
    None
}

fn remap_edges(
    doc: &Value,
    classes: &[Value],
    foreign: &[Value],
    owners: &HashMap<String, Value>,
    ranks: &Value,
) -> Value {
    let mut visible: Vec<Value> = classes
        .iter()
        .map(|c| get(c, "id").cloned().unwrap())
        .collect();
    for c in foreign {
        visible.push(get(c, "id").cloned().unwrap());
    }
    let mut edges: Vec<Value> = Vec::new();
    for e in seq(get(doc, "edges")) {
        let f = covering(get(e, "from").unwrap(), &visible, owners);
        let t = covering(get(e, "to").unwrap(), &visible, owners);
        if let (Some(f), Some(t)) = (f, t)
            && f != t
        {
            let mut out = e.clone();
            geom::put(&mut out, "from", f);
            geom::put(&mut out, "to", t);
            edges.push(out);
        }
    }
    let merged = policy::merge_edges(&edges);
    let merged = seq(Some(&merged));
    let kinds = policy::apply_edge_kinds(merged, get(doc, "edge-kinds"), get(doc, "omit-edges"));
    let restamped = policy::restamp_ranks(classes, seq(Some(&kinds)), ranks);
    geom::get(&restamped, "edges").cloned().unwrap()
}

fn group_box(group: &Value, kids: &[Value]) -> Value {
    let cs: Vec<Value> = kids
        .iter()
        .map(|c| {
            Value::Map(vec![
                (Value::keyword("id"), get(c, "id").cloned().unwrap()),
                (Value::keyword("name"), Value::String(module_name(c))),
                (
                    Value::keyword("drill?"),
                    Value::Bool(get(c, "drill?") == Some(&Value::Bool(true))),
                ),
            ])
        })
        .collect();
    let mut out = Value::Map(vec![
        (Value::keyword("id"), get(group, "id").cloned().unwrap()),
        (
            Value::keyword("name"),
            get(group, "label")
                .cloned()
                .unwrap_or(Value::String(label(get(group, "id").unwrap()))),
        ),
        (Value::keyword("drill?"), Value::Bool(true)),
        (Value::keyword("proposal-group?"), Value::Bool(true)),
    ]);
    if let Some(nses) = get(group, "nses") {
        geom::put(&mut out, "nses", nses.clone());
    }
    geom::put(&mut out, "members", Value::Vector(kids.to_vec()));
    geom::put(&mut out, "hide-members", Value::Bool(true));
    geom::put(&mut out, "contents", Value::Vector(cs));
    metrics(&mut out, kids, false);
    out
}

/// The proposal view: declared layers become packages, leftovers land in
/// `proposal.unassigned`, edges remap onto visible boxes or their owners.
pub fn proposal_view(doc: &Value, which: Option<&Value>) -> Value {
    let mut root = view_at(doc, &[]);
    let (proposal, named) = chosen(doc, which);
    let layers: Vec<Value> = seq(proposal.as_ref().and_then(|p| get(p, "layers"))).to_vec();
    let ranks = policy::ranks_from_layers(&layers);
    let mut omit: Vec<Value> = seq(get(doc, "omit")).to_vec();
    omit.extend(
        seq(proposal.as_ref().and_then(|p| get(p, "omit")))
            .iter()
            .cloned(),
    );
    let leaves: Vec<Value> = seq(get(doc, "classes"))
        .iter()
        .filter(|c| {
            get(c, "foreign").is_none()
                && !policy::omitted_id(get(c, "id").unwrap(), Some(&Value::Vector(omit.clone())))
        })
        .cloned()
        .collect();
    let restamped = policy::restamp_ranks(&leaves, &[], &ranks);
    let stamped = seq(get(&restamped, "classes")).to_vec();
    let idx = index(&stamped);
    let mut claimed: Vec<Value> = Vec::new();
    let mut owners: HashMap<String, Value> = HashMap::new();

    fn register(nses: Option<&Value>, owners: &mut HashMap<String, Value>) {
        for x in seq(nses) {
            if matches!(x, Value::Map(_)) {
                for cid in policy::nse_ids(get(x, "nses")) {
                    owners.insert(name(&cid), get(x, "id").cloned().unwrap());
                }
                register(get(x, "nses"), owners);
            }
        }
    }

    let as_module = |c: &Value| {
        let mut out = c.clone();
        geom::put(&mut out, "name", Value::String(module_name(c)));
        out
    };

    let pick = |x: &Value, stamped: &[Value], idx: &HashMap<String, &Value>| -> Vec<Value> {
        fn pick_rec(
            x: &Value,
            stamped: &[Value],
            idx: &HashMap<String, &Value>,
            as_module: &dyn Fn(&Value) -> Value,
        ) -> Vec<Value> {
            let mut r = Vec::new();
            if matches!(x, Value::Map(_)) {
                let mut kids = Vec::new();
                for n in seq(get(x, "nses")) {
                    kids.extend(pick_rec(n, stamped, idx, as_module));
                }
                if !kids.is_empty() {
                    r.push(group_box(x, &kids));
                }
            } else if let Some(c) = idx.get(&name(x)) {
                r.push(as_module(c));
            } else {
                for c in stamped {
                    if under(get(c, "id").unwrap(), x) {
                        r.push(as_module(c));
                    }
                }
            }
            r
        }
        pick_rec(x, stamped, idx, &as_module)
    };

    let mut pkgs: Vec<Value> = Vec::new();
    for layer in &layers {
        claimed.extend(policy::nse_ids(get(layer, "nses")));
        register(get(layer, "nses"), &mut owners);
        let mut cs = Vec::new();
        for x in seq(get(layer, "nses")) {
            cs.extend(pick(x, &stamped, &idx));
        }
        if !cs.is_empty() {
            pkgs.push(Value::Map(vec![
                (
                    Value::keyword("id"),
                    Value::keyword(&format!("proposal.{}", name(get(layer, "id").unwrap()))),
                ),
                (
                    Value::keyword("label"),
                    get(layer, "label").cloned().unwrap_or(Value::Nil),
                ),
                (Value::keyword("classes"), Value::Vector(cs)),
            ]));
        }
    }
    let mut extras: Vec<Value> = Vec::new();
    for c in &stamped {
        let covered = claimed
            .iter()
            .any(|cid| under(get(c, "id").unwrap(), cid) || under(cid, get(c, "id").unwrap()));
        if !covered {
            extras.push(as_module(c));
        }
    }
    if !extras.is_empty() {
        pkgs.push(Value::Map(vec![
            (Value::keyword("id"), Value::keyword("proposal.unassigned")),
            (Value::keyword("label"), Value::String("Unassigned".into())),
            (Value::keyword("classes"), Value::Vector(extras)),
        ]));
    }
    pkgs.reverse();
    let all: Vec<Value> = pkgs
        .iter()
        .flat_map(|p| seq(get(p, "classes")).to_vec())
        .collect();
    if !pkgs.is_empty() {
        let title = named
            .as_ref()
            .and_then(|n| get(n, "notice"))
            .or_else(|| proposal.as_ref().and_then(|p| get(p, "notice")))
            .cloned()
            .unwrap_or(Value::String(policy::PROPOSAL_NOTICE.into()));
        geom::put(&mut root, "title", title);
        geom::put(&mut root, "proposal", Value::Bool(true));
        geom::put(&mut root, "packages", Value::Vector(pkgs));
        let foreign = seq(get(&root, "foreign")).to_vec();
        let remapped = remap_edges(doc, &all, &foreign, &owners, &ranks);
        geom::put(&mut root, "edges", remapped);
    }
    root
}

/// Find a nested proposal group box declaration by id.
pub fn find_nested_group(doc: &Value, which: Option<&Value>, cid: &Value) -> Option<Value> {
    let cid = id(cid);
    let (proposal, _) = chosen(doc, which);
    let proposal = proposal?;
    fn find(nses: Option<&Value>, cid: &Value) -> Option<Value> {
        for x in seq(nses) {
            if matches!(x, Value::Map(_)) {
                if get(x, "id") == Some(cid) {
                    return Some(x.clone());
                }
                if let Some(found) = find(get(x, "nses"), cid) {
                    return Some(found);
                }
            }
        }
        None
    }
    for layer in seq(get(&proposal, "layers")) {
        if let Some(found) = find(get(layer, "nses"), &cid) {
            return Some(found);
        }
    }
    None
}

/// Drill into a proposal package or a nested group.
pub fn layer_view(doc: &Value, which: Option<&Value>, pkg_id: &Value) -> Value {
    let mut root = proposal_view(doc, which);
    let root_packages = seq(get(&root, "packages")).to_vec();
    for p in &root_packages {
        if get(p, "id") == Some(pkg_id) {
            let title = get(p, "label")
                .cloned()
                .unwrap_or(Value::String(name(pkg_id)));
            geom::put(&mut root, "title", title);
            geom::put(&mut root, "packages", Value::Vector(vec![p.clone()]));
            return root;
        }
    }
    for p in &root_packages {
        for c in seq(get(p, "classes")) {
            if get(c, "id") == Some(pkg_id)
                && get(c, "proposal-group?").is_some()
                && !seq(get(c, "members")).is_empty()
            {
                let cs = seq(get(c, "members")).to_vec();
                let title = get(c, "name")
                    .cloned()
                    .unwrap_or(Value::String(name(pkg_id)));
                let (proposal, _) = chosen(doc, which);
                let layers = seq(proposal.as_ref().and_then(|p| get(p, "layers"))).to_vec();
                geom::put(&mut root, "title", title.clone());
                geom::put(
                    &mut root,
                    "packages",
                    Value::Vector(vec![Value::Map(vec![
                        (Value::keyword("id"), pkg_id.clone()),
                        (Value::keyword("label"), title),
                        (Value::keyword("classes"), Value::Vector(cs.clone())),
                    ])]),
                );
                let foreign = seq(get(&root, "foreign")).to_vec();
                let ranks = policy::ranks_from_layers(&layers);
                let remapped = remap_edges(doc, &cs, &foreign, &HashMap::new(), &ranks);
                geom::put(&mut root, "edges", remapped);
                return root;
            }
        }
    }
    root
}

/// Is this id a collapsed proposal package (`proposal.*`)?
pub fn proposal_package_id(cid: Option<&Value>) -> bool {
    cid.map(|c| name(c).starts_with("proposal."))
        .unwrap_or(false)
}

pub fn declutter_modes() -> Vec<Value> {
    vec![
        Value::keyword("full"),
        Value::keyword("arrows"),
        Value::keyword("triangles"),
        Value::keyword("elements"),
        Value::keyword("classes"),
    ]
}

pub fn next_declutter(mode: Option<&Value>) -> Value {
    let modes = declutter_modes();
    let i = modes
        .iter()
        .position(|x| Some(x) == mode)
        .map(|i| i + 1)
        .unwrap_or(1);
    modes[i % modes.len()].clone()
}

fn dummy(p: &Value) -> Value {
    Value::Map(vec![
        (
            Value::keyword("id"),
            get(p, "id").cloned().unwrap_or(Value::Nil),
        ),
        (
            Value::keyword("name"),
            get(p, "label")
                .cloned()
                .unwrap_or(Value::String(name(get(p, "id").unwrap()))),
        ),
        (Value::keyword("dummy?"), Value::Bool(true)),
        (Value::keyword("drill?"), Value::Bool(true)),
        (Value::keyword("hide-members"), Value::Bool(true)),
    ])
}

fn packages(view: &Value, transform: &dyn Fn(Value) -> Value) -> Value {
    let mut r = view.clone();
    let ps: Vec<Value> = seq(get(view, "packages"))
        .iter()
        .map(|p| transform(p.clone()))
        .collect();
    geom::put(&mut r, "packages", Value::Vector(ps));
    r
}

/// Collapse directional arrows between packages into one edge each with leaf
/// provenance (`:via-ids`, `:deps`); each package gains a dummy roll-up box.
pub fn collapse_arrows(view: &Value) -> Value {
    let multi = seq(get(view, "packages")).len() > 1;
    let view = packages(view, &|mut p: Value| {
        let cs = seq(get(&p, "classes")).to_vec();
        let found = cs.iter().any(|c| get(c, "id") == get(&p, "id"));
        if multi && !found {
            let mut roll = dummy(&p);
            metrics(&mut roll, &cs, false);
            let mut next_cs = vec![roll];
            next_cs.extend(cs);
            geom::put(&mut p, "classes", Value::Vector(next_cs));
        }
        p
    });
    let mut owners: HashMap<String, Value> = HashMap::new();
    let mut known: Vec<String> = Vec::new();
    for p in seq(get(&view, "packages")) {
        for c in seq(get(p, "classes")) {
            let n = name(get(c, "id").unwrap());
            owners.insert(n.clone(), get(p, "id").cloned().unwrap());
            known.push(n);
        }
    }
    for c in seq(get(&view, "foreign")) {
        known.push(name(get(c, "id").unwrap()));
    }
    struct Group {
        f: Value,
        t: Value,
        edges: Vec<Value>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for e in seq(get(&view, "edges")) {
        let of = get(e, "from").unwrap().clone();
        let ot = get(e, "to").unwrap().clone();
        let f = if multi {
            owners.get(&name(&of)).cloned().unwrap_or(of.clone())
        } else {
            of.clone()
        };
        let t = if multi {
            owners.get(&name(&ot)).cloned().unwrap_or(ot.clone())
        } else {
            ot.clone()
        };
        if !known.contains(&name(&f)) || !known.contains(&name(&t)) || f == t {
            continue;
        }
        let gi = groups
            .iter()
            .position(|g| g.f == f && g.t == t)
            .unwrap_or_else(|| {
                groups.push(Group {
                    f: f.clone(),
                    t: t.clone(),
                    edges: Vec::new(),
                });
                groups.len() - 1
            });
        let mut out = e.clone();
        geom::put(&mut out, "from", f);
        geom::put(&mut out, "to", t);
        geom::put(&mut out, "orig-from", of);
        geom::put(&mut out, "orig-to", ot);
        groups[gi].edges.push(out);
    }
    let mut edges = Vec::new();
    for g in &groups {
        let mut best: Option<&Value> = None;
        let mut viol = false;
        let mut via: Vec<Value> = Vec::new();
        let mut deps: Vec<Value> = Vec::new();
        for e in &g.edges {
            if get(e, "violating").is_some() && !viol {
                best = Some(e);
                viol = true;
            } else if !viol
                && (best.is_none()
                    || policy::kind_rank(get(e, "kind"))
                        >= policy::kind_rank(get(best.unwrap(), "kind")))
            {
                best = Some(e);
            }
            for k in ["from", "to", "orig-from", "orig-to"] {
                if let Some(x) = get(e, k)
                    && !contains(&via, x)
                {
                    via.push(x.clone());
                }
            }
            deps.push(Value::Map(vec![
                (
                    Value::keyword("from"),
                    get(e, "orig-from")
                        .or_else(|| get(e, "from"))
                        .cloned()
                        .unwrap(),
                ),
                (
                    Value::keyword("to"),
                    get(e, "orig-to").or_else(|| get(e, "to")).cloned().unwrap(),
                ),
                (
                    Value::keyword("violating"),
                    Value::Bool(get(e, "violating") == Some(&Value::Bool(true))),
                ),
            ]));
        }
        let mut out = geom::drop_keys(best.unwrap(), &["violating", "orig-from", "orig-to"]);
        geom::put(&mut out, "via-ids", Value::Vector(via));
        geom::put(&mut out, "deps", Value::Vector(deps));
        if viol {
            geom::put(&mut out, "kind", Value::keyword("dependency"));
            geom::put(&mut out, "violating", Value::Bool(true));
        }
        edges.push(out);
    }
    let mut view = view;
    geom::put(&mut view, "edges", Value::Vector(edges));
    view
}

/// Hide all member content (declutter "elements").
pub fn hide_elements(view: &Value) -> Value {
    packages(view, &|mut p: Value| {
        let cs: Vec<Value> = seq(get(&p, "classes"))
            .iter()
            .map(|c| {
                let mut out = geom::drop_keys(c, &["contents", "in-deps", "out-deps"]);
                geom::put(&mut out, "hide-members", Value::Bool(true));
                out
            })
            .collect();
        geom::put(&mut p, "classes", Value::Vector(cs));
        p
    })
}

/// Collapse each package to a single metrics roll-up box (declutter
/// "classes"), or hide members on a single-package view.
pub fn hide_classes(view: &Value) -> Value {
    let multi = seq(get(view, "packages")).len() > 1;
    packages(view, &|mut p: Value| {
        let mut real: Vec<Value> = Vec::new();
        let mut endpoint: Option<Value> = None;
        for c in seq(get(&p, "classes")) {
            if get(c, "dummy?").is_some() {
                if endpoint.is_none() {
                    endpoint = Some(c.clone());
                }
            } else {
                real.push(c.clone());
            }
        }
        if multi {
            let mut ep = geom::drop_keys(
                &endpoint.unwrap_or_else(|| dummy(&p)),
                &["crap", "killed", "survived", "uncovered", "mut-gap"],
            );
            metrics(&mut ep, &real, false);
            let nses: Vec<Value> = real
                .iter()
                .map(|c| get(c, "id").cloned().unwrap())
                .collect();
            geom::put(&mut p, "classes", Value::Vector(vec![ep]));
            geom::put(&mut p, "nses", Value::Vector(nses));
        } else {
            let cs: Vec<Value> = real
                .iter()
                .map(|c| {
                    let mut out = geom::drop_keys(c, &["contents"]);
                    geom::put(&mut out, "hide-members", Value::Bool(true));
                    out
                })
                .collect();
            geom::put(&mut p, "classes", Value::Vector(cs));
        }
        p
    })
}

/// Apply a declutter mode to a view; unknown modes are "full".
pub fn apply_declutter(view: &Value, mode: Option<&Value>) -> Value {
    let mut mode = mode.map(name).unwrap_or_else(|| "full".into());
    if mode == "methods" {
        mode = "elements".into();
    }
    let mut view = view.clone();
    if mode == "arrows" || mode == "triangles" || mode == "elements" || mode == "classes" {
        view = collapse_arrows(&view);
    }
    if mode == "triangles" {
        geom::put(&mut view, "hide-edges", Value::Bool(true));
    }
    if mode == "elements" || mode == "classes" {
        view = hide_elements(&view);
    }
    if mode == "classes" {
        view = hide_classes(&view);
    }
    view
}
