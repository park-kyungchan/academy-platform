//! Result envelope with the one-of-three invariant (DESIGN-P8-v2:4).
use serde::{Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Ask,
    Saved,
    Held,
    Unavailable,
}

/// Closed set of stop codes: the refusal catalogue of DESIGN-P8-v2 part 1,
/// plus `tool_not_in_slice_1` and `student_ambiguous_unresolvable`, the
/// extra codes of slice 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopCode {
    EditionTreeUnavailable,
    UnitScopeUnavailable,
    DisplayInvalid,
    StudentNotFound,
    RosterClassMismatch,
    BookNotFound,
    DateNotAdmitted,
    DraftExpired,
    SaveNoOpenDraft,
    SourceUnavailable,
    ToolNotInSlice1,
    UnknownTool,
    StudentAmbiguousUnresolvable,
}

impl StopCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EditionTreeUnavailable => "edition_tree_unavailable",
            Self::UnitScopeUnavailable => "unit_scope_unavailable",
            Self::DisplayInvalid => "display_invalid",
            Self::StudentNotFound => "student_not_found",
            Self::RosterClassMismatch => "roster_class_mismatch",
            Self::BookNotFound => "book_not_found",
            Self::DateNotAdmitted => "date_not_admitted",
            Self::DraftExpired => "draft_expired",
            Self::SaveNoOpenDraft => "save_no_open_draft",
            Self::SourceUnavailable => "source_unavailable",
            Self::UnknownTool => "unknown_tool",
            Self::ToolNotInSlice1 => "tool_not_in_slice_1",
            Self::StudentAmbiguousUnresolvable => "student_ambiguous_unresolvable",
        }
    }
}

impl Serialize for StopCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Closed set of ask fields: the input field names of the tool table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskField {
    Topic,
    Student,
    ClassName,
    Date,
    Book,
    Unit,
    Draft,
}

impl AskField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Topic => "topic",
            Self::Student => "student",
            Self::ClassName => "class_name",
            Self::Date => "date",
            Self::Book => "book",
            Self::Unit => "unit",
            Self::Draft => "draft",
        }
    }
}

impl Serialize for AskField {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AskOption {
    pub value: String,
    pub label_ko: String,
    pub meaning_ko: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ask {
    pub field: AskField,
    pub question_ko: String,
    pub options: Vec<AskOption>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NextCall {
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Stop {
    pub code: StopCode,
    pub message_ko: String,
}
impl Stop {
    pub fn new(code: StopCode, message_ko: &str) -> Self {
        Self {
            code,
            message_ko: message_ko.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceRead {
    pub name: String,
    pub read_at: String,
    pub scope: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Coverage {
    pub sources: Vec<SourceRead>,
    pub unknowns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Envelope {
    pub status: Status,
    pub display: String,
    pub data: serde_json::Value,
    pub ask: Option<Ask>,
    pub next: Option<NextCall>,
    pub stop: Option<Stop>,
    pub coverage: Coverage,
}

impl Envelope {
    /// The one constructor enforcing the invariant: exactly one of
    /// `ask|next|stop` is non-null when status is not `ok|saved`, all null
    /// otherwise. `data` is the JSON value built from the calling tool's own
    /// typed, `Serialize` struct.
    pub fn new(
        status: Status,
        display: String,
        data: serde_json::Value,
        ask: Option<Ask>,
        next: Option<NextCall>,
        stop: Option<Stop>,
        coverage: Coverage,
    ) -> Result<Self, String> {
        let carriers = [ask.is_some(), next.is_some(), stop.is_some()]
            .iter()
            .filter(|v| **v)
            .count();
        match status {
            Status::Ok | Status::Saved => {
                if carriers != 0 {
                    return Err("ok/saved envelope must have no ask/next/stop".into());
                }
            }
            Status::Ask | Status::Held | Status::Unavailable => {
                if carriers != 1 {
                    return Err(
                        "ask/held/unavailable envelope needs exactly one of ask/next/stop".into(),
                    );
                }
            }
        }
        Ok(Self {
            status,
            display,
            data,
            ask,
            next,
            stop,
            coverage,
        })
    }
    pub fn ok(display: String, data: serde_json::Value, coverage: Coverage) -> Self {
        Self::new(Status::Ok, display, data, None, None, None, coverage).expect("ok invariant")
    }
    /// An ask with no option can never be answered, so building one is a
    /// constructor error the caller must handle; it never panics.
    pub fn ask(
        display: String,
        data: serde_json::Value,
        ask: Ask,
        coverage: Coverage,
    ) -> Result<Self, String> {
        if ask.options.is_empty() {
            return Err("ask_requires_options".into());
        }
        let mut display = display;
        for option in &ask.options {
            display.push_str(&format!("\n- {}", option.label_ko));
            if !option.meaning_ko.is_empty() {
                display.push_str(&format!(": {}", option.meaning_ko));
            }
        }
        Self::new(Status::Ask, display, data, Some(ask), None, None, coverage)
    }
    pub fn stop(display: String, data: serde_json::Value, stop: Stop, coverage: Coverage) -> Self {
        Self::new(
            Status::Unavailable,
            display,
            data,
            None,
            None,
            Some(stop),
            coverage,
        )
        .expect("stop invariant")
    }
    pub fn unknown_tool() -> Self {
        let names = crate::tools::TOOLS
            .iter()
            .filter(|tool| tool.handler == crate::tools::Handler::LessonPrepare)
            .map(|tool| tool.name)
            .collect::<Vec<_>>()
            .join(", ");
        let display = crate::display_labels::unknown_tool(&names);
        Self::stop(
            display.clone(),
            serde_json::json!({}),
            Stop::new(StopCode::UnknownTool, &display),
            Coverage::default(),
        )
    }
    pub fn unavailable_tool() -> Self {
        Self::stop(
            crate::display_labels::TOOL_UNAVAILABLE.into(),
            serde_json::Value::Object(Default::default()),
            Stop::new(
                StopCode::ToolNotInSlice1,
                crate::display_labels::TOOL_UNAVAILABLE,
            ),
            Coverage::default(),
        )
    }
}
