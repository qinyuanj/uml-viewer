use viewer::real_input::{Action, Frame, Pointer, convert};

fn frame(pointer: Pointer) -> Frame {
    Frame {
        size: [800.0, 600.0],
        pointer: Pointer {
            position: Some([25.0, 30.0]),
            ..pointer
        },
        ..Frame::default()
    }
}

#[test]
fn double_press_wins_over_pressed_in_the_same_frame() {
    let converted = convert(&frame(Pointer {
        double: true,
        pressed: true,
        down: true,
        ..Pointer::default()
    }));
    assert_eq!(
        converted.actions,
        vec![
            Action::ChromePress(25.0, 30.0),
            Action::DoublePress(25.0, 30.0)
        ]
    );
}

#[test]
fn chrome_consumes_presses_and_double_presses_before_canvas_actions() {
    for double in [false, true] {
        let mut frame = frame(Pointer {
            double,
            pressed: true,
            down: true,
            ..Pointer::default()
        });
        frame.capture.window_chrome = true;
        assert_eq!(
            convert(&frame).actions,
            vec![Action::ChromePress(25.0, 30.0)]
        );
    }
}

#[test]
fn dragging_bypasses_ordinary_capture_but_a_new_chrome_press_is_consumed() {
    let mut frame = frame(Pointer {
        down: true,
        ..Pointer::default()
    });
    frame.capture.ordinary = true;
    frame.capture.dragging = true;
    assert_eq!(convert(&frame).actions, vec![Action::Drag(25.0, 30.0)]);
    frame.pointer.pressed = true;
    assert_eq!(
        convert(&frame).actions,
        vec![Action::ChromePress(25.0, 30.0)]
    );
}

#[test]
fn release_remains_ordered_after_press_and_is_not_delivered_for_consumed_chrome() {
    let mut frame = frame(Pointer {
        pressed: true,
        released: true,
        ..Pointer::default()
    });
    assert_eq!(
        convert(&frame).actions,
        vec![
            Action::ChromePress(25.0, 30.0),
            Action::Press(25.0, 30.0),
            Action::ReleaseCandidate(25.0, 30.0),
        ]
    );
    frame.capture.window_chrome = true;
    frame.capture.dragging = true;
    assert_eq!(
        convert(&frame).actions,
        vec![Action::ChromePress(25.0, 30.0)]
    );
}

#[test]
fn drag_release_and_idle_move_keep_their_order_and_canvas_edges_preserve_held_drags() {
    let mut frame = frame(Pointer {
        released: true,
        ..Pointer::default()
    });
    frame.capture.dragging = true;
    assert_eq!(
        convert(&frame).actions,
        vec![
            Action::Move(25.0, 30.0),
            Action::ReleaseCandidate(25.0, 30.0)
        ]
    );
    frame.pointer.down = true;
    assert_eq!(
        convert(&frame).actions,
        vec![
            Action::Drag(25.0, 30.0),
            Action::ReleaseCandidate(25.0, 30.0)
        ]
    );
    frame.pointer.position = Some([-1.0, 30.0]);
    assert!(convert(&frame).actions.is_empty());
    frame.pointer.down = false;
    assert_eq!(convert(&frame).actions, vec![Action::Leave]);
}

#[test]
fn an_existing_drag_continues_across_window_chrome_until_release() {
    let mut frame = frame(Pointer {
        down: true,
        ..Pointer::default()
    });
    frame.capture.window_chrome = true;
    frame.capture.dragging = true;
    assert_eq!(convert(&frame).actions, vec![Action::Drag(25.0, 30.0)]);
    frame.pointer.down = false;
    frame.pointer.released = true;
    assert_eq!(
        convert(&frame).actions,
        vec![
            Action::Move(25.0, 30.0),
            Action::ReleaseCandidate(25.0, 30.0)
        ]
    );
}
