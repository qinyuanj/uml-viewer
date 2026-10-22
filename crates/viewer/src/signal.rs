//! SIGTERM / SIGINT as a quit request (#142). The recovery end-to-end suite
//! closes a viewer with SIGTERM and then expects the companion it owns to be
//! released, so the process must stop its helper and exit through the normal
//! shutdown path instead of dying in place. The handler only sets a flag;
//! the frame loop acts on it.

use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;

static TERMINATED: AtomicBool = AtomicBool::new(false);

extern "C" fn handler(_signal: c_int) {
    TERMINATED.store(true, Ordering::SeqCst);
}

unsafe extern "C" {
    /// BSD semantics on glibc: interrupted syscalls restart, so a flag is all
    /// the handler needs to be.
    fn signal(sig: c_int, handler: usize) -> usize;
}

/// Install the handlers once, before the window exists.
pub fn install() {
    for sig in [SIGTERM, SIGINT] {
        // SAFETY: the handler is a process-wide flag write; installing it is
        // safe before any thread is spawned.
        // A function item to its machine address: the C signature the
        // libc `signal` expects.
        unsafe { signal(sig, handler as extern "C" fn(c_int) as usize) };
    }
}

/// A termination signal arrived.
pub fn terminated() -> bool {
    TERMINATED.load(Ordering::SeqCst)
}
