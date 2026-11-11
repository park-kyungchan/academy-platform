//! Synthetic frame state and raw History port. No transport or real account.
use crate::synthetic::{Scenario, TEACHER_ID};
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{Error, Result, obj, string},
    history::{Port, decode_code},
};

pub use crate::backend::ReadClock;
pub use crate::unit_data::SharedFrameClock;
pub struct SyntheticDailyTestPort {
    scenario: Scenario,
    clock: SharedFrameClock,
}
impl SyntheticDailyTestPort {
    pub fn new(scenario: Scenario, clock: SharedFrameClock) -> Self {
        Self { scenario, clock }
    }
}
impl Port for SyntheticDailyTestPort {
    fn dated_students(&mut self, _input: &Js) -> Result<Js> {
        let frame = self.clock.active()?;
        let student = self
            .scenario
            .student_by_key(&frame.student_key)
            .ok_or_else(|| Error::error("synthetic_student_missing"))?;
        Ok(Js::Arr(vec![obj(&[
            ("teacherKey", TEACHER_ID.into()),
            ("studentKey", student.key.clone().into()),
            ("studentName", student.name.clone().into()),
            ("date", frame.scope.to.into()),
        ])]))
    }
    fn daily_page(&mut self, student: &Js, selector: &Js) -> Result<Js> {
        let frame = self.clock.active()?;
        // History mandates an admitted relative selector in addition to the current page.
        // Its empty response is represented explicitly; it is not a second whole history.
        let rows = if selector.is("1") {
            frame
                .attempts
                .iter()
                .map(|a| {
                    obj(&[
                        ("priNo", frame.student_key.clone().into()),
                        ("userName", student.get("studentName").clone()),
                        ("pNo", a.paper_key.clone().into()),
                        ("testingNo", a.attempt_key.clone().into()),
                        ("score", a.score.map_or(Js::Null, Js::Num)),
                    ])
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(obj(&[
            ("selector", selector.clone()),
            ("operation", "DtResultSearch".into()),
            ("page", Js::Num(1.0)),
            ("rawRows", Js::Arr(rows)),
            ("relativeSelectors", Js::Arr(vec!["11".into()])),
            ("observedAt", frame.read_at.into()),
        ]))
    }
    fn paper_header(&mut self, student: &Js, code: &Js) -> Result<Js> {
        let frame = self.clock.active()?;
        let key = string(&decode_code(code)?);
        let header = frame
            .headers
            .iter()
            .find(|h| h.paper_key == key)
            .ok_or_else(|| Error::error("synthetic_header_missing"))?;
        Ok(obj(&[
            ("studentId", student.clone()),
            ("paperKey", header.paper_key.clone().into()),
            ("paperCode", code.clone()),
            (
                "metadataRows",
                Js::Arr(vec![obj(&[
                    ("label", "synthetic source title".into()),
                    ("value", header.source_title.clone().into()),
                ])]),
            ),
            ("sourceText", header.source_title.clone().into()),
            ("observedAt", frame.read_at.into()),
        ]))
    }
}
pub fn validate(s: &Scenario) -> std::result::Result<(), String> {
    if let Some(at) = &s.unit_read_at {
        chrono::DateTime::parse_from_rfc3339(at).map_err(|_| "unit_clock_invalid")?;
    }
    let mut editions = std::collections::HashSet::new();
    for tree in &s.edition_trees {
        if !editions.insert(&tree.edition) {
            return Err("edition_tree_duplicate".into());
        }
        chrono::DateTime::parse_from_rfc3339(&tree.read_at).map_err(|_| "tree_clock_invalid")?;
        let mut codes = std::collections::HashSet::new();
        for leaf in tree.leaves() {
            if !codes.insert(leaf.minor.code) {
                return Err("edition_leaf_duplicate".into());
            }
        }
    }
    for f in &s.dailytest_frames {
        if s.student_by_key(&f.student_key).is_none() {
            return Err("daily_student_unknown".into());
        }
        chrono::DateTime::parse_from_rfc3339(&f.read_at).map_err(|_| "daily_clock_invalid")?;
        let from = chrono::NaiveDate::parse_from_str(&f.scope.from, "%Y-%m-%d")
            .map_err(|_| "daily_scope_invalid")?;
        let to = chrono::NaiveDate::parse_from_str(&f.scope.to, "%Y-%m-%d")
            .map_err(|_| "daily_scope_invalid")?;
        if from > to || f.scope.complete {
            return Err("daily_scope_not_observed_page".into());
        }
        let mut keys = std::collections::HashSet::new();
        for a in &f.attempts {
            if !keys.insert((&a.paper_key, &a.attempt_key)) {
                return Err("daily_attempt_duplicate".into());
            }
            edu_academy_sources::history::encode_code(&a.paper_key.as_str().into())
                .map_err(|_| "daily_paper_key_invalid")?;
        }
    }
    Ok(())
}
