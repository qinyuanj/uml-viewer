//! Class card model (#141): port of `application/detail.lua` (card model,
//! content-space rows, scroll clamping, row hits) and
//! `application/detail_view.lua` (the paint plan: fixed right-aligned metric
//! columns, colors, swatches, Cov / kill-rate bars, stripes). Window-free;
//! the egui window only paints [`Item`]s and reports pointer rows.

use edn::Value;
use scene::config;
use scene::document::ViewState;
use scene::geom::{get, get_slice, name, num};

use crate::source::Ident;
use crate::strings;
use crate::theme::{self, Rgb};

/// Default card window, in points.
pub const WIDTH: f64 = 640.0;
pub const HEIGHT: f64 = 700.0;
pub const PAD: f64 = 16.0;
pub const SWATCH_W: f64 = 10.0;
pub const SWATCH_GAP: f64 = 4.0;
/// One wheel notch scrolls one step.
pub const SCROLL_STEP: f64 = 48.0;

/// A relationship row: the other class and the directional phrase.
#[derive(Debug, Clone, PartialEq)]
pub struct Rel {
    pub id: Value,
    pub name: String,
    pub label: Option<String>,
    pub phrase: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub class: Value,
    pub ns: Option<String>,
    pub package: Option<Value>,
    pub title: Option<String>,
    pub lang: Option<Value>,
    pub file: Option<String>,
    pub rels: Vec<Rel>,
}

impl Model {
    /// A card for a bare class record (no scene context).
    pub fn of(class: Value) -> Model {
        Model {
            class,
            ns: None,
            package: None,
            title: None,
            lang: None,
            file: None,
            rels: Vec::new(),
        }
    }
}

fn string(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Nil => None,
        other => Some(name(other)),
    }
}

fn named<'a>(scene: &'a Value, key: &str, id: &Value) -> Option<&'a Value> {
    get_slice(get(scene, key))
        .iter()
        .find(|c| get(c, "id") == Some(id))
}

fn phrase(kind: Option<&Value>, outgoing: bool) -> String {
    let kind = kind.map(name).unwrap_or_default();
    let (out, inc) = strings::PHRASES
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map_or(strings::PHRASE_FALLBACK, |(_, o, i)| (*o, *i));
    if outgoing { out } else { inc }.to_string()
}

/// The card of class `id` in a scene, or None when it is not there.
pub fn model(scene: &Value, id: &Value) -> Option<Model> {
    let c = named(scene, "classes", id)?;
    let mut rels = Vec::new();
    for e in get_slice(get(scene, "edges")) {
        let outgoing = get(e, "from") == Some(id);
        if !(outgoing || get(e, "to") == Some(id)) {
            continue;
        }
        let other = get(e, if outgoing { "to" } else { "from" })
            .cloned()
            .unwrap_or(Value::Nil);
        let other_name = named(scene, "classes", &other)
            .and_then(|o| string(get(o, "name")))
            .unwrap_or_else(|| name(&other));
        rels.push(Rel {
            id: other,
            name: other_name,
            label: string(get(e, "label")),
            phrase: phrase(get(e, "kind"), outgoing),
        });
    }
    Some(Model {
        class: c.clone(),
        ns: string(get(c, "ns")).or_else(|| get(c, "id").map(name)),
        package: get(c, "package")
            .and_then(|p| named(scene, "packages", p))
            .cloned(),
        title: get(scene, "diagram").and_then(|d| string(get(d, "title"))),
        lang: get(c, "lang").cloned(),
        file: string(get(c, "file")),
        rels,
    })
}

/// Merge the visible scene with the full overlaid document so a card keeps
/// its leaf identities and relationships across namespaces.
pub fn for_state(state: &ViewState, id: &Value) -> Option<Model> {
    let Some(doc) = &state.document else {
        return model(&state.scene, id);
    };
    let doc = scene::overlay::apply(doc, &state.snapshots);
    let mut classes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut seen: Vec<Value> = Vec::new();
    let mut seen_edges: Vec<Value> = Vec::new();
    fn add_edges(edges: &mut Vec<Value>, seen: &mut Vec<Value>, list: &[Value]) {
        for e in list {
            let key = Value::Vector(
                ["from", "to", "kind", "label"]
                    .iter()
                    .map(|k| get(e, k).cloned().unwrap_or(Value::Nil))
                    .collect(),
            );
            if !seen.contains(&key) {
                seen.push(key);
                edges.push(e.clone());
            }
        }
    }
    fn add(classes: &mut Vec<Value>, seen: &mut Vec<Value>, c: &Value) {
        let cid = get(c, "id").cloned().unwrap_or(Value::Nil);
        if !seen.contains(&cid) {
            seen.push(cid);
            classes.push(c.clone());
        }
        for child in get_slice(get(c, "contents")) {
            add(classes, seen, child);
        }
    }
    let add_view = |classes: &mut Vec<Value>,
                    seen: &mut Vec<Value>,
                    edges: &mut Vec<Value>,
                    seen_edges: &mut Vec<Value>,
                    view: &Value| {
        add_edges(edges, seen_edges, get_slice(get(view, "edges")));
        for p in get_slice(get(view, "packages")) {
            for c in get_slice(get(p, "classes")) {
                add(classes, seen, c);
            }
        }
        for key in ["classes", "foreign"] {
            for c in get_slice(get(view, key)) {
                add(classes, seen, c);
            }
        }
    };
    add_edges(&mut edges, &mut seen_edges, get_slice(get(&doc, "edges")));
    // Authored leaf data wins over the abbreviated rows of collapsed boxes.
    for c in get_slice(get(&doc, "classes")) {
        add(&mut classes, &mut seen, c);
    }
    for p in get_slice(get(&doc, "packages")) {
        for c in get_slice(get(p, "classes")) {
            add(&mut classes, &mut seen, c);
        }
    }
    if get(&doc, "hierarchical").is_some_and(|v| v != &Value::Bool(false)) {
        let root = scene::hierarchy::view_at(&doc, &[]);
        add_view(&mut classes, &mut seen, &mut edges, &mut seen_edges, &root);
        // A port can name an intermediate namespace absent from the root and
        // current boxes: resolve its parent view only when asked.
        let mut parent: Vec<Value> = name(id).split('.').map(Value::keyword).collect();
        parent.pop();
        if !parent.is_empty() {
            let target = scene::hierarchy::view_at(&doc, &parent);
            add_view(
                &mut classes,
                &mut seen,
                &mut edges,
                &mut seen_edges,
                &target,
            );
            for p in get_slice(get(&target, "packages")) {
                for c in get_slice(get(p, "classes")) {
                    if get(c, "id") != Some(id) || get(c, "drill?") != Some(&Value::Bool(true)) {
                        continue;
                    }
                    for dir in ["in-deps", "out-deps"] {
                        for dep in get_slice(get(c, dir)) {
                            add(&mut classes, &mut seen, dep);
                            let other = get(dep, "id").cloned().unwrap_or(Value::Nil);
                            let (from, to) = if dir == "out-deps" {
                                (id.clone(), other)
                            } else {
                                (other, id.clone())
                            };
                            let edge = Value::Map(vec![
                                (Value::keyword("from"), from),
                                (Value::keyword("to"), to),
                                (Value::keyword("kind"), Value::keyword("dependency")),
                            ]);
                            add_edges(&mut edges, &mut seen_edges, &[edge]);
                        }
                    }
                }
            }
        }
    }
    add_edges(
        &mut edges,
        &mut seen_edges,
        get_slice(get(&state.scene, "edges")),
    );
    for c in get_slice(get(&state.scene, "classes")) {
        add(&mut classes, &mut seen, c);
    }
    let merged = Value::Map(vec![
        (Value::keyword("classes"), Value::Vector(classes)),
        (Value::keyword("edges"), Value::Vector(edges)),
        (
            Value::keyword("packages"),
            get(&state.scene, "packages")
                .cloned()
                .unwrap_or(Value::Vector(vec![])),
        ),
        (
            Value::keyword("diagram"),
            get(&state.scene, "diagram").cloned().unwrap_or(Value::Nil),
        ),
    ]);
    model(&merged, id)
}

/// What a source jump from this card opens: the module, or one member.
pub fn member_ident(model: &Model, member: Option<&str>) -> Ident {
    Ident {
        lang: model.lang.as_ref().map(name),
        file: model.file.clone(),
        ns: model.ns.clone(),
        name: member.filter(|m| !m.is_empty()).map(Into::into),
    }
}

/// Formatted metric cells of one stats row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cells {
    pub crap_s: Option<String>,
    pub crap_n: Option<f64>,
    pub cc_s: Option<String>,
    pub cov_s: Option<String>,
    pub coverage: Option<f64>,
    pub killed: Option<f64>,
    pub survived: Option<f64>,
    pub uncovered: Option<f64>,
    pub killed_s: Option<String>,
    pub survived_s: Option<String>,
    pub uncovered_s: Option<String>,
    pub mut_note: Option<String>,
}

/// One content-space row (y and h in points).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub kind: &'static str,
    pub text: String,
    pub y: f64,
    pub h: f64,
    pub module: bool,
    pub op_name: Option<String>,
    pub private: bool,
    /// The related class of a `rel` row.
    pub id: Option<Value>,
    pub cells: Cells,
}

fn mu(c: Option<&Value>) -> Option<f64> {
    match c? {
        Value::Map(_) => get(c?, "mu").and_then(num),
        other => num(other),
    }
}

fn one(n: f64) -> String {
    format!("{n:.1}")
}

fn whole(n: f64) -> String {
    format!("{}", n.floor() as i64)
}

fn number_text(v: &Value) -> String {
    match num(v) {
        Some(n) if n.fract() == 0.0 => whole(n),
        Some(n) => n.to_string(),
        None => string(Some(v)).unwrap_or_default(),
    }
}

fn cells(c: &Value, class: bool) -> Cells {
    let count = |k: &str| {
        let n = get(c, k).and_then(num);
        if n.is_some() || !class {
            return n;
        }
        get_slice(get(c, "ops"))
            .iter()
            .filter_map(|op| get(op, k).and_then(num))
            .fold(None, |acc, v| Some(acc.unwrap_or(0.0) + v))
    };
    let crap = mu(get(c, "crap"));
    let coverage = get(c, "coverage").and_then(num);
    let (killed, survived, uncovered, sites) = (
        count("killed"),
        count("survived"),
        count("uncovered"),
        count("sites"),
    );
    let mut r = Cells {
        crap_s: crap.map(|x| format!("{}{}", one(x), if class { "μ" } else { "" })),
        crap_n: crap,
        cc_s: if class {
            None
        } else {
            get(c, "cc").and_then(num).map(whole)
        },
        cov_s: coverage.map(|x| format!("{:.0}%", 100.0 * x)),
        coverage,
        killed,
        survived,
        uncovered,
        ..Cells::default()
    };
    let total = killed.unwrap_or(0.0) + survived.unwrap_or(0.0) + uncovered.unwrap_or(0.0);
    if total == 0.0 && sites.unwrap_or(0.0) == 0.0 {
        r.mut_note = Some(strings::NO_MUTATION_SITES.into());
    } else {
        r.killed_s = killed.map(whole);
        r.survived_s = survived.map(whole);
        r.uncovered_s = uncovered.map(whole);
    }
    r
}

fn row(kind: &'static str, text: impl Into<String>, y: f64, h: f64) -> Row {
    Row {
        kind,
        text: text.into(),
        y,
        h,
        module: false,
        op_name: None,
        private: false,
        id: None,
        cells: Cells::default(),
    }
}

/// The card rows in content space (20-point title, 18-point body rows).
pub fn rows(model: &Model) -> Vec<Row> {
    let c = &model.class;
    let mut rows: Vec<Row> = Vec::new();
    let mut y = 0.0;
    let emit = |rows: &mut Vec<Row>, y: &mut f64, mut r: Row| {
        r.y = *y;
        *y += r.h;
        rows.push(r);
    };
    emit(
        &mut rows,
        &mut y,
        row(
            "name",
            string(get(c, "name")).unwrap_or_default(),
            0.0,
            20.0,
        ),
    );
    if let Some(level) = get(c, "level").filter(|v| **v != Value::Nil) {
        let text = format!("{}{}", strings::LEVEL, number_text(level));
        emit(&mut rows, &mut y, row("muted", text, 0.0, 18.0));
    }
    if let Some(stereo) = get(c, "stereotype") {
        emit(
            &mut rows,
            &mut y,
            row("muted", format!("«{}»", name(stereo)), 0.0, 18.0),
        );
    }
    let base = model
        .file
        .as_ref()
        .and_then(|f| f.replace('\\', "/").rsplit('/').next().map(String::from));
    let drill = get(c, "drill?") == Some(&Value::Bool(true));
    let module = match &base {
        Some(b) if !drill && b.to_lowercase().ends_with(".rs") => Some(b.clone()),
        _ => model.ns.clone(),
    };
    if let Some(module) = module {
        let mut r = row("module", module, 0.0, 18.0);
        r.module = true;
        emit(&mut rows, &mut y, r);
    }
    let package = model
        .package
        .as_ref()
        .and_then(|p| string(get(p, "label")))
        .or_else(|| get(c, "package").map(name))
        .unwrap_or_default();
    emit(
        &mut rows,
        &mut y,
        row("muted", format!("{}{package}", strings::PACKAGE), 0.0, 18.0),
    );
    if let Some(title) = &model.title {
        emit(&mut rows, &mut y, row("muted", title.clone(), 0.0, 18.0));
    }
    let crap = get(c, "crap").filter(|v| **v != Value::Nil);
    if let Some(crap) = crap {
        let text = match num(crap) {
            Some(n) => strings::CRAP_SINGLE.replace("%s", &one(n)),
            None => {
                let f = |k: &str| get(crap, k).and_then(num).map(one).unwrap_or_default();
                strings::CRAP_LINE
                    .replacen("%s", &f("mu"), 1)
                    .replacen("%s", &f("max"), 1)
                    .replacen("%s", &f("sigma"), 1)
            }
        };
        emit(&mut rows, &mut y, row("crap", text, 0.0, 18.0));
    }
    let ops = get_slice(get(c, "ops"));
    let has = |k: &str| get(c, k).is_some_and(|v| *v != Value::Nil);
    if !ops.is_empty()
        || mu(crap).is_some()
        || has("coverage")
        || has("cc")
        || has("killed")
        || has("survived")
    {
        y += 6.0;
        emit(&mut rows, &mut y, row("group-header", "", 0.0, 18.0));
        emit(&mut rows, &mut y, row("col-header", "", 0.0, 18.0));
        let mut stats = row(
            "stats",
            string(get(c, "name")).unwrap_or_default(),
            0.0,
            18.0,
        );
        stats.cells = cells(c, true);
        emit(&mut rows, &mut y, stats);
        for op in ops {
            let private = get(op, "private") == Some(&Value::Bool(true));
            let text = format!(
                "{}{}",
                if private { "- " } else { "+ " },
                string(get(op, "text")).unwrap_or_default()
            );
            let mut r = row("stats", text, 0.0, 18.0);
            r.cells = cells(op, false);
            r.private = private;
            r.op_name = string(get(op, "name"));
            emit(&mut rows, &mut y, r);
        }
    }
    let fields = get_slice(get(c, "fields"));
    if !fields.is_empty() {
        y += 12.0;
        emit(
            &mut rows,
            &mut y,
            row("heading", strings::FIELDS, 0.0, 18.0),
        );
        for f in fields {
            let text = match f {
                Value::String(s) => s.clone(),
                other => string(get(other, "text")).unwrap_or_default(),
            };
            emit(&mut rows, &mut y, row("field", text, 0.0, 18.0));
        }
    }
    if !model.rels.is_empty() {
        y += 12.0;
        emit(
            &mut rows,
            &mut y,
            row("heading", strings::RELATIONSHIPS, 0.0, 18.0),
        );
        for rel in &model.rels {
            let label = rel
                .label
                .as_ref()
                .map(|l| format!("  «{l}»"))
                .unwrap_or_default();
            let mut r = row(
                "rel",
                format!("{}  {}{label}", rel.phrase, rel.name),
                0.0,
                18.0,
            );
            r.id = Some(rel.id.clone());
            emit(&mut rows, &mut y, r);
        }
    }
    rows
}

pub fn content_h(rows: &[Row]) -> f64 {
    rows.last().map_or(32.0, |r| r.y + r.h + 16.0)
}

pub fn max_scroll(rows: &[Row], view_h: f64) -> f64 {
    (content_h(rows) - view_h).max(0.0)
}

/// Clamped scroll offset after `delta` notches.
pub fn scroll(rows: &[Row], view_h: f64, offset: f64, delta: f64) -> f64 {
    (offset + delta * SCROLL_STEP).clamp(0.0, max_scroll(rows, view_h))
}

/// The hovered card row: an operation, or the module row.
#[derive(Debug, Clone, PartialEq)]
pub enum Hover {
    Module,
    Op(String),
}

pub fn highlight<'a>(rows: &'a [Row], hover: Option<&Hover>) -> Option<&'a Row> {
    let hover = hover?;
    rows.iter().find(|r| match hover {
        Hover::Module => r.module,
        Hover::Op(n) => r.op_name.as_deref() == Some(n.as_str()),
    })
}

fn at(rows: &[Row], y: f64) -> impl Iterator<Item = &Row> {
    rows.iter()
        .filter(move |r| y >= r.y && y <= r.y + r.h - 1.0)
}

pub fn rel_at(rows: &[Row], y: f64) -> Option<Value> {
    at(rows, y)
        .find(|r| r.kind == "rel")
        .and_then(|r| r.id.clone())
}

pub fn member_at(rows: &[Row], y: f64) -> Option<String> {
    at(rows, y).find_map(|r| r.op_name.clone())
}

pub fn module_at(rows: &[Row], y: f64) -> bool {
    at(rows, y).any(|r| r.module)
}

/// A metric column in content space.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub id: &'static str,
    pub label: &'static str,
    pub left: f64,
    pub right: f64,
}

const COLUMN_WIDTHS: [(&str, f64); 6] = [
    ("crap", 56.0),
    ("cc", 32.0),
    ("cov", 44.0),
    ("killed", 52.0),
    ("survived", 60.0),
    ("uncovered", 78.0),
];

/// Right-anchored columns; `width` defaults to 624 and never goes below 560
/// (narrower windows scroll horizontally).
pub fn column_layout(width: Option<f64>) -> Vec<Column> {
    let mut x = width.unwrap_or(624.0).max(560.0);
    let mut out = Vec::new();
    for (i, (id, w)) in COLUMN_WIDTHS.iter().enumerate().rev() {
        out.push(Column {
            id,
            label: strings::COLUMNS[i].1,
            left: x - w,
            right: x,
        });
        x -= w + 10.0;
    }
    out.reverse();
    out
}

pub fn group_layout(width: Option<f64>) -> [Column; 2] {
    let cols = column_layout(width);
    [
        Column {
            id: "crap",
            label: strings::GROUP_CRAP,
            left: cols[0].left,
            right: cols[2].right,
        },
        Column {
            id: "mutation",
            label: strings::GROUP_MUTATION,
            left: cols[3].left,
            right: cols[5].right,
        },
    ]
}

/// A painted cell: right-aligned unless `center`.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub id: &'static str,
    pub text: String,
    pub left: f64,
    pub right: f64,
    pub center: bool,
    pub color: Rgb,
    pub tip: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub id: &'static str,
    pub left: f64,
    pub right: f64,
    pub fraction: f64,
    pub color: Rgb,
}

/// One painted card row.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: &'static str,
    pub y: f64,
    pub h: f64,
    pub text: Option<String>,
    pub color: Rgb,
    pub title_font: bool,
    pub x: f64,
    pub clip_right: Option<f64>,
    pub cells: Vec<Cell>,
    pub bars: Vec<Bar>,
    pub stripe: bool,
    pub swatch: Option<Rgb>,
    pub clickable: bool,
    pub module: bool,
    pub op_name: Option<String>,
    pub rel: Option<Value>,
}

pub fn row_color(row: &Row) -> Rgb {
    if row.private {
        return theme::MUTED;
    }
    match row.kind {
        "crap" | "heading" | "group-header" | "col-header" => theme::GOLD,
        "name" | "module" | "field" | "rel" | "stats" => theme::INK,
        _ if row.module => theme::INK,
        _ => theme::MUTED,
    }
}

fn kill_ratio(c: &Cells) -> Option<f64> {
    let total = c.killed.unwrap_or(0.0) + c.survived.unwrap_or(0.0) + c.uncovered.unwrap_or(0.0);
    (total > 0.0).then(|| c.killed.unwrap_or(0.0) / total)
}

pub fn cell_color(c: &Cells, id: &str) -> Rgb {
    let positive = |v: Option<f64>| {
        if v.unwrap_or(0.0) > 0.0 {
            theme::VIOLATION
        } else {
            theme::GOOD
        }
    };
    match id {
        "cov" => theme::coverage(c.coverage),
        "crap" => theme::stroke(Some(config::crap_grade(c.crap_n))),
        "killed" => theme::WHITE,
        "survived" => positive(c.survived),
        "uncovered" => positive(c.uncovered),
        _ => theme::MUTED,
    }
}

/// Averaged CRAP / mutation grade; a missing metric enters as grade 1.
pub fn swatch(c: &Cells) -> Rgb {
    theme::stroke(config::combined_grade(
        Some(config::crap_grade(c.crap_n)),
        Some(config::mutation_grade(kill_ratio(c))),
    ))
}

fn cell_text(c: &Cells, id: &str) -> Option<String> {
    match id {
        "crap" => c.crap_s.clone(),
        "cc" => c.cc_s.clone(),
        "cov" => c.cov_s.clone(),
        "killed" => c.killed_s.clone(),
        "survived" => c.survived_s.clone(),
        "uncovered" => c.uncovered_s.clone(),
        _ => None,
    }
}

fn tip(id: &str) -> Option<&'static str> {
    strings::COLUMN_TIPS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, t)| *t)
}

/// The paint plan for `rows` at a content width (points).
pub fn view(rows: &[Row], width: Option<f64>) -> Vec<Item> {
    let cols = column_layout(width);
    let groups = group_layout(width);
    let member_right = cols[0].left - 10.0;
    let mut stats = 0;
    rows.iter()
        .map(|r| {
            let mut it = Item {
                kind: r.kind,
                y: r.y,
                h: r.h,
                text: Some(r.text.clone()),
                color: row_color(r),
                title_font: r.kind == "name",
                x: PAD,
                clip_right: matches!(r.kind, "stats" | "col-header").then_some(member_right),
                cells: Vec::new(),
                bars: Vec::new(),
                stripe: false,
                swatch: None,
                clickable: r.module || r.op_name.is_some() || r.kind == "rel",
                module: r.module,
                op_name: r.op_name.clone(),
                rel: r.id.clone(),
            };
            match r.kind {
                "group-header" => {
                    it.text = None;
                    for g in &groups {
                        it.cells.push(Cell {
                            id: g.id,
                            text: g.label.into(),
                            left: g.left,
                            right: g.right,
                            center: true,
                            color: theme::GOLD,
                            tip: None,
                        });
                    }
                }
                "col-header" => {
                    it.text = Some(strings::MEMBER.into());
                    for c in &cols {
                        it.cells.push(Cell {
                            id: c.id,
                            text: c.label.into(),
                            left: c.left,
                            right: c.right,
                            center: false,
                            color: theme::GOLD,
                            tip: tip(c.id),
                        });
                    }
                }
                "stats" => {
                    stats += 1;
                    it.stripe = stats % 2 == 0;
                    it.swatch = Some(swatch(&r.cells));
                    it.x = PAD + SWATCH_W + SWATCH_GAP;
                    for (i, c) in cols.iter().enumerate() {
                        if (i < 3 || r.cells.mut_note.is_none())
                            && let Some(text) = cell_text(&r.cells, c.id)
                        {
                            it.cells.push(Cell {
                                id: c.id,
                                text,
                                left: c.left,
                                right: c.right,
                                center: false,
                                color: cell_color(&r.cells, c.id),
                                tip: None,
                            });
                        }
                    }
                    if let Some(note) = &r.cells.mut_note {
                        it.cells.push(Cell {
                            id: "mut-note",
                            text: note.clone(),
                            left: groups[1].left,
                            right: groups[1].right,
                            center: true,
                            color: theme::MUTED,
                            tip: None,
                        });
                    } else if let Some(ratio) = kill_ratio(&r.cells) {
                        it.bars.push(Bar {
                            id: "kill",
                            left: groups[1].left,
                            right: groups[1].right,
                            fraction: ratio,
                            color: theme::stroke(Some(config::mutation_grade(Some(ratio)))),
                        });
                    }
                    if let Some(cov) = r.cells.coverage {
                        it.bars.push(Bar {
                            id: "cov",
                            left: cols[2].left,
                            right: cols[2].right,
                            fraction: cov,
                            color: theme::coverage(Some(cov)),
                        });
                    }
                }
                _ => {}
            }
            it
        })
        .collect()
}
