//! Lua truthiness of authored IR flags (#147 review): in the original scene
//! core `false` and an absent key are both false. Hand-written IR may spell
//! a flag out as `false`; the view must then be the one without the flag.
//! Expected values come from the pre-#144 Lua core (git a692019^, love/uml),
//! compiled once offline on these exact documents.

use edn::Value;
use edn::record::get;

fn compiled(text: &str) -> Value {
    let doc = scene::ir::read_document(text).unwrap();
    scene::document::compile_view(&doc, &[], &[], &Value::Map(vec![]))
}

fn class(scene: &Value, id: &str) -> Value {
    edn::record::seq(get(scene, "classes"))
        .iter()
        .find(|c| {
            get(c, "id")
                .map(|v| edn::record::local_name(v).unwrap_or_default().to_owned())
                .as_deref()
                == Some(id)
        })
        .cloned()
        .unwrap_or_else(|| panic!("class {id}"))
}

fn rect(c: &Value) -> String {
    edn::encode(get(c, "rect").unwrap()).unwrap()
}

#[test]
fn foreign_false_on_a_package_class_keeps_the_class_box() {
    let scene = compiled(
        r#"{:packages [{:id :p :label "P" :classes [{:id :a :name "A" :foreign false
             :ops [{:name "run"}]} {:id :b :name "B"}]}]
            :edges [{:from :b :to :a :kind :dependency}]}"#,
    );
    let a = class(&scene, "d0-a");
    assert_eq!(None, get(&a, "shape"), "Lua: shape nil, not :oval");
    assert_eq!("{:h 72 :w 120 :x 108 :y 346}", rect(&a));
}

#[test]
fn foreign_false_on_a_hierarchical_class_keeps_it_a_module() {
    let scene = compiled(
        r#"{:hierarchical true :prefix "x" :order [a b]
            :classes [{:id :a :name "A" :ns "x.a" :foreign false :ops [{:name "run" :text "run"}]}
                      {:id :b :name "B" :ns "x.b"}]
            :edges [{:from :b :to :a :kind :dependency}]}"#,
    );
    let a = class(&scene, "a");
    assert_eq!(
        None,
        get(&a, "shape"),
        "Lua: a module box, not an oval stub"
    );
    assert_eq!("{:h 72 :w 120 :x 68 :y 282}", rect(&a));
}

#[test]
fn private_false_on_an_op_keeps_the_member_line() {
    let scene = compiled(
        r#"{:hierarchical true :prefix "x" :order [a]
            :classes [{:id :a :name "A" :ns "x.a"
                       :ops [{:name "run" :text "run" :private false} {:name "stop" :text "stop"}]}]
            :edges []}"#,
    );
    // Lua: name, rule and both ops (4 lines, h 90); `:private true` gives 72.
    assert_eq!("{:h 90 :w 120 :x 68 :y 100}", rect(&class(&scene, "a")));
}

// #154: every flag the Lua core read as `if g(x,'k')` (or `not not g(..)`)
// treats an authored `false` or `nil` as omission and any other value
// (`:yes`, `1`) as `true`. Each test compares those spellings against the
// omitted and the `true` output, which are the reference behaviours.

const FALSY: [&str; 2] = ["false", "nil"];
const TRUTHY: [&str; 2] = [":yes", "1"];

fn encoded(v: &Value) -> String {
    edn::encode(v).unwrap()
}

/// What the scene draws: class rects, packages and edges.
fn drawn(scene: &Value) -> String {
    let rects: Vec<Value> = edn::record::seq(get(scene, "classes"))
        .iter()
        .map(|c| get(c, "rect").cloned().unwrap_or(Value::Nil))
        .collect();
    format!(
        "{} {} {}",
        encoded(&Value::Vector(rects)),
        encoded(get(scene, "packages").unwrap_or(&Value::Nil)),
        encoded(get(scene, "edges").unwrap_or(&Value::Nil)),
    )
}

#[test]
fn dummy_false_lays_out_and_routes_like_an_ordinary_class() {
    // layout.lua:106/110, compose.lua:33, route.lua:117: `g(c,'dummy?')`.
    let compiled = |flag: &str| {
        scene::compose::compile_diagram(&edn::parse(&format!(
            r#"{{:packages [{{:id :p :label "P" :classes [{{:id :a :name "A" {flag}}} {{:id :wall :name "W"}} {{:id :b :name "B"}}]}}]
                :edges [{{:from :a :to :b :kind :dependency}} {{:from :b :to :a :kind :dependency}}]}}"#
        )).unwrap())
    };
    let omitted = drawn(&compiled(""));
    let dummy = drawn(&compiled(":dummy? true"));
    assert_ne!(omitted, dummy, "fixture must tell a dummy apart");
    for v in FALSY {
        assert_eq!(omitted, drawn(&compiled(&format!(":dummy? {v}"))), "{v}");
    }
    for v in TRUTHY {
        assert_eq!(dummy, drawn(&compiled(&format!(":dummy? {v}"))), "{v}");
    }
}

#[test]
fn dummy_false_is_hit_before_a_dummy_roll_up() {
    // hit.lua:34-35: dummies are skipped once any class is not a dummy.
    let hit = |flag: &str| {
        let scene = edn::parse(&format!(
            r#"{{:classes [{{:id :a :rect {{:x 0 :y 0 :w 100 :h 50}} {flag}}}
                          {{:id :roll :dummy? true :rect {{:x 0 :y 0 :w 100 :h 50}}}}]}}"#
        ))
        .unwrap();
        let h = scene::hit::at(&scene, 50.0, 40.0).unwrap();
        encoded(get(&h, "id").unwrap())
    };
    assert_eq!(":a", hit(""));
    for v in FALSY {
        assert_eq!(":a", hit(&format!(":dummy? {v}")), "{v}");
    }
}

#[test]
fn dummy_false_is_not_taken_as_the_package_endpoint_when_hiding_classes() {
    // hierarchy.lua:239: `if get(c,'dummy?') then endpoint=…`.
    let hidden = |flag: &str| {
        let view = edn::parse(&format!(
            r#"{{:packages [{{:id :p :classes [{{:id :a {flag}}} {{:id :b}}]}} {{:id :q :classes [{{:id :c}}]}}]
                :edges []}}"#
        ))
        .unwrap();
        let out = scene::hierarchy::hide_classes(&view);
        let ids: Vec<String> = edn::record::seq(get(&out, "packages"))
            .iter()
            .flat_map(|p| edn::record::seq(get(p, "classes")).to_vec())
            .map(|c| encoded(get(&c, "id").unwrap()))
            .collect();
        ids.join(" ")
    };
    let omitted = hidden("");
    let dummy = hidden(":dummy? true");
    assert_ne!(omitted, dummy, "fixture must tell a dummy apart");
    for v in FALSY {
        assert_eq!(omitted, hidden(&format!(":dummy? {v}")), "{v}");
    }
    for v in TRUTHY {
        assert_eq!(dummy, hidden(&format!(":dummy? {v}")), "{v}");
    }
}

#[test]
fn proposal_group_false_keeps_the_namespace_label() {
    // hierarchy.lua:34 `module_name`: `if get(c,'proposal-group?')` uses :name.
    let name = |flag: &str| {
        let doc = edn::parse(&format!(
            r#"{{:hierarchical true :classes [{{:id :a.x :name "Custom" {flag}}} {{:id :b}}] :edges []
                :proposals [{{:id :p :layers [{{:id :l1 :nses [:a.x]}} {{:id :l2 :nses [:b]}}]}}]}}"#
        ))
        .unwrap();
        let v = scene::hierarchy::proposal_view(&doc, None);
        let names: Vec<String> = edn::record::seq(get(&v, "packages"))
            .iter()
            .flat_map(|p| edn::record::seq(get(p, "classes")).to_vec())
            .map(|c| encoded(get(&c, "name").unwrap_or(&Value::Nil)))
            .collect();
        names.join(" ")
    };
    let omitted = name("");
    let group = name(":proposal-group? true");
    assert_ne!(omitted, group, "fixture must tell a group apart");
    for v in FALSY {
        assert_eq!(omitted, name(&format!(":proposal-group? {v}")), "{v}");
    }
    for v in TRUTHY {
        assert_eq!(group, name(&format!(":proposal-group? {v}")), "{v}");
    }
}

#[test]
fn violating_false_does_not_turn_a_collapsed_arrow_into_a_violation() {
    // hierarchy.lua:222: `if get(e,'violating') and not viol then …`.
    let collapsed = |flag: &str| {
        let view = edn::parse(&format!(
            r#"{{:packages [{{:id :p :classes [{{:id :a}} {{:id :b}}]}}]
                :edges [{{:from :a :to :b :kind :association {flag}}}]}}"#
        ))
        .unwrap();
        let out = scene::hierarchy::collapse_arrows(&view);
        let e = edn::record::seq(get(&out, "edges"))[0].clone();
        (
            encoded(get(&e, "kind").unwrap()),
            get(&e, "violating").cloned(),
        )
    };
    let omitted = collapsed("");
    assert_eq!((":association".to_string(), None), omitted);
    for v in FALSY {
        assert_eq!(omitted, collapsed(&format!(":violating {v}")), "{v}");
    }
    let violating = collapsed(":violating true");
    for v in TRUTHY {
        assert_eq!(violating, collapsed(&format!(":violating {v}")), "{v}");
    }
}

#[test]
fn violating_is_lua_truthy_on_hit_deps_and_indicators() {
    // hit.lua:7 `not not g(e,'violating')`; hit.lua:18 `if g(dep,'violating')`.
    for (v, want) in [
        ("false", false),
        ("nil", false),
        ("true", true),
        (":yes", true),
        ("1", true),
    ] {
        let e = edn::parse(&format!("{{:from :a :to :b :violating {v}}}")).unwrap();
        let deps = scene::hit::deps_of(&e).unwrap();
        let dep = &edn::record::seq(Some(&deps))[0];
        assert_eq!(
            Some(&Value::Bool(want)),
            get(dep, "violating"),
            "deps_of {v}"
        );
        let scene = edn::parse(&format!(
            r#"{{:classes [{{:id :a :rect {{:x 0 :y 0 :w 100 :h 50}}}}]
                :edges [{{:from :a :to :b :deps [{{:from :a :to :b :violating {v}}}]}}]}}"#
        ))
        .unwrap();
        let inds = scene::hit::dep_indicators(&scene);
        let ind = &edn::record::seq(Some(&inds))[0];
        assert_eq!(
            Some(&Value::Bool(want)),
            get(ind, "violating"),
            "indicator {v}"
        );
    }
}

#[test]
fn drill_is_lua_truthy_on_class_hits_and_layer_ids() {
    // hit.lua:39 `['drill?']=not not g(c,'drill?')`; events.lua:174
    // `if kind=='class' and G.get(hit,'drill?')`.
    for (v, want) in [
        ("false", false),
        ("nil", false),
        ("true", true),
        (":yes", true),
        ("1", true),
    ] {
        let scene = edn::parse(&format!(
            r#"{{:classes [{{:id :a :drill? {v} :rect {{:x 0 :y 0 :w 100 :h 50}}}}]}}"#
        ))
        .unwrap();
        let h = scene::hit::at(&scene, 50.0, 4.0).unwrap();
        assert_eq!(Some(&Value::Bool(want)), get(&h, "drill?"), "hit {v}");
        let raw = edn::parse(&format!("{{:kind :class :id :a :drill? {v}}}")).unwrap();
        assert_eq!(
            want,
            scene::events::layer_id(Some(&raw)).is_some(),
            "layer_id {v}"
        );
    }
}

#[test]
fn hierarchical_is_lua_truthy_on_read_and_compile() {
    // ir.lua:53 `if g(raw,'hierarchical') then return raw end`;
    // document.lua:14 and overlay.lua:39 `if g(doc,'hierarchical')`.
    let doc = |v: &str| {
        format!(
            r#"{{:hierarchical {v} :prefix "x" :order [a b]
                :classes [{{:id :a :name "A" :ns "x.a"}} {{:id :b :name "B" :ns "x.b"}}]
                :edges [{{:from :b :to :a :kind :dependency}}]}}"#
        )
    };
    let want = drawn(&compiled(&doc("true")));
    for v in TRUTHY {
        assert_eq!(want, drawn(&compiled(&doc(v))), "{v}");
    }
}

#[test]
fn proposal_extra_is_lua_truthy_when_compiling_a_view() {
    // document.lua:16: `(extra==true or g(extra,'proposal')) and ps[1]`.
    let doc = scene::ir::read_document(
        r#"{:hierarchical true :classes [{:id :a} {:id :b}] :edges []
            :proposals [{:id :p :layers [{:id :l1 :nses [:a]} {:id :l2 :nses [:b]}]}]}"#,
    )
    .unwrap();
    let view = |extra: &str| {
        drawn(&scene::document::compile_view(
            &doc,
            &[],
            &[],
            &edn::parse(extra).unwrap(),
        ))
    };
    let real = view("{}");
    let proposed = view("{:proposal true}");
    assert_ne!(real, proposed, "fixture must tell the proposal apart");
    for v in FALSY {
        assert_eq!(real, view(&format!("{{:proposal {v}}}")), "{v}");
    }
    for v in TRUTHY {
        assert_eq!(proposed, view(&format!("{{:proposal {v}}}")), "{v}");
    }
}
