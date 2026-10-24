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
