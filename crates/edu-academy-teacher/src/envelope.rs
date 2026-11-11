//! Result envelope with the one-of-three invariant (DESIGN-P8-v2:4).
use crate::render::Verdict;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Ask,
    Saved,
    Held,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AskOption {
    pub value: String,
    pub label_ko: String,
    pub meaning_ko: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ask {
    pub field: String,
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
    pub code: String,
    pub message_ko: String,
}
impl Stop {
    pub fn new(code: &str, message_ko: &str) -> Self {
        Self {
            code: code.to_string(),
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
pub struct Check {
    pub kind: String,
    pub scope: String,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreviousLesson {
    pub date: String,
    pub how_found: String,
    pub progress: String,
    pub homework: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StudentLesson {
    pub student: String,
    pub previous_lesson: Option<PreviousLesson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searched_range: Option<String>,
    pub checks: Vec<Check>,
    pub carry_over: Vec<String>,
    pub excluded: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LessonData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub lesson_date: String,
    pub students: Vec<StudentLesson>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Envelope {
    pub status: Status,
    pub display: String,
    pub data: LessonData,
    pub ask: Option<Ask>,
    pub next: Option<NextCall>,
    pub stop: Option<Stop>,
    pub coverage: Coverage,
}

impl Envelope {
    /// The one constructor enforcing the invariant: exactly one of
    /// `ask|next|stop` is non-null when status is not `ok|saved`, all null
    /// otherwise.
    pub fn new(
        status: Status,
        display: String,
        data: LessonData,
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
    pub fn ok(display: String, data: LessonData, coverage: Coverage) -> Self {
        Self::new(Status::Ok, display, data, None, None, None, coverage).expect("ok invariant")
    }
    pub fn ask(display: String, data: LessonData, ask: Ask, coverage: Coverage) -> Self {
        Self::new(Status::Ask, display, data, Some(ask), None, None, coverage)
            .expect("ask invariant")
    }
    pub fn stop(display: String, data: LessonData, stop: Stop, coverage: Coverage) -> Self {
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
    pub fn unavailable_tool() -> Self {
        Self::stop(
            "이 도구는 아직 제공되지 않습니다.".into(),
            LessonData::default(),
            Stop::new("tool_not_in_slice_1", "이 도구는 아직 제공되지 않습니다."),
            Coverage::default(),
        )
    }
}
