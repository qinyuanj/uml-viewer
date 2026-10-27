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
}

/// The host owns platform key names; text and paste never enter this stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Named(&'static str),
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
    pub keys: Vec<Key>,
    pub text_edit_focused: bool,
    pub search_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Press(f64, f64),
    Move(f64, f64),
    Leave,
    /// Temporary handoff for #175's double/drag/chrome conversion.
    LegacyPointer {
        x: f64,
        y: f64,
        double: bool,
        pressed: bool,
        down: bool,
    },
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
            if frame.capture.ordinary && !frame.capture.dragging {
                actions.push(Action::Leave);
            } else if frame.pointer.double || (frame.pointer.down && frame.capture.dragging) {
                actions.push(Action::LegacyPointer {
                    x,
                    y,
                    double: frame.pointer.double,
                    pressed: frame.pointer.pressed,
                    down: frame.pointer.down,
                });
            } else if frame.pointer.pressed {
                actions.push(Action::Press(x, y));
            } else if !frame.pointer.down {
                actions.push(Action::Move(x, y));
            }
            if frame.pointer.released {
                actions.push(Action::ReleaseCandidate(x, y));
            }
        } else if !frame.pointer.down {
            actions.push(Action::Leave);
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
                Key::Named("") => {}
                Key::Named(key) => actions.push(Action::Key {
                    key,
                    ctrl: frame.modifiers.ctrl,
                    shift: frame.modifiers.shift,
                }),
            }
        }
    }
    Converted {
        size: frame.size,
        modifiers: frame.modifiers,
        actions,
    }
}
