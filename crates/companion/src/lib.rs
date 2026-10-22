//! Headless companion roles for the single uml-viewer binary (#134/#138):
//! `companion-preflight`, `companion-worker`, `companion-runner` and
//! `companion-controller`, with the Herdr session backend, ownership rules,
//! durable recovery and the POSIX boundary they stand on. Behaviour follows
//! `docs/herdr-backend-contract.md` and the ADR 0007 platform table; the code
//! is a port of `love/uml/{domain,adapters}/{companion,herdr,recovery,...}.lua`.

pub mod domain;
pub mod herdr;
pub mod herdr_domain;
pub mod json;
pub mod lifecycle;
pub mod mailbox;
pub mod ownership;
pub mod posix;
pub mod preflight;
pub mod record;
pub mod recovery;
pub mod recovery_domain;
pub mod role;
pub mod text;
pub mod worker;

pub use posix::{Failure, Result};

/// `pub fn run(args: &[OsString]) -> ExitCode` for each `companion-*` role,
/// as the binary's ROLES table expects.
pub fn run_preflight(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    role::preflight(args)
}

pub fn run_worker(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    role::worker(args)
}

pub fn run_runner(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    role::runner(args)
}

pub fn run_controller(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    role::controller(args)
}

pub fn run_smoke(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    role::smoke(args)
}
