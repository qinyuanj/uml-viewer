//! Window-free conversion of real frontend frames into ordered actions.
//! Capture geometry stays with the search box and GUI; positions are points
//! in the host, and output coordinates are view units from the canvas origin.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Pointer {
    pub position: Option<[f64; 2]>,
    pub pressed: bool,
    pub double: bool,
    pub released: bool,
    pub down: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamOwner {
    #[default]
    Real,
    Driver,
    Bench,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Capture {
    pub ordinary: bool,
    pub dragging: bool,
    pub window_chrome: bool,
}

/// Raw host zoom factor and scroll points, plus the existing UI owners'
/// capture observations. The converter decides which owner gets the wheel.
#[derive(Debug, Clone, Copy)]
pub struct Wheel {
    pub zoom: f64,
    pub scroll: [f64; 2],
    pub over_search: bool,
    pub over_gui: bool,
    pub over_card: bool,
}

impl Default for Wheel {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            scroll: [0.0; 2],
            over_search: false,
            over_gui: false,
            over_card: false,
        }
    }
}

/// The host owns platform key names; text and paste never enter this stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Named(&'static str),
    Unknown,
    PerformanceToggle,
}

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub capture: Capture,
    pub owner: StreamOwner,
    pub origin: [f64; 2],
    pub size: [f64; 2],
    pub pointer: Pointer,
    pub modifiers: Modifiers,
    pub wheel: Wheel,
    pub keys: Vec<Key>,
    pub text_edit_focused: bool,
    pub search_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Wheel {
        dx: f64,
        dy: f64,
        ctrl: bool,
        shift: bool,
    },
    Press(f64, f64),
    DoublePress(f64, f64),
    /// Let the existing chrome/search owner handle the press first.
    ChromePress(f64, f64),
    Move(f64, f64),
    Leave,
    Drag(f64, f64),
    /// Execute only if the session is dragging after preceding actions.
    ReleaseCandidate(f64, f64),
    Key {
        key: &'static str,
        ctrl: bool,
        shift: bool,
    },
    TogglePerformance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub size: [f64; 2],
    pub modifiers: Modifiers,
    pub actions: Vec<Action>,
}

pub fn convert(frame: &Frame) -> Converted {
    let mut actions = Vec::new();
    if frame.owner == StreamOwner::Real {
        let position = frame
            .pointer
            .position
            .map(|[x, y]| [x - frame.origin[0], y - frame.origin[1]]);
        if let Some([x, y]) = position
            .filter(|[x, y]| *x >= 0.0 && *y >= 0.0 && *x <= frame.size[0] && *y <= frame.size[1])
        {
            if frame.capture.window_chrome && (frame.pointer.pressed || frame.pointer.double) {
                actions.push(Action::ChromePress(x, y));
            } else if frame.capture.ordinary && !frame.capture.dragging {
                actions.push(Action::Leave);
            } else if frame.pointer.double || frame.pointer.pressed {
                actions.push(Action::ChromePress(x, y));
                if !frame.capture.ordinary {
                    actions.push(if frame.pointer.double {
                        Action::DoublePress(x, y)
                    } else {
                        Action::Press(x, y)
                    });
                }
            } else if frame.pointer.down && frame.capture.dragging {
                actions.push(Action::Drag(x, y));
            } else if !frame.pointer.down {
                actions.push(Action::Move(x, y));
            }
            if frame.pointer.released {
                actions.push(Action::ReleaseCandidate(x, y));
            }
        } else if !frame.pointer.down {
            actions.push(Action::Leave);
        }
        let wheel_captured = frame.capture.window_chrome
            || frame.wheel.over_search
            || (frame.wheel.over_gui && !frame.wheel.over_card);
        let zoom = frame.wheel.zoom;
        if !wheel_captured && zoom.is_finite() && zoom != 1.0 && zoom > 0.0 {
            actions.push(Action::Wheel {
                dx: 0.0,
                dy: zoom.ln() / scene::ZOOM_STEP.ln(),
                ctrl: true,
                shift: false,
            });
        }
        let [dx, dy] = frame.wheel.scroll;
        if !wheel_captured && (dx != 0.0 || dy != 0.0) {
            actions.push(Action::Wheel {
                dx,
                dy: dy / 50.0,
                ctrl: false,
                shift: false,
            });
        }
        // Rename text fields own keys before even the local F3 shortcut.
        // Search keeps routing named keys through the existing pending path.
        for key in frame
            .keys
            .iter()
            .filter(|_| !frame.text_edit_focused || frame.search_active)
        {
            match key {
                Key::PerformanceToggle => actions.push(Action::TogglePerformance),
                Key::Unknown => {}
                Key::Named(key) => {
                    if !frame.capture.window_chrome {
                        actions.push(Action::Key {
                            key,
                            ctrl: frame.modifiers.ctrl,
                            shift: frame.modifiers.shift,
                        });
                    }
                }
            }
        }
    }
    Converted {
        size: frame.size,
        modifiers: frame.modifiers,
        actions,
    }
}
