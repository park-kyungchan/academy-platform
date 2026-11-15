//! Education sources with explicit native owner inputs and bounded transport.
//! Construction grants no runtime activation or business/source-write authority.
#![forbid(unsafe_code)]
pub mod capture;
pub mod dailytest;
pub mod data;
pub mod dom;
pub mod evidence;
pub mod history;
pub mod http;
pub(crate) mod js_text;
pub mod live_driver;
pub mod native;
pub mod owner_admission;
pub mod owner_root;
pub mod owner_workflow;
pub mod paper;
pub mod prepare_owner;
pub mod presentation;
pub mod prestudy;
pub mod propose_paper_form;
pub mod saved;
pub mod script;
pub mod student_course;
pub mod student_metadata;
pub mod textbook;
pub mod textbook_index;

pub mod replay;

pub mod dailyrecord;
