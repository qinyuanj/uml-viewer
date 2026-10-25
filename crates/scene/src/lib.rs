//! Scene core (#137): the pure-logic port of the LOVE scene pipeline
//! (ADR 0008). No egui, no windowing: IR EDN and quality snapshots in,
//! scene geometry EDN out, plus navigation state and proposal write-back.
//!
//! The public contract is `document::compile_view` together with the
//! navigation (`events`), search (`search`), highlighting (`highlight`)
//! and proposal write-back (`document::write_proposals`) operations.
//! The `scene` subcommand (`run`) exports scene geometry headlessly.

use std::ffi::OsString;
use std::process::ExitCode;

/// Entry point of the `scene` role: export scene geometry, headless.
pub fn run(args: &[OsString]) -> ExitCode {
    cli::run(args)
}

pub use card::{Neighbourhood, Relation, class_card, class_card_in};
pub use config::{
    Band, GRADE_BEST, GRADE_MID, GRADE_WORST, MetricRating, Rating, metric_rating, rating,
};

mod card;
pub mod compose;
pub mod config;
pub mod curve;
pub mod document;
pub mod events;
pub mod geom;
pub mod hierarchy;
pub mod highlight;
pub mod hit;
pub mod ir;
pub mod layout;
pub mod overlay;
pub mod policy;
pub mod route;
pub mod search;

pub mod cli;
