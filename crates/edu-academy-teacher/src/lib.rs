//! Teacher-facing Academy surface (slice 1, packet A): library only.
//! No binary, no live Academy call and no credential is constructed here.
#![forbid(unsafe_code)]

pub mod backend;
pub mod envelope;
pub mod facts;
pub mod labels;
pub mod lesson;
pub mod log;
pub mod render;
pub mod synthetic;
pub mod tools;

pub use lesson::prepare;
pub use synthetic::load_scenario;
