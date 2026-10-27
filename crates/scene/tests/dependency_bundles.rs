//! Final dependency-bundle geometry at the existing scene projection seam.
//! Historical route goldens continue to pin the unbundled `:points`/`:strokes`.

mod common;

use common::{assert_close, assert_golden_tolerance, read_edn};
use edn::record::get;
use edn::{Value, parse};

fn document() -> Value {
    let path = common::fixture("bundle-goldens", "shared-channel.ir.edn");
    scene::ir::read_document(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn scene() -> Value {
    scene::document::compile_view(&document(), &[], &[], &Value::Map(vec![]))
}

#[test]
fn compatible_dependencies_share_a_trunk_and_keep_their_branches_heads_and_leaves() {
    let projection = scene::cli::project(
        &document(),
        None,
        &Value::Vector(vec![]),
        &Value::Map(vec![]),
    );
    let golden = read_edn(&common::fixture(
        "bundle-goldens",
        "shared-channel.geometry.edn",
    ));
    assert_golden_tolerance(&golden, "shared-channel");
    for (i, expected) in common::items(get(&golden, "edges")).iter().enumerate() {
        let actual = &common::items(get(&projection, "edges"))[i];
        for key in ["from", "to", "kind", "head", "deps", "tip", "branches"] {
            assert_close(
                get(expected, key).unwrap(),
                get(actual, key),
                &format!("bundle edge[{i}] {key}"),
            );
        }
    }
    assert_close(
        get(&golden, "bundles").unwrap(),
        get(&projection, "bundles"),
        "final bundles",
    );
}

#[test]
fn visible_trunk_and_branch_geometry_return_their_own_hit_details() {
    let scene = scene();
    let trunk = scene::hit::at(&scene, 288.0, 210.0).unwrap();
    assert_eq!(
        Some(&Value::keyword("bundle")),
        get(&trunk, "kind"),
        "shared trunk hit"
    );
    assert_eq!(2, common::items(get(&trunk, "members")).len());

    let branch = scene::hit::at(&scene, 448.0, 160.0).unwrap();
    assert_eq!(
        Some(&parse("{:kind :edge :from :d :to :e :deps [{:from :d :to :e :kind :dependency :violating false}]}").unwrap()),
        Some(&branch),
        "source branch keeps its own leaf detail",
    );
    assert!(
        scene::hit::at(&scene, 288.0, 270.0).is_some(),
        "target branch remains hittable"
    );
}

#[test]
fn incompatible_candidates_keep_base_geometry_and_repeated_compilation_is_deterministic() {
    let original = document();
    let edges = common::items(get(&original, "edges"));
    for variant in [
        "single",
        "reverse",
        "violating",
        "association",
        "implements",
        "inheritance",
        "aggregation",
        "composition",
        "no-overlap",
    ] {
        let mut doc = original.clone();
        let mut changed = edges.clone();
        match variant {
            "single" => {
                changed.pop();
            }
            "reverse" => {
                edn::record::put(&mut changed[1], "from", Value::keyword("a"));
                edn::record::put(&mut changed[1], "to", Value::keyword("c"));
            }
            "violating" => edn::record::put(&mut changed[1], "violating", Value::Bool(true)),
            "no-overlap" => {
                edn::record::put(
                    &mut doc,
                    "classes",
                    parse(
                        "[{:id :a :level 2} {:id :c :level 3} {:id :d :level 3} {:id :e :level 2}]",
                    )
                    .unwrap(),
                );
                edn::record::put(&mut doc, "order", parse("[a d e c]").unwrap());
            }
            kind => edn::record::put(&mut changed[1], "kind", Value::keyword(kind)),
        }
        edn::record::put(&mut doc, "edges", Value::Vector(changed));
        let scene = scene::document::compile_view(&doc, &[], &[], &Value::Map(vec![]));
        assert!(
            (variant == "no-overlap" || get(&scene, "bundles").is_none()),
            "{variant}: no compatible trunk"
        );
        assert!(
            variant == "no-overlap"
                || common::items(get(&scene, "edges"))
                    .iter()
                    .all(|e| get(e, "branches").is_none()),
            "{variant}: base routes retained"
        );
    }
    let first = scene();
    for _ in 0..16 {
        assert_eq!(first, scene());
    }
}

#[test]
fn flat_document_projection_keeps_base_sections_and_exports_final_bundle_geometry() {
    let doc = parse(
        r#"{:title "Shared channel" :packages [{:id :p :label "P"
      :classes [{:id :f :name "F" :level 3} {:id :c :name "C" :level 3}
                {:id :d :name "D" :level 3} {:id :a :name "A" :level 2}
                {:id :e :name "E" :level 2}]}]
      :edges [{:from :d :to :e :kind :dependency} {:from :c :to :a :kind :dependency}]}"#,
    )
    .unwrap();
    let scene = scene::document::compile_view(&doc, &[], &[], &Value::Map(vec![]));
    assert_eq!(1, common::items(get(&scene, "bundles")).len());
    let projection = scene::cli::project(&doc, None, &parse("[]").unwrap(), &Value::Map(vec![]));
    assert_eq!(
        1,
        common::items(get(&projection, "sections")).len(),
        "base sections remain"
    );
    assert_eq!(
        1,
        common::items(get(&projection, "bundles")).len(),
        "final trunks are observable"
    );
    assert_eq!(
        get(&scene, "edges"),
        get(&projection, "edges"),
        "final branches share viewer geometry"
    );
}
