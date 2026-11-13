//! Synthetic authoring policy. Dispositions and validation never authorize effects.
use crate::{
    facts::{SeededFacts, TeacherFacts},
    labels,
    record_data::*,
    record_fixture::AnswerRecord,
    record_parent::render_parent,
    record_parse::{ParsedLines, parse_lines},
};
use edu_academy_core::{
    js::{Fault, Js, Result, obj, sha},
    templates,
    workflow::{
        Runtime, canonical,
        review::{self, Policy},
    },
};

/// Map only explicit labels/codes, then use the owning core value validator.
pub fn wire_value(field: RecordField, raw: &str) -> Result<String> {
    let mut value = raw.to_owned();
    if !review::valid_value(field.wire_name(), &value.clone().into()) {
        value = match field {
            RecordField::Attendance => ["Y", "N", "L", " "]
                .into_iter()
                .find(|code| labels::attendance_label(code) == Some(raw))
                .map(str::to_owned),
            RecordField::DailyTestAssessment | RecordField::PreviousHomeworkAssessment => {
                let name = if field == RecordField::DailyTestAssessment {
                    "daily_test"
                } else {
                    "prev_homework"
                };
                labels::wire_code(name, raw).map(|code| code.to_string())
            }
            _ => None,
        }
        .ok_or_else(|| Fault::error("invalid_explicit_field_value"))?;
    }
    if !review::valid_value(field.wire_name(), &value.clone().into()) {
        return Err(Fault::error("invalid_explicit_field_value"));
    }
    Ok(value)
}

/// Validate BEFORE changing teacher inputs; the draft owner still checks revision.
pub fn apply_choice(
    input: &mut TeacherInputs,
    field: AnswerField,
    answer: &AnswerValue,
) -> Result<()> {
    use AnswerField as A;
    let target = match field {
        A::Progress => RecordField::Progress,
        A::Homework => RecordField::Homework,
        A::Memo => RecordField::LessonMemo,
        A::Attendance => RecordField::Attendance,
        A::DailyTest => RecordField::DailyTestAssessment,
        A::PrevHomework => RecordField::PreviousHomeworkAssessment,
        A::Replace | A::Restart => {
            let AnswerValue::Boolean(value) = answer else {
                return Err(Fault::error("record_question_answer_shape"));
            };
            if field == A::Replace {
                input.replace = Some(*value);
            } else {
                input.restart = Some(*value);
            }
            return Ok(());
        }
        _ => return Err(Fault::error("record_answer_field_not_owned")),
    };
    if matches!(field, A::Progress | A::Homework) {
        let AnswerValue::Lines(lines) = answer else {
            return Err(Fault::error("record_question_answer_shape"));
        };
        wire_value(target, &lines.join("\n"))?;
        if field == A::Progress {
            input.progress = Some(lines.clone());
        } else {
            input.homework = Some(lines.clone());
        }
    } else {
        let AnswerValue::Text(text) = answer else {
            return Err(Fault::error("record_question_answer_shape"));
        };
        wire_value(target, text)?;
        let destination = match field {
            A::Memo => &mut input.memo,
            A::Attendance => &mut input.attendance,
            A::DailyTest => &mut input.daily_test,
            A::PrevHomework => &mut input.prev_homework,
            _ => unreachable!(),
        };
        *destination = Some(text.clone());
    }
    Ok(())
}

/// Teacher-input dispositions only, NOT preview eligibility or saved status.
/// A proposal must also pass RecordPolicy::review / core Service::review.
pub fn settle_fields(
    before: &AnswerRecord,
    input: &TeacherInputs,
) -> Result<RecordFields<FieldDisposition>> {
    use FieldDisposition::*;
    let RecordObservation::Observed { fields, .. } = &before.snapshot.observation else {
        return Err(Fault::error("record_source_unavailable"));
    };
    let supplied = RecordFields {
        progress: input.progress.as_ref().map(|v| v.join("\n")),
        homework: input.homework.as_ref().map(|v| v.join("\n")),
        lesson_memo: input.memo.clone(),
        attendance: input.attendance.clone(),
        daily_test_assessment: input.daily_test.clone(),
        previous_homework_assessment: input.prev_homework.clone(),
    };
    let mut result = RecordFields::from_fn(|_| Unresolved);
    for field in RecordField::ALL {
        if field == RecordField::PreviousHomeworkAssessment {
            let target = before.field_evidence[field].get("target");
            let date = String::from_utf16(target.get("lessonDate").units()).unwrap_or_default();
            let valid = ["sourceNamespace", "teacherId", "studentId"]
                .iter()
                .all(|key| target.get(key) == before.answer.get("scope").get(key))
                && target
                    .get("studentId")
                    .is(&before.snapshot.identity.student_key)
                && chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_ok()
                && date < before.snapshot.identity.lesson_date;
            if !valid {
                if supplied[field].is_some() {
                    return Err(Fault::error("record_previous_target_unverified"));
                }
                continue;
            }
        }
        let RecordValue::Known(old) = &fields[field] else {
            if supplied[field].is_none() {
                continue;
            }
            // Explicit input can resolve an unknown intent; core still blocks an
            // unqualified before-state. No Unknown -> blank conversion occurs.
            if let Some(raw) = &supplied[field] {
                let value = wire_value(field, raw)?;
                result[field] = if field == RecordField::LessonMemo && raw == "없음" {
                    NoAddition
                } else if matches!(
                    field,
                    RecordField::DailyTestAssessment | RecordField::PreviousHomeworkAssessment
                ) && value == "-1"
                {
                    Deferred
                } else {
                    ProposalConfirmed
                };
            }
            continue;
        };
        let Some(raw) = &supplied[field] else {
            let unassessed = matches!(
                field,
                RecordField::DailyTestAssessment | RecordField::PreviousHomeworkAssessment
            ) && old == "-1";
            if !old.trim().is_empty() && !unassessed {
                result[field] = ConfirmedExisting;
            }
            continue;
        };
        let value = wire_value(field, raw)?;
        result[field] = if field == RecordField::LessonMemo && raw == "없음" {
            NoAddition
        } else if matches!(
            field,
            RecordField::DailyTestAssessment | RecordField::PreviousHomeworkAssessment
        ) && value == "-1"
        {
            Deferred
        } else if *old == value {
            ConfirmedExisting
        } else if matches!(
            field,
            RecordField::Progress | RecordField::Homework | RecordField::LessonMemo
        ) && !old.is_empty()
            && !value.is_empty()
            && input.replace != Some(true)
        {
            if input.replace == Some(false) {
                ConfirmedExisting
            } else {
                Unresolved
            }
        } else {
            ProposalConfirmed
        };
    }
    Ok(result)
}

pub struct RecordPolicy {
    facts: SeededFacts,
    catalog: Js,
}
impl RecordPolicy {
    pub fn new(facts: SeededFacts, catalog: Js) -> Self {
        Self { facts, catalog }
    }
    /// Runs the actual core schema, value, fact/count/deadline and policy hooks.
    /// This pure review owns no journal; E1's Service owns durable publication.
    pub fn review(&self, input: &Js, held: &Js, fresh: &Js, runtime: &dyn Runtime) -> Result<Js> {
        review::review(input, held, fresh, self, runtime)
    }
    fn learning(&self, scope: &Js, value: &Js, entries: &Js) -> Result<()> {
        if !self.scope_qualified(scope) {
            return Err(Fault::error(
                "student_namespace_bridge_unverified_ask_teacher",
            ));
        }
        let text = String::from_utf16(value.units())
            .map_err(|_| Fault::error("learning_render_mismatch"))?;
        let mut date = None;
        let mut lines = Vec::new();
        for line in text.split('\n') {
            if !line.contains(", ")
                && let Some(header) = line.strip_prefix("- ").and_then(|s| s.strip_suffix("까지"))
            {
                date = Some(header);
            } else {
                lines.push(if let Some(date) = date {
                    format!("{line} {date}까지")
                } else {
                    line.into()
                });
            }
        }
        let ParsedLines::Ready(parsed) = parse_lines(RecordField::Progress, &lines, &self.catalog)?
        else {
            return Err(Fault::error("learning_render_mismatch"));
        };
        if render_parent(&parsed, &self.catalog)?.progress.as_deref() != Some(&text) {
            return Err(Fault::error("learning_render_mismatch"));
        }
        let projected: Vec<_> = parsed
            .iter()
            .map(|e| {
                obj(&[
                    ("bookRef", e.book_ref.clone().into()),
                    ("unitRef", e.unit_ref.clone().into()),
                    (
                        "scope",
                        if e.scope == EntryScope::Unit {
                            "unit"
                        } else {
                            "sections"
                        }
                        .into(),
                    ),
                    (
                        "sectionRefs",
                        Js::Arr(e.section_refs.iter().cloned().map(Js::from).collect()),
                    ),
                    ("activity", e.activity.clone().into()),
                    ("deadline", e.deadline.clone().map_or(Js::Null, Js::from)),
                ])
            })
            .collect();
        if learning_keys(&Js::Arr(projected))? != learning_keys(entries)? {
            return Err(Fault::error("learning_render_mismatch"));
        }
        Ok(())
    }
}
fn learning_keys(entries: &Js) -> Result<Vec<String>> {
    let mut keys = Vec::new();
    for e in entries.array() {
        let sections = if e.get("scope").is("sections") {
            e.get("sectionRefs").array().to_vec()
        } else {
            vec![Js::Null]
        };
        for section in sections {
            keys.push(canonical(&Js::Arr(vec![
                e.get("bookRef").clone(),
                e.get("unitRef").clone(),
                e.get("scope").clone(),
                section,
                e.get("activity").clone(),
                e.get("deadline").clone(),
            ]))?);
        }
    }
    keys.sort();
    Ok(keys)
}
impl Policy for RecordPolicy {
    fn parent_pins(&self) -> Js {
        obj(&[
            (
                "sourceRef",
                "education-platform:synthetic-record-policy".into(),
            ),
            ("policySha256", sha(include_str!("record_policy.rs")).into()),
            (
                "safeguardSourceSha256",
                sha(include_str!("../../edu-academy-core/src/templates.rs")).into(),
            ),
        ])
    }
    fn learning_pins(&self) -> Js {
        canonical(&self.catalog).map_or(Js::Null, |catalog| {
            obj(&[
                ("bundleSha256", sha(include_str!("record_parent.rs")).into()),
                (
                    "configSha256",
                    sha(self.facts.read_at().unwrap_or("unobserved")).into(),
                ),
                ("catalogSha256", sha(&catalog).into()),
            ])
        })
    }
    fn check_lines(&self, value: &Js) -> bool {
        value.string()
            && String::from_utf16(value.units()).is_ok_and(|text| {
                !text.contains('\r')
                    && text
                        .split('\n')
                        .all(|line| line.starts_with("- ") && line.len() > "- ".len())
            })
    }
    fn excluded_parent_language(&self, value: &Js) -> bool {
        templates::parent_internal(value)
    }
    fn scope_qualified(&self, scope: &Js) -> bool {
        let books = self.catalog.get("books").array();
        !books.is_empty()
            && books
                .iter()
                .all(|b| b.get("studentRef") == scope.get("studentId"))
    }
    fn learning_review(&self, scope: &Js, value: &Js, entries: &Js) -> Vec<Js> {
        match self.learning(scope, value, entries) {
            Ok(()) => vec![],
            Err(_) => vec!["learning_render_mismatch".into()],
        }
    }
}
