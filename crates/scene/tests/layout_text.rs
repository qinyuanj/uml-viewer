//! Text width and class box lines — the layout_text_spec.lua oracle plus the
//! member-line rules exercised by viewer_spec.

use edn::parse;
use scene::geom::{get, name};
use scene::layout::{CHAR_W, class_box_size, class_lines, text_w};

#[test]
fn jvm_width_for_ascii_two_byte_and_astral_text() {
    assert_eq!(8.0, CHAR_W);
    assert_eq!(0.0, text_w(None));
    assert_eq!(40.0, text_w(Some("Hello")));
    assert_eq!(24.0, text_w(Some("«α»")));
    // U+1F600 is one UTF-16 surrogate pair on the JVM: two units.
    assert_eq!(16.0, text_w(Some("\u{1F600}")));
}

#[test]
fn full_width_cjk_counts_as_two_half_width_characters() {
    assert_eq!(32.0, text_w(Some("中文")));
    assert_eq!(16.0 + 8.0 * 3.0 + 16.0, text_w(Some("类Box提")));
    assert_eq!(16.0, text_w(Some("，"))); // U+FF0C full-width comma
    assert_eq!(16.0, text_w(Some("。"))); // U+3002 CJK punctuation
    assert_eq!(16.0, text_w(Some("한"))); // Hangul syllable
    assert_eq!(8.0, text_w(Some("—"))); // U+2014 stays half width
    assert_eq!(8.0, text_w(Some("ｱ"))); // U+FF71 half-width katakana
}

#[test]
fn chinese_names_widen_class_boxes() {
    let (w, _, _) = class_box_size(&parse("{:name \"中文类名中文类名中文类名\"}").unwrap());
    assert_eq!(24.0 + 12.0 * 16.0, w);
}

#[test]
fn member_lines_show_stereotype_public_members_and_rules() {
    let c = parse(
        "{:id :a :name \"a\" :stereotype :interface :fields [{:text \"field\"}] :ops [{:text \"visible\"} {:text \"secret\" :private true}]}",
    )
    .unwrap();
    let lines = class_lines(&c);
    let texts: Vec<String> = lines
        .iter()
        .filter_map(|l| match get(l, "text") {
            Some(edn::Value::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(vec!["«interface»", "a", "field", "visible"], texts);
    let rules = lines
        .iter()
        .filter(|l| name(get(l, "kind").unwrap()) == "rule")
        .count();
    assert_eq!(2, rules);
    let (_, h, _) = class_box_size(&c);
    // 24 base + name/stereo 2*18 + field/op 2*18 + rules 2*12.
    assert_eq!(24.0 + 4.0 * 18.0 + 2.0 * 12.0, h);
}

#[test]
fn hidden_members_and_children_replace_member_lines() {
    let c = parse("{:name \"a\" :contents [{:id :a.b :name \"B\" :drill? true}]}").unwrap();
    let lines = class_lines(&c);
    let kinds: Vec<String> = lines
        .iter()
        .map(|l| name(get(l, "kind").unwrap()))
        .collect();
    assert_eq!(vec!["name", "rule", "child"], kinds);
    let child = &lines[2];
    assert_eq!(Some(&edn::Value::keyword("a.b")), get(child, "id"),);
    let hidden = parse("{:name \"a\" :hide-members true :fields [{:text \"f\"}]}").unwrap();
    assert_eq!(1, class_lines(&hidden).len());
}

#[test]
fn box_width_grows_with_the_widest_line() {
    // 24 + 10 half-width cells is below the 120 floor.
    let (w, _, _) = class_box_size(&parse("{:name \"abcdefghij\"}").unwrap());
    assert_eq!(120.0, w);
    let (w, _, _) = class_box_size(&parse("{:name \"abcdefghijklmnopqrstuvw\"}").unwrap());
    assert_eq!(24.0 + 23.0 * 8.0, w);
}
