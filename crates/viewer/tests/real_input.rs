use viewer::real_input::{Action, Capture, Frame, Modifiers, Pointer, StreamOwner, convert};

#[test]
fn ordinary_press_uses_view_units_and_preserves_modifiers() {
    let frame = Frame {
        origin: [100.0, 40.0],
        size: [800.0, 600.0],
        pointer: Pointer {
            position: Some([125.0, 70.0]),
            pressed: true,
            down: true,
            ..Pointer::default()
        },
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
        },
        ..Frame::default()
    };
    let result = convert(&frame);
    assert_eq!(result.size, [800.0, 600.0]);
    assert_eq!(
        result.modifiers,
        Modifiers {
            ctrl: true,
            shift: true
        }
    );
    assert_eq!(
        result.actions,
        vec![Action::ChromePress(25.0, 30.0), Action::Press(25.0, 30.0)]
    );
}

#[test]
fn automatic_stream_owners_suppress_real_actions_but_keep_current_size() {
    for owner in [StreamOwner::Driver, StreamOwner::Bench] {
        let result = convert(&Frame {
            size: [940.0, 710.0],
            owner,
            pointer: Pointer {
                position: Some([25.0, 30.0]),
                pressed: true,
                down: true,
                ..Pointer::default()
            },
            ..Frame::default()
        });
        assert_eq!(result.size, [940.0, 710.0]);
        assert!(result.actions.is_empty());
    }
}

#[test]
fn ordinary_pointer_capture_and_canvas_edges_select_one_action() {
    for (position, pressed, down, captured, expected) in [
        (
            Some([125.0, 70.0]),
            false,
            false,
            false,
            vec![Action::Move(25.0, 30.0)],
        ),
        (Some([125.0, 70.0]), true, true, true, vec![Action::Leave]),
        (Some([125.0, 70.0]), false, false, true, vec![Action::Leave]),
        (Some([99.0, 70.0]), false, false, false, vec![Action::Leave]),
        (Some([901.0, 70.0]), true, true, false, vec![]),
        (None, false, false, false, vec![Action::Leave]),
        (None, false, true, false, vec![]),
    ] {
        let result = convert(&Frame {
            origin: [100.0, 40.0],
            size: [800.0, 600.0],
            pointer: Pointer {
                position,
                pressed,
                down,
                ..Pointer::default()
            },
            capture: Capture {
                ordinary: captured,
                dragging: false,
                ..Capture::default()
            },
            ..Frame::default()
        });
        assert_eq!(
            result.actions, expected,
            "position={position:?}, down={down}, captured={captured}"
        );
    }
}

#[test]
fn drag_capture_bypasses_ordinary_capture_and_release_follows_pointer_action() {
    let result = convert(&Frame {
        size: [800.0, 600.0],
        pointer: Pointer {
            position: Some([25.0, 30.0]),
            down: true,
            released: true,
            ..Pointer::default()
        },
        capture: Capture {
            ordinary: true,
            dragging: true,
            ..Capture::default()
        },
        ..Frame::default()
    });
    assert_eq!(
        result.actions,
        vec![
            Action::Drag(25.0, 30.0),
            Action::ReleaseCandidate(25.0, 30.0)
        ]
    );
}
