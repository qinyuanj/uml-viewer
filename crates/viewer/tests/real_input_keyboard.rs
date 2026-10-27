use viewer::real_input::{Action, Frame, Key, Modifiers, Pointer, StreamOwner, convert};

#[test]
fn rename_focus_filters_all_keys_before_f3_but_search_retains_its_key_path() {
    for (text_edit_focused, search_active, expected) in [
        (true, false, vec![]),
        (
            true,
            true,
            vec![
                Action::TogglePerformance,
                Action::Key {
                    key: "escape",
                    ctrl: false,
                    shift: true,
                },
            ],
        ),
        (
            false,
            false,
            vec![
                Action::TogglePerformance,
                Action::Key {
                    key: "escape",
                    ctrl: false,
                    shift: true,
                },
            ],
        ),
        (
            false,
            true,
            vec![
                Action::TogglePerformance,
                Action::Key {
                    key: "escape",
                    ctrl: false,
                    shift: true,
                },
            ],
        ),
    ] {
        let result = convert(&Frame {
            pointer: Pointer {
                down: true,
                ..Pointer::default()
            },
            keys: vec![Key::PerformanceToggle, Key::Named("escape")],
            modifiers: Modifiers {
                ctrl: false,
                shift: true,
            },
            text_edit_focused,
            search_active,
            ..Frame::default()
        });
        assert_eq!(
            result.actions, expected,
            "focus={text_edit_focused}, search={search_active}"
        );
    }
}

#[test]
fn automatic_streams_suppress_keys_and_local_performance_toggle() {
    for owner in [StreamOwner::Driver, StreamOwner::Bench] {
        let result = convert(&Frame {
            owner,
            keys: vec![Key::Named("f"), Key::PerformanceToggle],
            ..Frame::default()
        });
        assert!(result.actions.is_empty());
    }
}

#[test]
fn performance_toggle_is_local_and_unknown_names_are_ignored() {
    let result = convert(&Frame {
        pointer: Pointer {
            down: true,
            ..Pointer::default()
        },
        keys: vec![
            Key::Named(""),
            Key::PerformanceToggle,
            Key::Named("p"),
            Key::PerformanceToggle,
        ],
        ..Frame::default()
    });
    assert_eq!(
        result.actions,
        vec![
            Action::TogglePerformance,
            Action::Key {
                key: "p",
                ctrl: false,
                shift: false
            },
            Action::TogglePerformance,
        ]
    );
}

#[test]
fn known_keys_keep_names_modifiers_and_order_after_pointer_actions() {
    let result = convert(&Frame {
        size: [800.0, 600.0],
        pointer: Pointer {
            position: Some([25.0, 30.0]),
            pressed: true,
            down: true,
            ..Pointer::default()
        },
        keys: vec![Key::Named("r"), Key::Named("f"), Key::Named("enter")],
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
        },
        ..Frame::default()
    });
    assert_eq!(
        result.actions,
        vec![
            Action::ChromePress(25.0, 30.0),
            Action::Press(25.0, 30.0),
            Action::Key {
                key: "r",
                ctrl: true,
                shift: true
            },
            Action::Key {
                key: "f",
                ctrl: true,
                shift: true
            },
            Action::Key {
                key: "enter",
                ctrl: true,
                shift: true
            },
        ]
    );
}
