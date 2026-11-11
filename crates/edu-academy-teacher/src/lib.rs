//! Teacher-facing Academy surface (slice 1, packet A): library only.
//! No binary, no live Academy call and no credential is constructed here.
#![forbid(unsafe_code)]

pub mod backend;
pub mod envelope;
pub mod facts;
pub mod lesson;
pub mod render;
pub mod synthetic;
pub mod tools;

pub use backend::{BackendError, ResolvedStudent, SectionCheck, SourceBackend};
pub use envelope::{Ask, AskOption, Coverage, Envelope, LessonData, NextCall, Status, Stop};
pub use facts::{ClassFact, MovedLesson, SeededFacts, TeacherFacts};
pub use lesson::{LineRef, PrepareRequest, prepare, previous_lesson, scan_line};
pub use render::{Verdict, display_lint};
pub use synthetic::{Scenario, load_scenario};
pub use tools::{Handler, TOOLS, ToolSpec, dispatch, docs_table, tools_json, tools_list_minified};
