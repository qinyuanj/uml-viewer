//! The 2000-class bench scene compiles in seconds (#137, ADR 0008).
//!
//! The Lua `prepare_scene` rebuilt the class index for every edge (O(E·N)):
//! the 2000-class scene took 532 s to compile. The Rust port indexes classes
//! once plus a spatial grid, so this must stay in the low seconds and scale
//! near-linearly. Timing bounds are generous (CI machines vary); the ratio
//! check is the near-linearity evidence.

use edn::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A bench scene with its class and edge counts (classes are nested in
/// packages, so count through the normalized diagram).
fn scene_of(name: &str) -> (Value, usize, usize) {
    let path = repo().join("bench/scenes").join(name);
    let text = std::fs::read_to_string(&path).unwrap();
    let doc = scene::ir::read_document(&text).unwrap();
    let diagrams = edn::record::seq(edn::record::get(&doc, "diagrams"));
    let diagram = &diagrams[0];
    let classes: usize = edn::record::seq(edn::record::get(diagram, "packages"))
        .iter()
        .map(|p| edn::record::seq(edn::record::get(p, "classes")).len())
        .sum();
    let edges = edn::record::seq(edn::record::get(diagram, "edges")).len();
    (doc, classes, edges)
}

fn compile(doc: &Value) -> (Duration, Value) {
    let started = Instant::now();
    let scene = scene::document::compile_view(doc, &[], &[], &Value::Map(vec![]));
    let elapsed = started.elapsed();
    (elapsed, scene)
}

#[test]
fn the_bench_scenes_load_with_the_expected_size() {
    let (_, classes, edges) = scene_of("synthetic-2000.edn");
    assert_eq!(2000, classes);
    assert!(edges >= 6000, "expected ~6495 edges, got {edges}");
    let (_, classes, edges) = scene_of("synthetic-500.edn");
    assert_eq!(500, classes);
    assert!(edges >= 1500, "expected ~1617 edges, got {edges}");
}

#[test]
// Coverage instrumentation slows the build several-fold; time it uninstrumented.
#[cfg_attr(coverage, ignore)]
fn the_2000_class_scene_compiles_in_seconds() {
    let (doc, _, _) = scene_of("synthetic-2000.edn");
    let (elapsed, scene) = compile(&doc);
    let classes = edn::record::seq(edn::record::get(&scene, "classes")).len();
    let edges = edn::record::seq(edn::record::get(&scene, "edges")).len();
    eprintln!(
        "scene: 2000-class compile {:.3}s ({} classes, {} edges)",
        elapsed.as_secs_f64(),
        classes,
        edges
    );
    assert_eq!(2000, classes);
    assert!(edges >= 6000);
    // The Lua baseline was 532 s; allow a wide margin for slow CI machines.
    assert!(
        elapsed < Duration::from_secs(30),
        "2000-class compile took {elapsed:?}"
    );
}

#[test]
// Coverage instrumentation slows the build several-fold; time it uninstrumented.
#[cfg_attr(coverage, ignore)]
fn prepare_scene_scales_near_linearly_from_500_to_2000_classes() {
    let (small, _, _) = scene_of("synthetic-500.edn");
    let (large, _, _) = scene_of("synthetic-2000.edn");
    // Warm up both once so allocator/page-cache effects do not dominate.
    let _ = compile(&small);
    let _ = compile(&large);
    let mut small_times = Vec::new();
    let mut large_times = Vec::new();
    for _ in 0..3 {
        small_times.push(compile(&small).0.as_secs_f64());
        large_times.push(compile(&large).0.as_secs_f64());
    }
    let best = |xs: &[f64]| xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let ratio = best(&large_times) / best(&small_times).max(1e-6);
    eprintln!(
        "scene: 500-class {:.3}s, 2000-class {:.3}s, ratio {ratio:.1}x (4x data)",
        best(&small_times),
        best(&large_times)
    );
    // O(E·N) would be ~16x for 4x data; near-linear stays well below that.
    assert!(
        ratio < 10.0,
        "4x data compiled {ratio:.1}x slower: not near-linear"
    );
}

#[test]
fn the_exported_projection_covers_every_bench_class_and_edge() {
    let (doc, classes, edges) = scene_of("synthetic-2000.edn");
    let view = scene::cli::project(&doc, None, &Value::Vector(vec![]), &Value::Map(vec![]));
    // The synthetic scenes are flat diagrams, so the projection stacks one
    // section per diagram.
    let sections = edn::record::seq(edn::record::get(&view, "sections"));
    assert_eq!(1, sections.len());
    let section = &sections[0];
    assert_eq!(
        classes,
        edn::record::seq(edn::record::get(section, "classes")).len()
    );
    let exported = edn::record::seq(edn::record::get(section, "edges"));
    assert_eq!(edges, exported.len());
    // Section edges are routed but not clipped (prepare runs once on the
    // stacked scene), so the export carries finite routed points per section.
    for e in exported {
        let points = edn::record::seq(edn::record::get(e, "points"));
        assert!(points.len() >= 2);
        for p in points {
            assert!(edn::record::num(p.as_vector_first().unwrap()).is_some());
        }
    }
}

/// Tiny helper for the first element of an EDN vector.
trait FirstVector {
    fn as_vector_first(&self) -> Option<&Value>;
}

impl FirstVector for Value {
    fn as_vector_first(&self) -> Option<&Value> {
        match self {
            Value::Vector(items) => items.first(),
            _ => None,
        }
    }
}
