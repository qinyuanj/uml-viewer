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

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub capture: Capture,
    pub owner: StreamOwner,
    pub origin: [f64; 2],
    pub size: [f64; 2],
    pub pointer: Pointer,
    pub modifiers: Modifiers,
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
    }
    Converted {
        size: frame.size,
        modifiers: frame.modifiers,
        actions,
    }
}
