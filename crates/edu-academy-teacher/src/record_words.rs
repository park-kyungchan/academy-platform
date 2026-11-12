//! Question content and lexical words only; no draft, revision or approval gate.
use crate::{display_labels as l, record_data::*};
use edu_academy_core::js::{Fault, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQuestion {
    pub field: AnswerField,
    pub question: String,
    pub choices: Vec<QuestionChoice>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct AskSet(Vec<RecordQuestion>);
impl AskSet {
    pub fn new(questions: Vec<RecordQuestion>) -> Result<Self> {
        if questions.is_empty()
            || questions.iter().any(|q| {
                q.question.trim().is_empty()
                    || q.choices
                        .iter()
                        .any(|choice| choice.label.trim().is_empty())
            })
        {
            return Err(Fault::error("record_questions_empty"));
        }
        for question in &questions {
            for choice in &question.choices {
                let valid = match question.field {
                    AnswerField::Progress | AnswerField::Homework => {
                        matches!(choice.answer, AnswerValue::Lines(_))
                    }
                    AnswerField::Replace | AnswerField::Restart => {
                        matches!(choice.answer, AnswerValue::Boolean(_))
                    }
                    _ => matches!(choice.answer, AnswerValue::Text(_)),
                };
                if !valid {
                    return Err(Fault::error("record_question_answer_shape"));
                }
            }
        }
        Ok(Self(questions))
    }
    /// These are content only. The E2 draft owner must bind/check the current
    /// PendingQuestion revision BEFORE reducing it to a RecordQuestion. There
    /// is deliberately no conversion that discards an unbound parser revision.
    pub fn questions(&self) -> &[RecordQuestion] {
        &self.0
    }
    pub fn display(&self) -> String {
        let mut fields = Vec::new();
        let mut lines = Vec::new();
        for question in &self.0 {
            if fields.contains(&question.field) {
                continue;
            }
            fields.push(question.field);
            lines.push(format!("{} 선택", l::record_answer_label(question.field)));
            for question in self.0.iter().filter(|q| q.field == question.field) {
                lines.push(question.question.clone());
                lines.extend(question.choices.iter().map(|c| format!("- {}", c.label)));
            }
        }
        lines.join("\n")
    }
}

/// Exact lexical recognition, not permission to dispatch or release any hold.
pub fn approval_words(words: &str) -> bool {
    matches!(
        words.trim(),
        "저장"
            | "저장해"
            | "저장해줘"
            | "저장해주세요"
            | "저장하세요"
            | "저장하자"
            | "응"
            | "네"
            | "예"
            | "그래"
            | "좋아"
            | "맞아"
            | "ㅇㅇ"
            | "오케이"
    )
}
