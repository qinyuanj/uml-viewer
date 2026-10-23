//! Window decoration choice and the undecorated window's own controls: with
//! no window-manager title bar, the drag strip carries minimise and close.

use viewer::app::{DRAG_STRIP, RESIZE_GRIP, WindowMode, mode_for, window_controls};

#[test]
fn x11_is_decorated_and_its_absence_falls_back_to_undecorated_wayland() {
    assert_eq!(WindowMode::X11Decorated, mode_for(true, false));
    assert_eq!(WindowMode::WaylandUndecorated, mode_for(false, false));
}

#[test]
fn the_test_seam_forces_the_undecorated_window_even_when_x11_works() {
    assert_eq!(WindowMode::WaylandUndecorated, mode_for(true, true));
    assert_eq!(WindowMode::WaylandUndecorated, mode_for(false, true));
}

#[test]
fn the_controls_sit_in_the_drag_strip_at_the_right_clear_of_the_resize_grips() {
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1500.0, 920.0));
    let [minimise, close] = window_controls(screen);
    assert!(
        close.max.x <= screen.max.x - RESIZE_GRIP,
        "close overlaps the east grip"
    );
    assert!(minimise.max.x <= close.min.x, "minimise sits left of close");
    for rect in [minimise, close] {
        assert!(
            rect.min.y >= screen.min.y + RESIZE_GRIP,
            "a control overlaps the north grip"
        );
        assert!(
            rect.max.y <= screen.min.y + DRAG_STRIP,
            "a control leaves the drag strip"
        );
        assert!(rect.width() >= 20.0, "a control is too narrow to hit");
    }
}

#[test]
fn the_controls_follow_an_offset_content_rect() {
    let screen = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(640.0, 480.0));
    let [_, close] = window_controls(screen);
    assert!(close.max.x <= screen.max.x - RESIZE_GRIP);
    assert!(close.min.y >= screen.min.y + RESIZE_GRIP);
}
