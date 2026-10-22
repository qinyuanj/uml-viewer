//! Render cache invalidation (#143 performance fix, spec #134 "渲染"):
//! draw geometry is rebuilt only when the scene, selection, hover or LOD
//! tier changes; pan and zoom only change the transform.

use edn::Value;
use scene::document::{ViewState, compile_view};
use viewer::lod::Tier;
use viewer::render_cache::RenderCache;

fn state(classes: &str) -> ViewState {
    let text = format!("{{:packages [{{:label \"P\" :classes {classes}}}]}}");
    let doc = scene::ir::read_document(&text).unwrap();
    ViewState {
        scene: compile_view(&doc, &[], &[], &Value::Map(vec![])),
        revision: 1,
        ..ViewState::default()
    }
}

fn two_classes() -> ViewState {
    state("[{:id :a :deps [:b]} {:id :b}]")
}

#[test]
fn pan_and_zoom_within_a_tier_reuse_the_built_geometry() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    cache.frame(&s, Tier::Full);
    s.cam_x += 120.0;
    s.cam_y -= 40.0;
    s.zoom = 1.7;
    cache.frame(&s, Tier::Full);
    assert_eq!(1, cache.builds());
}

#[test]
fn selection_and_hover_changes_rebuild_the_highlighted_geometry() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    cache.frame(&s, Tier::Full);
    s.selected = Some(Value::keyword("a"));
    cache.frame(&s, Tier::Full);
    assert_eq!(2, cache.builds());
    s.hover = Some(Value::keyword("b"));
    cache.frame(&s, Tier::Full);
    cache.frame(&s, Tier::Full);
    assert_eq!(3, cache.builds());
}

#[test]
fn an_unchanged_state_with_a_cloned_scene_does_not_rebuild() {
    let mut cache = RenderCache::default();
    let s = two_classes();
    cache.frame(&s, Tier::Full);
    // Event handlers return a cloned state every input; equal inputs reuse.
    let again = s.clone();
    cache.frame(&again, Tier::Full);
    assert_eq!(1, cache.builds());
}

#[test]
fn a_new_revision_rebuilds() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    cache.frame(&s, Tier::Full);
    s.revision += 1;
    cache.frame(&s, Tier::Full);
    assert_eq!(2, cache.builds());
}

#[test]
fn crossing_an_lod_tier_rebuilds_and_staying_in_it_does_not() {
    let mut cache = RenderCache::default();
    let s = two_classes();
    cache.frame(&s, Tier::Full);
    cache.frame(&s, Tier::Frames);
    cache.frame(&s, Tier::Frames);
    assert_eq!(2, cache.builds());
}

#[test]
fn mid_tier_decimation_rebuilds_per_zoom_step_not_per_frame() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    s.zoom = 0.22;
    cache.frame(&s, Tier::Frames);
    // Within one quarter-octave step: reuse.
    s.zoom = 0.225;
    cache.frame(&s, Tier::Frames);
    assert_eq!(1, cache.builds());
    // Half an octave further: a coarser step, rebuilt once.
    s.zoom = 0.32;
    cache.frame(&s, Tier::Frames);
    cache.frame(&s, Tier::Frames);
    assert_eq!(2, cache.builds());
}

#[test]
fn an_lod_change_redecimates_edges_without_rebuilding_commands() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    cache.frame(&s, Tier::Full);
    s.zoom = 0.3;
    cache.frame(&s, Tier::Frames);
    s.zoom = 0.2;
    cache.frame(&s, Tier::Frames);
    assert_eq!(3, cache.builds());
    assert_eq!(1, cache.command_builds());
}

#[test]
fn a_recompiled_scene_at_the_same_revision_rebuilds() {
    let mut cache = RenderCache::default();
    let mut s = two_classes();
    cache.frame(&s, Tier::Full);
    // Drill/declutter recompile the scene from focus/extra without bumping
    // the revision.
    s.extra
        .assoc(Value::keyword("declutter"), Value::keyword("leaves"));
    s.scene = state("[{:id :a} {:id :b} {:id :c}]").scene;
    cache.frame(&s, Tier::Full);
    assert_eq!(2, cache.builds());
    let classes = cache
        .commands()
        .iter()
        .filter(|(_, c)| c.kind() == "class")
        .count();
    assert_eq!(3, classes);
}
