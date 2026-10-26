//! Canvas painting under zoom: world-space labels scale with the camera.

use scene::document::ViewState;
use viewer::paint::TextCache;
use viewer::render_cache::RenderCache;

/// The font size of every painted galley whose text is `label`.
fn painted_sizes(state: &ViewState, label: &str) -> Vec<f32> {
    let ctx = egui::Context::default();
    let (mut cache, mut text) = (RenderCache::default(), TextCache::default());
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let painter = ui.painter_at(screen);
        viewer::paint::paint(
            ui.ctx(),
            &painter,
            state,
            screen,
            None,
            None,
            None,
            false,
            &mut cache,
            &mut text,
        );
    });
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.galley.job.sections[0].format.font_id.size)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_port_label_scales_with_zoom() {
    let scene = edn::parse(
        "{:classes [{:id :a :rect {:x 10 :y 10 :w 120 :h 60} \
          :out-ports [{:id :common :name \"Common\" :rect {:x 20 :y 100 :w 80 :h 24}}]}] \
          :packages [] :edges []}",
    )
    .unwrap();
    let mut state = ViewState {
        scene,
        revision: 1,
        zoom: 1.0,
        ..ViewState::default()
    };
    let near = painted_sizes(&state, "Common");
    state.zoom = 2.0;
    let far = painted_sizes(&state, "Common");
    assert_eq!(vec![10.0], near);
    assert_eq!(vec![20.0], far);
}

#[test]
fn a_quality_letter_stays_centred_in_its_dot_under_zoom() {
    let scene = edn::parse(
        "{:classes [{:id :a :name \"A\" :rect {:x 10 :y 40 :w 120 :h 60} :crap {:mu 2}}] \
          :packages [] :edges []}",
    )
    .unwrap();
    let mut state = ViewState {
        scene,
        revision: 1,
        zoom: 4.0,
        ..ViewState::default()
    };
    state.cam_x = 0.0;
    let ctx = egui::Context::default();
    let (mut cache, mut text) = (RenderCache::default(), TextCache::default());
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let painter = ui.painter_at(screen);
        viewer::paint::paint(
            ui.ctx(),
            &painter,
            &state,
            screen,
            None,
            None,
            None,
            false,
            &mut cache,
            &mut text,
        );
    });
    out.textures_delta.clear();
    let dot = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Circle(circle) if circle.fill != egui::Color32::TRANSPARENT => {
                Some(circle.center)
            }
            _ => None,
        })
        .expect("a C dot");
    let letter = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "C" => {
                Some(t.pos.y + t.galley.size().y / 2.0)
            }
            _ => None,
        })
        .expect("a C label");
    assert!(
        (letter - dot.y).abs() < 2.0,
        "letter middle {letter} vs dot centre {}",
        dot.y
    );
}
