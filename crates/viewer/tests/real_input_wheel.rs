use viewer::real_input::{Action, Frame, Key, Modifiers, Pointer, StreamOwner, Wheel, convert};

#[test]
fn automatic_streams_suppress_zoom_and_scroll_but_keep_frame_metadata() {
    for owner in [StreamOwner::Driver, StreamOwner::Bench] {
        let result = convert(&Frame {
            owner,
            size: [940.0, 710.0],
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
            },
            wheel: Wheel {
                zoom: 1.1,
                scroll: [12.0, -200.0],
                ..Wheel::default()
            },
            ..Frame::default()
        });
        assert!(result.actions.is_empty(), "owner={owner:?}");
        assert_eq!(result.size, [940.0, 710.0]);
        assert_eq!(
            result.modifiers,
            Modifiers {
                ctrl: true,
                shift: true
            }
        );
    }
}

#[test]
fn inverse_zoom_yields_negative_notches_and_zero_scroll_adds_nothing() {
    let result = convert(&Frame {
        wheel: Wheel {
            zoom: 1.0 / 1.1,
            ..Wheel::default()
        },
        ..Frame::default()
    });
    let [
        Action::Leave,
        Action::Wheel {
            dx,
            dy,
            ctrl,
            shift,
        },
    ] = result.actions.as_slice()
    else {
        panic!("unexpected actions: {:?}", result.actions);
    };
    assert_eq!((*dx, *ctrl, *shift), (0.0, true, false));
    assert!((*dy + 1.0).abs() < 1e-12);
    assert_eq!(convert(&Frame::default()).actions, vec![Action::Leave]);
}

#[test]
fn ctrl_zoom_is_queued_before_scroll_with_its_own_modifiers() {
    let result = convert(&Frame {
        wheel: Wheel {
            zoom: 1.21,
            scroll: [-8.0, 100.0],
            ..Wheel::default()
        },
        ..Frame::default()
    });
    let [
        Action::Leave,
        Action::Wheel {
            dx,
            dy,
            ctrl,
            shift,
        },
        scroll,
    ] = result.actions.as_slice()
    else {
        panic!("unexpected actions: {:?}", result.actions);
    };
    assert_eq!((*dx, *ctrl, *shift), (0.0, true, false));
    assert!((*dy - 2.0).abs() < 1e-12);
    assert_eq!(
        scroll,
        &Action::Wheel {
            dx: -8.0,
            dy: 2.0,
            ctrl: false,
            shift: false,
        }
    );
}

#[test]
fn invalid_zoom_is_ignored_without_losing_ordinary_scroll() {
    for zoom in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0, 1.0] {
        let result = convert(&Frame {
            wheel: Wheel {
                zoom,
                scroll: [0.0, 50.0],
                ..Wheel::default()
            },
            ..Frame::default()
        });
        assert_eq!(
            result.actions,
            vec![
                Action::Leave,
                Action::Wheel {
                    dx: 0.0,
                    dy: 1.0,
                    ctrl: false,
                    shift: false,
                }
            ],
            "zoom={zoom}"
        );
    }
}

#[test]
fn search_and_non_card_gui_capture_wheel_but_cards_keep_the_existing_queue_path() {
    for (over_search, over_gui, over_card, suppressed) in [
        (true, false, false, true),
        (false, true, false, true),
        (false, true, true, false),
        (true, true, true, true),
        (false, false, false, false),
    ] {
        let result = convert(&Frame {
            wheel: Wheel {
                zoom: 1.1,
                scroll: [0.0, -200.0],
                over_search,
                over_gui,
                over_card,
            },
            ..Frame::default()
        });
        if suppressed {
            assert_eq!(
                result.actions,
                vec![Action::Leave],
                "search={over_search}, gui={over_gui}, card={over_card}"
            );
        } else {
            assert_eq!(result.actions.len(), 3);
            assert_eq!(
                result.actions[2],
                Action::Wheel {
                    dx: 0.0,
                    dy: -4.0,
                    ctrl: false,
                    shift: false,
                }
            );
        }
    }
}

#[test]
fn ordinary_scroll_keeps_points_and_precedes_keys_after_pointer_actions() {
    let result = convert(&Frame {
        size: [800.0, 600.0],
        pointer: Pointer {
            position: Some([25.0, 30.0]),
            ..Pointer::default()
        },
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
        },
        wheel: Wheel {
            scroll: [12.5, -150.0],
            ..Wheel::default()
        },
        keys: vec![Key::Named("r")],
        ..Frame::default()
    });
    assert_eq!(
        result.actions,
        vec![
            Action::Move(25.0, 30.0),
            Action::Wheel {
                dx: 12.5,
                dy: -3.0,
                ctrl: false,
                shift: false,
            },
            Action::Key {
                key: "r",
                ctrl: true,
                shift: true,
            },
        ]
    );
}
