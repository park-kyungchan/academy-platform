//! Internal synthetic record contracts; no transport, storage or approval authority.
use edu_academy_core::{
    js::{Fault, Result, sha},
    workflow::{canonical, codec},
};
use serde::{Deserialize, Serialize};
use std::{
    num::NonZeroU64,
    ops::{Index, IndexMut},
};

pub const RECORD_FIELD_COUNT: usize = 6;
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecordField {
    Progress,
    Homework,
    LessonMemo,
    Attendance,
    DailyTestAssessment,
    PreviousHomeworkAssessment,
}
impl RecordField {
    pub const ALL: [Self; RECORD_FIELD_COUNT] = [
        Self::Progress,
        Self::Homework,
        Self::LessonMemo,
        Self::Attendance,
        Self::DailyTestAssessment,
        Self::PreviousHomeworkAssessment,
    ];
    pub fn wire_name(self) -> &'static str {
        edu_academy_sources::native::FIELDS[self as usize]
    }
}
/// A fixed product: omitting a field is never equivalent to observing it unknown.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordFields<T> {
    pub progress: T,
    pub homework: T,
    pub lesson_memo: T,
    pub attendance: T,
    pub daily_test_assessment: T,
    pub previous_homework_assessment: T,
}
impl<T> RecordFields<T> {
    pub fn from_fn(mut f: impl FnMut(RecordField) -> T) -> Self {
        Self {
            progress: f(RecordField::Progress),
            homework: f(RecordField::Homework),
            lesson_memo: f(RecordField::LessonMemo),
            attendance: f(RecordField::Attendance),
            daily_test_assessment: f(RecordField::DailyTestAssessment),
            previous_homework_assessment: f(RecordField::PreviousHomeworkAssessment),
        }
    }
}
impl<T> Index<RecordField> for RecordFields<T> {
    type Output = T;
    fn index(&self, field: RecordField) -> &T {
        match field {
            RecordField::Progress => &self.progress,
            RecordField::Homework => &self.homework,
            RecordField::LessonMemo => &self.lesson_memo,
            RecordField::Attendance => &self.attendance,
            RecordField::DailyTestAssessment => &self.daily_test_assessment,
            RecordField::PreviousHomeworkAssessment => &self.previous_homework_assessment,
        }
    }
}
impl<T> IndexMut<RecordField> for RecordFields<T> {
    fn index_mut(&mut self, field: RecordField) -> &mut T {
        match field {
            RecordField::Progress => &mut self.progress,
            RecordField::Homework => &mut self.homework,
            RecordField::LessonMemo => &mut self.lesson_memo,
            RecordField::Attendance => &mut self.attendance,
            RecordField::DailyTestAssessment => &mut self.daily_test_assessment,
            RecordField::PreviousHomeworkAssessment => &mut self.previous_homework_assessment,
        }
    }
}
/// Known("") is a known blank; Unknown never supplies a default or display label.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RecordValue {
    Known(String),
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OwnerScope {
    pub owner: String,
    pub account: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    DailyRecord,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordIdentity {
    pub owner: OwnerScope,
    pub student_key: String,
    pub lesson_date: String,
    pub kind: RecordKind,
}
impl RecordIdentity {
    pub fn key(&self) -> Result<String> {
        content_hash(self)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordProvenance {
    pub read_at: String,
    pub evidence_refs: Vec<String>,
    pub source_revision: Option<String>,
    pub wire_digest: Option<String>,
    /// Attribution/coverage reasons are separate from failed source reads.
    pub field_reasons: RecordFields<Option<String>>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordObservation {
    Observed {
        occurrence: String,
        fields: RecordFields<RecordValue>,
    },
    NotObserved,
    Failed {
        reason_code: String,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordSnapshot {
    pub identity: RecordIdentity,
    pub requested_date: String,
    pub provenance: RecordProvenance,
    pub observation: RecordObservation,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldDisposition {
    ConfirmedExisting,
    ProposalConfirmed,
    NoAddition,
    Deferred,
    Unresolved,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    Set,
    Replace,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldPatch {
    pub effect_target: String,
    pub operation: PatchOperation,
    pub value: String,
}
/// None means unsent, including preserved unknown/legacy values. Some with "" clears.
pub type RecordPatch = RecordFields<Option<FieldPatch>>;
/// Content approval A only; a matching binding does not establish the process/time gate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentBinding {
    pub identity: RecordIdentity,
    pub generation: String,
    pub patch_hash: String,
    pub source_hash: String,
}
impl ContentBinding {
    pub fn new(
        identity: RecordIdentity,
        generation: String,
        patch: &RecordPatch,
        source_values: &RecordFields<RecordValue>,
    ) -> Result<Self> {
        Ok(Self {
            identity,
            generation,
            patch_hash: content_hash(patch)?,
            source_hash: content_hash(source_values)?,
        })
    }
}
/// Typed fixed-order values go through the existing canonical JSON and SHA functions.
/// Metadata, attempt references, revisions and read times are not inputs to A.
pub fn content_hash(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|_| Fault::error("record_data_not_json"))?;
    Ok(sha(&canonical(&codec::parse(&bytes)?)?))
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeacherInputs {
    pub progress: Option<Vec<String>>,
    pub homework: Option<Vec<String>>,
    pub memo: Option<String>,
    pub attendance: Option<String>,
    pub daily_test: Option<String>,
    pub prev_homework: Option<String>,
    pub replace: Option<bool>,
    pub restart: Option<bool>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryScope {
    Unit,
    Sections,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordEntry {
    pub field: RecordField,
    pub raw_line: String,
    pub book_ref: String,
    pub unit_ref: String,
    pub unit_code: String,
    pub minor_title: String,
    pub activity: String,
    pub qualifiers: Vec<String>,
    pub scope: EntryScope,
    pub section_refs: Vec<String>,
    pub deadline: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerField {
    Student,
    Date,
    Progress,
    Homework,
    Memo,
    Attendance,
    DailyTest,
    PrevHomework,
    Replace,
    Restart,
    TeacherWords,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AnswerValue {
    Text(String),
    Lines(Vec<String>),
    Boolean(bool),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuestionChoice {
    pub label: String,
    pub answer: AnswerValue,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PendingQuestion {
    pub field: AnswerField,
    pub reason_code: String,
    pub text: String,
    pub choices: Vec<QuestionChoice>,
    pub revision: NonZeroU64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PendingAttempt {
    pub request_ref: String,
    pub ordinal: NonZeroU64,
    pub instruction_hash: String,
    pub binding: ContentBinding,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FailedAttemptSummary {
    pub binding: ContentBinding,
    pub definite_no_effect_batches: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftState {
    Open,
    Saved,
    Expired,
}
/// Durable intent only. PresentedContent/process id/monotonic display time belong
/// exclusively to the later RecordSession and cannot be restored from this object.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub identity: RecordIdentity,
    pub generation: String,
    pub revision: NonZeroU64,
    pub before: RecordSnapshot,
    pub binding: Option<ContentBinding>,
    pub next_attempt_ordinal: NonZeroU64,
    pub pending_attempt: Option<PendingAttempt>,
    pub failed_attempts: Option<FailedAttemptSummary>,
    pub editable: RecordFields<bool>,
    pub dispositions: RecordFields<FieldDisposition>,
    pub teacher_inputs: TeacherInputs,
    pub entries: Vec<RecordEntry>,
    pub patch: RecordPatch,
    pub rendered_preview: String,
    pub preview_hash: String,
    pub pending_questions: Vec<PendingQuestion>,
    pub created: String,
    pub last_used: String,
    pub closed_at: Option<String>,
    pub state: DraftState,
}
