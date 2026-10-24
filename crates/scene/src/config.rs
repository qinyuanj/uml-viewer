//! Quality thresholds, grades and pooling (port of
//! `love/uml/domain/config.lua`). The band boundaries, the grade ramps and
//! the pooling rules are golden-protected behaviour: unchanged from the JVM.

use crate::geom::numval;
use edn::Value;
use edn::record::{get, num, truthy};

pub const GRADE_BEST: f64 = 10.0;
pub const GRADE_MID: f64 = 5.5;
pub const GRADE_WORST: f64 = 1.0;

/// CRAP traffic-light: missing data is red (never hidden).
pub fn crap_band(score: Option<f64>) -> Value {
    Value::keyword(match score {
        None => "red",
        Some(s) if s <= 8.0 => "green",
        Some(s) if s <= 12.0 => "yellow",
        _ => "red",
    })
}

/// Mutation traffic-light: missing data is red as well.
pub fn mutation_band(score: Option<f64>) -> Value {
    Value::keyword(match score {
        None => "red",
        Some(s) if s <= 0.8 => "red",
        Some(s) if s <= 0.9 => "yellow",
        _ => "green",
    })
}

fn along(x: f64, a: f64, b: f64, c: f64, y: f64, z: f64, w: f64) -> f64 {
    if x <= a {
        y
    } else if x <= b {
        y + (x - a) / (b - a) * (z - y)
    } else if x <= c {
        z + (x - b) / (c - b) * (w - z)
    } else {
        w
    }
}

pub fn crap_grade(score: Option<f64>) -> f64 {
    match score {
        None => 1.0,
        Some(s) => along(s, 8.0, 12.0, 20.0, 10.0, 5.5, 1.0),
    }
}

pub fn mutation_grade(score: Option<f64>) -> f64 {
    match score {
        None => 1.0,
        Some(s) => along(s, 0.8, 0.9, 1.0, 1.0, 5.5, 10.0),
    }
}

/// CRAP risk of a summary map: `mu + sigma`; absent `mu` means no risk data.
pub fn crap_risk(crap: Option<&Value>) -> Option<f64> {
    let mu = num(get(crap?, "mu")?)?;
    Some(mu + get(crap?, "sigma").and_then(num).unwrap_or(0.0))
}

pub fn combined_grade(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some((a + b) / 2.0),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

/// killed / (killed + survived + mut-gap), None when nothing was measured.
pub fn mutation_ratio(m: &Value) -> Option<f64> {
    let k = get(m, "killed").and_then(num);
    let s = get(m, "survived").and_then(num);
    let g = get(m, "mut-gap").and_then(num);
    if k.is_some() || s.is_some() || g.is_some_and(|g| g > 0.0) {
        let n = k.unwrap_or(0.0) + s.unwrap_or(0.0) + g.unwrap_or(0.0);
        if n > 0.0 {
            return Some(k.unwrap_or(0.0) / n);
        }
    }
    None
}

/// One CRAP score per function, absent metrics default to one.
pub fn function_weight(c: &Value) -> f64 {
    match get(c, "ops") {
        Some(Value::Vector(ops)) => (ops.len().max(1)) as f64,
        _ => 1.0,
    }
}

struct Group {
    n: f64,
    mu: f64,
    sigma: f64,
    max: f64,
}

fn group(xs: &[f64]) -> Group {
    let mut sum = 0.0;
    let mut mx = f64::NEG_INFINITY;
    for x in xs {
        sum += x;
        mx = mx.max(*x);
    }
    let mu = sum / xs.len() as f64;
    let mut variance = 0.0;
    for x in xs {
        variance += (x - mu) * (x - mu);
    }
    Group {
        n: xs.len() as f64,
        mu,
        sigma: (variance / xs.len() as f64).sqrt(),
        max: mx,
    }
}

fn rounded(g: &Group) -> Value {
    let r = |x: f64| (x * 10.0 + 0.5).floor() / 10.0;
    Value::Map(vec![
        (Value::keyword("n"), numval(g.n)),
        (Value::keyword("mu"), numval(r(g.mu))),
        (Value::keyword("sigma"), numval(r(g.sigma))),
        (Value::keyword("max"), numval(r(g.max))),
    ])
}

/// Summarize bare numbers and `{:mu}` records; empty input gives None.
pub fn summarize_scores(scores: &[Value]) -> Option<Value> {
    let xs: Vec<f64> = scores
        .iter()
        .filter_map(|x| match x {
            Value::Integer(_) | Value::Float(_) => num(x),
            _ => num(get(x, "mu")?),
        })
        .collect();
    if xs.is_empty() {
        return None;
    }
    Some(rounded(&group(&xs)))
}

/// Modules only: no foreign classes, no oval stubs.
fn module(c: &Value) -> bool {
    !truthy(get(c, "foreign")) && get(c, "shape") != Some(&Value::keyword("oval"))
}

fn score(x: &Value) -> Option<f64> {
    match x {
        Value::Integer(_) | Value::Float(_) => num(x),
        _ => get(x, "mu").and_then(num),
    }
}

/// Pool per-class CRAP into a directory/package summary. Classes without any
/// metric count as red (`mu = 20` per function). `rolled` keeps an authored
/// summary over freshly pooled function scores.
pub fn pool_crap(classes: &[Value], rolled: bool) -> Option<Value> {
    let mut groups: Vec<Group> = Vec::new();
    for c in classes {
        if !module(c) {
            continue;
        }
        let xs: Vec<f64> = match get(c, "ops") {
            Some(Value::Vector(ops)) => ops
                .iter()
                .filter_map(|op| get(op, "crap").and_then(score))
                .collect(),
            _ => Vec::new(),
        };
        let summary = get(c, "crap");
        let mu = summary.and_then(|s| get(s, "mu")).and_then(num);
        let g = if !xs.is_empty() && !(rolled && mu.is_some()) {
            group(&xs)
        } else if let Some(mu) = mu {
            let n = match summary.and_then(|s| get(s, "n")).and_then(num) {
                Some(n) if n > 0.0 => n,
                _ => {
                    if !xs.is_empty() {
                        xs.len() as f64
                    } else {
                        function_weight(c)
                    }
                }
            };
            Group {
                n: n.floor(),
                mu,
                sigma: summary
                    .and_then(|s| get(s, "sigma"))
                    .and_then(num)
                    .unwrap_or(0.0),
                max: summary
                    .and_then(|s| get(s, "max"))
                    .and_then(num)
                    .unwrap_or(mu),
            }
        } else {
            Group {
                n: function_weight(c),
                mu: 20.0,
                sigma: 0.0,
                max: 20.0,
            }
        };
        groups.push(g);
    }
    if groups.is_empty() {
        return None;
    }
    let (mut n, mut sum, mut m2, mut mx) = (0.0, 0.0, 0.0, f64::NEG_INFINITY);
    for g in &groups {
        n += g.n;
        sum += g.n * g.mu;
        m2 += g.n * (g.sigma * g.sigma + g.mu * g.mu);
        mx = mx.max(g.max);
    }
    let mu = sum / n;
    Some(rounded(&Group {
        n,
        mu,
        sigma: (m2 / n - mu * mu).max(0.0).sqrt(),
        max: mx,
    }))
}

/// Pool mutation counts; unmeasured classes contribute their function weight
/// to the gap. None when nothing was measured and no gap exists.
pub fn pool_mutants(classes: &[Value]) -> Option<Value> {
    let (mut k, mut s, mut g, mut u) = (0.0, 0.0, 0.0, 0.0);
    let (mut measured, mut uncovered) = (false, false);
    for c in classes {
        if !module(c) {
            continue;
        }
        let ck = get(c, "killed").and_then(num);
        let cs = get(c, "survived").and_then(num);
        let cg = get(c, "mut-gap").and_then(num);
        let cu = get(c, "uncovered").and_then(num);
        let has = ck.is_some() || cs.is_some();
        if has {
            measured = true;
            k += ck.unwrap_or(0.0);
            s += cs.unwrap_or(0.0);
        }
        if let Some(cu) = cu {
            uncovered = true;
            u += cu;
        }
        g += if has || cg.is_some() {
            cg.unwrap_or(0.0)
        } else {
            function_weight(c)
        };
    }
    if !measured && g <= 0.0 {
        return None;
    }
    let mut r = Value::Map(vec![]);
    if measured {
        r.assoc(Value::keyword("killed"), numval(k));
        r.assoc(Value::keyword("survived"), numval(s));
    }
    if uncovered {
        r.assoc(Value::keyword("uncovered"), numval(u));
    }
    if g > 0.0 {
        r.assoc(Value::keyword("mut-gap"), numval(g));
    }
    Some(r)
}
