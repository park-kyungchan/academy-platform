//! Unit status types. Source observations and teacher judgments never share a field.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitStatusRequest {
    pub student: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub book: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub unit: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub unit_from: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub unit_to: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub topic: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitVerdict {
    Waiting,
    Completed,
    Found,
    NotYetVisible,
    NotInReadRange,
    NotObserved,
    NotObservedInEitherSource,
    Unknown,
    Eligible,
    NotEligible,
    Excluded,
    Confirmed,
    Deferred,
    Unjudged,
    NotApplicable,
    NotYetExpected,
    TeacherReportedNoServer,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EditionTree {
    pub edition: String,
    pub family: String,
    pub school_level: String,
    pub read_at: String,
    pub chapters: Vec<Chapter>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Chapter {
    pub code: String,
    pub title: String,
    #[serde(default)]
    pub middles: Vec<Middle>,
    #[serde(default)]
    pub minors: Vec<Minor>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Middle {
    pub code: String,
    pub title: String,
    pub minors: Vec<Minor>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Minor {
    pub code: String,
    pub title: String,
    pub activities: Vec<String>,
    #[serde(default)]
    pub unit_wide: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Leaf {
    pub chapter: String,
    pub middle: Option<String>,
    pub minor: Minor,
}
impl EditionTree {
    pub fn leaves(&self) -> Vec<Leaf> {
        self.chapters
            .iter()
            .flat_map(|c| {
                let mut leaves: Vec<_> = c
                    .minors
                    .iter()
                    .map(|m| Leaf {
                        chapter: c.code.clone(),
                        middle: None,
                        minor: m.clone(),
                    })
                    .collect();
                leaves.extend(c.middles.iter().flat_map(|mid| {
                    mid.minors.iter().map(|m| Leaf {
                        chapter: c.code.clone(),
                        middle: Some(format!("{} {}", mid.code, mid.title)),
                        minor: m.clone(),
                    })
                }));
                leaves
            })
            .collect()
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TeacherJudgment {
    pub student_key: String,
    pub edition: String,
    pub unit: String,
    pub topic: String,
    pub activity: Option<String>,
    pub level: Option<String>,
    pub stage: Option<String>,
    pub snapshot: Option<String>,
    pub verdict: UnitVerdict,
    pub actor: Option<String>,
    pub recorded_at: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExcludedSection {
    pub student_key: String,
    pub edition: String,
    pub unit: String,
    pub activity: String,
    pub state: String,
    pub actor: Option<String>,
    pub recorded_at: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DailyScope {
    pub from: String,
    pub to: String,
    pub selector_label: String,
    pub page_label: String,
    pub complete: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExpectedToday {
    pub edition: String,
    pub unit: String,
    pub level: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DailyAttempt {
    pub paper_key: String,
    pub attempt_key: String,
    pub edition_mapping: Option<String>,
    pub unit_mapping: Option<String>,
    pub source_title: String,
    pub level: Option<String>,
    pub occurred_at: Option<String>,
    pub score: Option<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DailyFrame {
    pub student_key: String,
    pub read_at: String,
    pub scope: DailyScope,
    #[serde(default)]
    pub expected_today: Vec<ExpectedToday>,
    #[serde(default)]
    pub attempts: Vec<DailyAttempt>,
    /// Synthetic mapping facts, joined only after the actual History header identity check.
    #[serde(default)]
    pub headers: Vec<DailyAttempt>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct DailyRead {
    pub sources: Vec<crate::envelope::SourceRead>,
    pub read_at: String,
    pub scope: DailyScope,
    pub expected_today: Vec<ExpectedToday>,
    pub attempts: Vec<DailyAttempt>,
    pub unknowns: Vec<String>,
    pub observed: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Item {
    pub outer_lecture_key: Option<String>,
    pub ordinal: String,
    pub label: Option<String>,
    pub scoring: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Section {
    pub student_key: String,
    pub student_name: String,
    pub course_key: String,
    pub book_no: String,
    pub unit: String,
    pub activity: String,
    pub source_title: String,
    pub paper_key: String,
    pub attempt_key: String,
    pub score: f64,
    pub items: Vec<Item>,
    pub source_date: Option<String>,
    pub date_precision: String,
}
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct SectionRead {
    pub records: Vec<Section>,
    pub absent_units: Vec<String>,
    pub index_observed: bool,
    pub paper_observed: bool,
    pub unknowns: Vec<String>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ClinicCell {
    pub unit: String,
    pub activity: String,
    pub verdict: UnitVerdict,
    pub labels: Vec<String>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct UnitData {
    pub student_key: String,
    pub student: String,
    pub edition: String,
    pub family: String,
    pub topic: String,
    pub default_read_at: String,
    pub read_at: String,
    pub tree_read_at: String,
    pub minors: Vec<Leaf>,
    pub activity: Option<String>,
    pub level: Option<String>,
    pub daily: Option<DailyRead>,
    pub sections: Option<SectionRead>,
    pub judgments: Vec<TeacherJudgment>,
    pub excluded: Vec<ExcludedSection>,
    pub stages: Vec<crate::unit::Stage>,
    pub prestudy: Vec<crate::unit::Stage>,
    pub clinic: Vec<ClinicCell>,
    pub first_omitted: Option<String>,
}

fn optional_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
