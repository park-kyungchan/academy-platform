//! Tree-ordered display, qualified evidence and complete-leaf byte bounds.
use crate::{
    display_labels as l,
    envelope::Coverage,
    render::cell,
    unit::{dependent_stage, leaf_label},
    unit_data::*,
};
use chrono::{DateTime, Datelike, Timelike};
pub fn verdict(v: UnitVerdict) -> &'static str {
    match v {
        UnitVerdict::Found => l::SERVER,
        UnitVerdict::Waiting => l::WAITING,
        UnitVerdict::Completed => l::TEACHER,
        UnitVerdict::NotYetVisible => l::NOT_YET,
        UnitVerdict::NotInReadRange => l::NOT_IN_RANGE,
        UnitVerdict::NotObserved => l::NOT_OBSERVED,
        UnitVerdict::NotObservedInEitherSource => l::NOT_IN_EITHER,
        UnitVerdict::Eligible => l::ELIGIBLE,
        UnitVerdict::NotEligible => l::NOT_ELIGIBLE,
        UnitVerdict::Excluded => l::EXCLUDED,
        UnitVerdict::Confirmed => l::UNIT_CONFIRMED,
        UnitVerdict::Deferred => l::DEFERRED,
        UnitVerdict::Unjudged => l::UNJUDGED,
        UnitVerdict::NotApplicable => l::NOT_APPLICABLE,
        UnitVerdict::NotYetExpected => l::UNIT_NOT_EXPECTED,
        UnitVerdict::TeacherReportedNoServer => l::UNIT_TEACHER_ONLY,
        UnitVerdict::Unknown => l::UNAVAILABLE,
    }
}
fn time(at: &str) -> String {
    DateTime::parse_from_rfc3339(at)
        .map(|d| {
            let d = d.with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).expect("offset"));
            format!(
                "{}월 {}일 {:02}:{:02}",
                d.month(),
                d.day(),
                d.hour(),
                d.minute()
            )
        })
        .unwrap_or_else(|_| cell(at))
}
fn scoped(unit: &str, level: Option<&str>) -> String {
    format!(
        "{}{}",
        cell(unit),
        level.map(|l| format!(" {}", cell(l))).unwrap_or_default()
    )
}
fn judgments<'a>(d: &'a UnitData, code: &str, topic: &str) -> Vec<&'a TeacherJudgment> {
    d.judgments
        .iter()
        .filter(|j| {
            j.unit == code
                && j.topic == topic
                && (d
                    .level
                    .as_ref()
                    .is_none_or(|level| j.level.as_ref() == Some(level)))
                && (d
                    .activity
                    .as_ref()
                    .is_none_or(|a| j.activity.as_ref().is_none_or(|ja| ja == a)))
        })
        .collect()
}
fn daily_attempts<'a>(d: &'a UnitData, code: &str) -> Vec<&'a DailyAttempt> {
    d.daily
        .as_ref()
        .map(|r| {
            r.attempts
                .iter()
                .filter(|a| a.unit_mapping.as_deref() == Some(code))
                .collect()
        })
        .unwrap_or_default()
}
fn daily_state(d: &UnitData, code: &str, level: Option<&str>) -> UnitVerdict {
    if d.family == "supplementary" {
        return UnitVerdict::NotApplicable;
    }
    match &d.daily {
        None => UnitVerdict::Unknown,
        Some(r) if !r.observed || !r.unknowns.is_empty() => UnitVerdict::Unknown,
        Some(r)
            if r.expected_today
                .iter()
                .any(|e| e.unit == code && level.is_none_or(|l| e.level == l)) =>
        {
            UnitVerdict::NotYetVisible
        }
        Some(_) => UnitVerdict::NotInReadRange,
    }
}
fn daily_block(d: &UnitData, leaf: &Leaf, grid: bool) -> (String, Vec<String>) {
    let code = &leaf.minor.code;
    let attempts = daily_attempts(d, code);
    let mut details = Vec::new();
    if grid {
        let state = if attempts.is_empty() {
            verdict(daily_state(d, code, None)).into()
        } else {
            attempts
                .iter()
                .map(|a| l::unit_source(a.score))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let js = judgments(d, code, l::UNIT_TOPICS[1]);
        for a in &attempts {
            let scope = scoped(code, a.level.as_deref());
            details.push(l::unit_date_line(
                &scope,
                a.occurred_at.as_deref().map(cell).as_deref(),
            ));
            details.push(l::unit_level_line(
                &scope,
                a.level.as_deref().map(cell).as_deref(),
            ));
            let current = js.iter().find(|j| {
                j.level == a.level
                    && j.snapshot
                        .as_ref()
                        .map_or(attempts.len() == 1, |s| s == &a.attempt_key)
            });
            details.push(l::unit_judgment_line(
                &scope,
                &current
                    .map(|j| cell(&l::unit_judgment(j)))
                    .unwrap_or_else(|| l::UNJUDGED.into()),
            ));
        }
        for j in js.iter().filter(|j| {
            attempts.is_empty()
                || j.snapshot.as_ref().map_or(attempts.len() != 1, |s| {
                    !attempts.iter().any(|a| &a.attempt_key == s)
                })
        }) {
            details.push(l::unit_judgment_line(
                &scoped(code, j.level.as_deref()),
                &cell(&l::unit_judgment(j)),
            ));
        }
        (format!("{} | {state}", cell(&leaf_label(leaf))), details)
    } else {
        let mut levels: Vec<Option<String>> = Vec::new();
        for a in &attempts {
            if !levels.contains(&a.level) {
                levels.push(a.level.clone());
            }
        }
        if let Some(r) = &d.daily {
            for e in r.expected_today.iter().filter(|e| e.unit == *code) {
                let level = Some(e.level.clone());
                if !levels.contains(&level) {
                    levels.push(level);
                }
            }
        }
        for j in judgments(d, code, l::UNIT_TOPICS[1]) {
            if !levels.contains(&j.level) {
                levels.push(j.level.clone());
            }
        }
        if levels.is_empty() {
            levels.push(d.level.clone());
        }
        let mut lines = Vec::new();
        for level in levels {
            let scope = scoped(code, level.as_deref());
            let matching: Vec<_> = attempts.iter().filter(|a| a.level == level).collect();
            let js: Vec<_> = judgments(d, code, l::UNIT_TOPICS[1])
                .into_iter()
                .filter(|j| j.level == level)
                .collect();
            if !matching.is_empty() {
                for a in &matching {
                    lines.push(l::unit_source_line(&scope, &l::unit_source(a.score)));
                    lines.push(l::unit_date_line(
                        &scope,
                        a.occurred_at.as_deref().map(cell).as_deref(),
                    ));
                    if level.is_none() {
                        lines.push(l::unit_level_line(&scope, None));
                    }
                    let current = js.iter().find(|j| {
                        j.snapshot
                            .as_ref()
                            .map_or(matching.len() == 1, |s| s == &a.attempt_key)
                    });
                    lines.push(l::unit_judgment_line(
                        &scope,
                        &current
                            .map(|j| cell(&l::unit_judgment(j)))
                            .unwrap_or_else(|| l::UNJUDGED.into()),
                    ));
                }
                if matching.len() > 1 {
                    lines.push(l::UNIT_SAME_ATTEMPT.into());
                }
                for j in js.iter().filter(|j| {
                    j.snapshot.as_ref().map_or(matching.len() != 1, |s| {
                        !matching.iter().any(|a| &a.attempt_key == s)
                    })
                }) {
                    lines.push(l::unit_judgment_line(&scope, &cell(&l::unit_judgment(j))));
                }
            } else {
                if js.is_empty()
                    || d.daily.as_ref().is_some_and(|r| {
                        r.expected_today
                            .iter()
                            .any(|e| e.unit == *code && Some(&e.level) == level.as_ref())
                    })
                {
                    lines.push(l::unit_source_line(
                        &scope,
                        verdict(daily_state(d, code, level.as_deref())),
                    ));
                }
                if js.is_empty() {
                    lines.push(l::unit_judgment_line(&scope, l::UNJUDGED));
                } else {
                    for j in js {
                        lines.push(l::unit_judgment_line(&scope, &cell(&l::unit_judgment(j))));
                    }
                }
            }
        }
        (lines.join("\n"), details)
    }
}
fn clinic_block(d: &UnitData, leaf: &Leaf) -> (String, Vec<String>) {
    let code = &leaf.minor.code;
    let mut fields = vec![cell(&leaf_label(leaf))];
    let mut details = Vec::new();
    for a in &l::UNIT_ACTIVITIES[..3] {
        let c = d
            .clinic
            .iter()
            .find(|c| c.unit == *code && c.activity == *a);
        fields.push(
            c.map(|c| {
                if c.verdict == UnitVerdict::Unknown {
                    l::UNIT_DASH
                } else {
                    verdict(c.verdict)
                }
            })
            .unwrap_or(l::UNIT_DASH)
            .into(),
        );
        if let Some(c) = c {
            match c.verdict {
                UnitVerdict::Eligible => details.push(l::unit_clinic_basis(
                    &cell(code),
                    a,
                    &c.labels
                        .iter()
                        .map(|v| cell(v))
                        .collect::<Vec<_>>()
                        .join(", "),
                )),
                UnitVerdict::Excluded => details.push(l::unit_clinic_excluded(&cell(code), a)),
                _ => {}
            }
        }
    }
    for j in judgments(d, code, l::UNIT_TOPICS[2]) {
        details.push(l::unit_clinic_judgment(
            &cell(code),
            &cell(&l::unit_judgment(j)),
        ));
    }
    (fields.join(" | "), details)
}
fn section_block(d: &UnitData, leaf: &Leaf, grid: bool) -> String {
    let code = &leaf.minor.code;
    let records: Vec<_> = d
        .sections
        .as_ref()
        .map(|r| r.records.iter().filter(|r| r.unit == *code).collect())
        .unwrap_or_default();
    if grid {
        let mut fields = vec![cell(&leaf_label(leaf))];
        for activity in section_columns(d) {
            let titles = records
                .iter()
                .filter(|r| r.activity == activity)
                .map(|r| cell(&r.source_title))
                .collect::<Vec<_>>();
            fields.push(if titles.is_empty() {
                l::UNIT_DASH.into()
            } else {
                titles.join(" / ")
            });
        }
        return fields.join(" | ");
    }
    if records.is_empty() {
        return l::unit_absent(
            &cell(code),
            if d.sections
                .as_ref()
                .is_some_and(|s| s.absent_units.contains(code))
            {
                l::NOT_IN_EITHER
            } else if d
                .sections
                .as_ref()
                .is_some_and(|s| s.index_observed && s.paper_observed)
            {
                l::NOT_OBSERVED
            } else {
                l::UNAVAILABLE
            },
        );
    }
    let mut lines = Vec::new();
    for r in records {
        lines.extend(l::unit_section_details(
            &cell(&r.source_title),
            r.items.iter().filter(|i| i.scoring == "O").count(),
            r.items.len(),
            r.score,
            &l::unit_source_date(
                r.source_date.as_deref().map(cell).as_deref(),
                &r.date_precision,
            ),
        ));
        for item in &r.items {
            if item.scoring == "X" {
                lines.push(l::unit_item(
                    &cell(&item.ordinal),
                    &item.scoring,
                    &item
                        .label
                        .as_deref()
                        .map(cell)
                        .unwrap_or_else(|| l::NOT_OBSERVED.into()),
                ));
            }
        }
    }
    lines.join("\n")
}
fn fact_scope(d: &UnitData) -> String {
    let mut facts = vec![if d.tree_read_at != d.default_read_at {
        l::unit_cached_tree(&cell(&d.tree_read_at))
    } else {
        l::UNIT_TREE_FACT.into()
    }];
    if d.topic == l::UNIT_TOPICS[1] {
        if let Some(r) = &d.daily {
            for e in r
                .expected_today
                .iter()
                .filter(|e| d.minors.iter().any(|l| l.minor.code == e.unit))
            {
                facts.push(l::unit_expected(&cell(&e.unit), &cell(&e.level)));
            }
        }
        for leaf in &d.minors {
            for j in judgments(d, &leaf.minor.code, l::UNIT_TOPICS[1]) {
                let mut description = l::unit_fact_judgment(
                    &cell(&j.unit),
                    j.level.as_deref().map(cell).as_deref(),
                    j,
                );
                if facts
                    .last()
                    .is_some_and(|f| f.starts_with(&format!("{} ", cell(&j.unit))))
                {
                    description = description
                        .strip_prefix(&format!("{} ", cell(&j.unit)))
                        .unwrap_or(&description)
                        .to_string();
                }
                facts.push(description);
            }
        }
    }
    if d.topic == l::UNIT_TOPICS[2] {
        for e in &d.excluded {
            if d.minors.iter().any(|l| l.minor.code == e.unit) {
                facts.push(l::unit_exclusion(&cell(&e.unit), &cell(&e.activity)));
            }
        }
        let unjudged = d
            .minors
            .iter()
            .filter(|leaf| {
                judgments(d, &leaf.minor.code, l::UNIT_TOPICS[2])
                    .iter()
                    .any(|j| j.verdict == UnitVerdict::Unjudged)
            })
            .map(|l| cell(&l.minor.code))
            .collect::<Vec<_>>();
        if !unjudged.is_empty() {
            facts.push(l::unit_clinic_missing_facts(&unjudged.join("·")));
        }
    }
    facts.join("·")
}
fn source_scope(d: &UnitData) -> String {
    let who = cell(&d.student);
    let book = cell(&d.edition);
    if d.topic == l::UNIT_TOPICS[1] {
        if d.family == "supplementary" {
            return l::NOT_APPLICABLE.into();
        }
        return d
            .daily
            .as_ref()
            .map(|r| {
                l::unit_daily_range(
                    &r.scope.from,
                    &r.scope.to,
                    &cell(&r.scope.selector_label),
                    &cell(&r.scope.page_label),
                )
            })
            .unwrap_or_else(|| l::UNAVAILABLE.into());
    }
    if d.topic == l::UNIT_TOPICS[2] {
        return l::unit_section_source(
            &who,
            &book,
            if d.sections.as_ref().is_some_and(|r| r.paper_observed) {
                l::UNIT_SECTION_SCOPE
            } else {
                l::NOT_OBSERVED
            },
            false,
        );
    }
    if d.topic == l::UNIT_TOPICS[3] {
        let code = d
            .minors
            .first()
            .map(|l| cell(&l.minor.code))
            .unwrap_or_default();
        let absent = d.sections.as_ref().is_some_and(|r| {
            d.minors.len() == 1 && r.absent_units.contains(&d.minors[0].minor.code)
        });
        let scope = if absent {
            l::unit_section_absent_scope(&code)
        } else if d.minors.len() == 1 && d.sections.as_ref().is_some_and(|r| !r.records.is_empty())
        {
            l::unit_section_small_scope(&code, d.activity.as_deref().map(cell).as_deref())
        } else {
            l::NOT_OBSERVED.into()
        };
        return l::unit_section_source(&who, &book, &scope, absent);
    }
    l::NOT_OBSERVED.into()
}
fn next(d: &UnitData) -> String {
    if d.family == "supplementary" {
        return l::UNIT_NEXT_SUPPLEMENTARY.into();
    }
    if d.topic == l::UNIT_TOPICS[1] {
        if d.minors.len() > 1 {
            let missing: Vec<_> = d
                .minors
                .iter()
                .filter(|l| daily_attempts(d, &l.minor.code).is_empty())
                .collect();
            if let (Some(from), Some(to)) = (missing.first(), missing.last()) {
                return l::unit_next_daily_range(&cell(&from.minor.code), &cell(&to.minor.code));
            }
        } else if let Some(leaf) = d.minors.first() {
            let attempts = daily_attempts(d, &leaf.minor.code);
            let level = attempts
                .first()
                .and_then(|a| a.level.as_deref())
                .or_else(|| {
                    d.daily
                        .as_ref()
                        .and_then(|r| r.expected_today.first())
                        .map(|e| e.level.as_str())
                });
            return l::unit_next_daily(&scoped(&leaf.minor.code, level), !attempts.is_empty());
        }
    }
    if d.topic == l::UNIT_TOPICS[2] {
        let js = d
            .minors
            .iter()
            .filter(|leaf| !judgments(d, &leaf.minor.code, l::UNIT_TOPICS[2]).is_empty())
            .map(|l| cell(&l.minor.code))
            .collect::<Vec<_>>();
        if !js.is_empty() {
            return l::unit_next_clinic_judgments(&js.join("·"));
        }
        let mut codes = Vec::new();
        for leaf in &d.minors {
            let activities = d
                .clinic
                .iter()
                .filter(|c| c.unit == leaf.minor.code && c.verdict == UnitVerdict::Eligible)
                .map(|c| cell(&c.activity))
                .collect::<Vec<_>>();
            if !activities.is_empty() {
                codes.push(format!(
                    "{} {}",
                    cell(&leaf.minor.code),
                    activities.join("·")
                ));
            }
        }
        if !codes.is_empty() {
            return l::unit_next_clinic(&codes.join("·"));
        }
    }
    if d.topic == l::UNIT_TOPICS[3]
        && let Some(leaf) = d.minors.first()
    {
        return l::unit_next_section(
            &cell(&leaf.minor.code),
            d.activity.as_deref().map(cell).as_deref(),
            d.sections
                .as_ref()
                .is_some_and(|r| r.absent_units.contains(&leaf.minor.code)),
        );
    }
    l::unit_next_generic()
}
fn compose(d: &UnitData, coverage: &Coverage) -> String {
    let who = cell(&d.student);
    let book = cell(&d.edition);
    let small = d.minors.len() == 1;
    let mut lines = Vec::new();
    let mut details = Vec::new();
    match d.topic.as_str() {
        t if t == l::UNIT_TOPICS[1] => {
            lines.push(l::unit_daily_heading(
                &who,
                &book,
                small.then(|| cell(&leaf_label(&d.minors[0]))).as_deref(),
            ));
            if !small {
                lines.push(l::UNIT_DAILY_GRID.into());
            }
            for leaf in &d.minors {
                let (block, extra) = daily_block(d, leaf, !small);
                lines.push(block);
                details.extend(extra);
            }
        }
        t if t == l::UNIT_TOPICS[2] => {
            lines.push(format!("{who} {book} {}", l::UNIT_CLINIC_TITLE));
            lines.push(l::UNIT_CLINIC_GRID.into());
            for leaf in &d.minors {
                let (block, extra) = clinic_block(d, leaf);
                lines.push(block);
                details.extend(extra);
            }
            lines.push(l::UNIT_DASH_LEGEND.into());
            lines.append(&mut details);
            if d.stages
                .iter()
                .filter(|s| s.stage == "original_clinic_solving")
                .all(|s| s.verdict == UnitVerdict::TeacherReportedNoServer)
            {
                lines.push(format!(
                    "{}: {}",
                    l::UNIT_STAGE_NAMES[0],
                    verdict(dependent_stage(None, true))
                ));
            }
            if d.stages
                .iter()
                .filter(|s| s.stage == "original_clinic_grading")
                .all(|s| s.verdict == UnitVerdict::TeacherReportedNoServer)
            {
                lines.push(format!(
                    "{}: {}",
                    l::UNIT_STAGE_NAMES[1],
                    verdict(dependent_stage(None, true))
                ));
            }
            if d.stages
                .iter()
                .filter(|s| s.stage == "similar_clinic")
                .all(|s| s.verdict == UnitVerdict::Unknown)
            {
                lines.push(format!(
                    "{}: {}",
                    l::UNIT_STAGE_NAMES[2],
                    l::UNIT_SOURCE_NOT_OBSERVED
                ));
            }
        }
        t if t == l::UNIT_TOPICS[3] => {
            let scope = if small {
                l::unit_section_identity(
                    &cell(&d.minors[0].chapter),
                    d.minors[0].middle.as_deref().map(cell).as_deref(),
                    &cell(&leaf_label(&d.minors[0])),
                )
            } else {
                String::new()
            };
            lines.push(format!("{who} {book} {scope} {}", l::UNIT_SECTION_TITLE));
            if !small {
                let mut headers = vec![
                    l::UNIT_DAILY_GRID
                        .split(" | ")
                        .next()
                        .unwrap_or("")
                        .to_string(),
                ];
                headers.extend(section_columns(d));
                lines.push(headers.join(" | "));
            }
            for leaf in &d.minors {
                lines.push(section_block(d, leaf, !small));
            }
        }
        _ => {
            lines.push(format!("{who} {book}"));
            for leaf in &d.minors {
                lines.push(cell(&leaf_label(leaf)));
                if d.daily.is_some() {
                    let (block, extra) = daily_block(d, leaf, false);
                    lines.push(block);
                    lines.extend(extra);
                }
                if d.sections.is_some() {
                    lines.push(section_block(d, leaf, false));
                }
            }
            if d.topic == l::UNIT_TOPICS[0] {
                for leaf in &d.minors {
                    let (block, extra) = clinic_block(d, leaf);
                    lines.push(block);
                    lines.extend(extra);
                }
                for (index, stage) in l::UNIT_STAGE_NAMES.iter().enumerate() {
                    lines.push(format!(
                        "{stage}: {}",
                        if index < 2 {
                            l::UNIT_TEACHER_ONLY
                        } else {
                            l::NOT_OBSERVED
                        }
                    ));
                }
            }
            for stage in &d.prestudy {
                lines.push(format!(
                    "{} {}: {}",
                    cell(&stage.unit),
                    cell(&stage.stage),
                    verdict(stage.verdict)
                ));
            }
        }
    }
    lines.extend(details);
    for stage in d.stages.iter().filter(|s| {
        !is_clinic_stage(&s.stage)
            || (d.topic == l::UNIT_TOPICS[2] || d.topic == l::UNIT_TOPICS[0])
                && s.verdict
                    != if s.stage == "original_clinic_solving"
                        || s.stage == "original_clinic_grading"
                    {
                        UnitVerdict::TeacherReportedNoServer
                    } else {
                        UnitVerdict::Unknown
                    }
    }) {
        lines.push(format!(
            "{} {}: {}",
            cell(&stage.unit),
            cell(l::unit_stage_name(&stage.stage)),
            verdict(stage.verdict)
        ));
    }
    for reason in &coverage.unknowns {
        if !reason.starts_with("first_omitted:") {
            let (code, scope) = reason.split_once(':').unwrap_or((reason.as_str(), ""));
            let description = if scope.is_empty() {
                l::unit_unknown_reason(code).to_string()
            } else {
                format!("{}·{}", cell(scope), l::unit_unknown_reason(code))
            };
            lines.push(l::unit_unknown_line(&description));
        }
    }
    if let Some(omitted) = &d.first_omitted {
        lines.push(l::unit_truncation(&cell(omitted)));
    }
    let registered = if d.topic == l::UNIT_TOPICS[1] {
        format!("{who}·{book}")
    } else {
        who
    };
    let source = if d.topic == l::UNIT_TOPICS[0] || d.topic == l::UNIT_TOPICS[4] {
        let mut seen = std::collections::HashSet::new();
        coverage
            .sources
            .iter()
            .filter(|s| {
                s.name != "edition_tree"
                    && s.name != "teacher_facts"
                    && s.name != "student_book_bindings"
            })
            .filter(|s| seen.insert((&s.name, &s.read_at)))
            .map(|s| {
                l::unit_source_observation(
                    l::unit_source_name(&s.name),
                    &time(&s.read_at),
                    &safe_source_scope(d, s),
                )
            })
            .collect::<Vec<_>>()
            .join("·")
    } else {
        source_scope(d)
    };
    lines.push(l::unit_footer(
        &time(&d.read_at),
        &source,
        &registered,
        &fact_scope(d),
    ));
    lines.push(l::unit_next_line(&next(d)));
    lines.push(l::CLOSING.into());
    lines.join("\n")
}
pub fn render(data: &mut UnitData, coverage: &mut Coverage) -> String {
    loop {
        let display = compose(data, coverage);
        if display.len() <= 6000 {
            if let Some(omitted) = &data.first_omitted {
                coverage.unknowns.push(format!("first_omitted:{omitted}"));
            }
            return display;
        }
        if let Some(leaf) = data.minors.pop() {
            data.first_omitted = Some(leaf_label(&leaf));
        } else {
            return String::new();
        }
    }
}
fn safe_source_scope(d: &UnitData, s: &crate::envelope::SourceRead) -> String {
    match s.name.as_str() {
        "dailytest" => d
            .daily
            .as_ref()
            .map(|r| {
                l::unit_daily_range(
                    &r.scope.from,
                    &r.scope.to,
                    &cell(&r.scope.selector_label),
                    &cell(&r.scope.page_label),
                )
            })
            .unwrap_or_else(|| l::UNAVAILABLE.into()),
        "dailytest_relative_page" => l::UNIT_RELATIVE_PAGE.into(),
        "dailytest_header" => cell(&d.edition),
        "section_index" => format!(
            "{}·{}",
            cell(&d.edition),
            if d.sections.as_ref().is_some_and(|r| r.paper_observed) {
                l::UNIT_SECTION_SCOPE
            } else {
                l::UNIT_WRONG_NOTE_PAGE
            }
        ),
        "section_grades" => cell(&d.edition),
        "prestudy" => cell(&d.edition),
        _ => cell(&d.edition),
    }
}
fn is_clinic_stage(stage: &str) -> bool {
    [
        "original_clinic_solving",
        "original_clinic_grading",
        "similar_clinic",
        "advanced_clinic",
        "cism_support",
        "printed",
        "handout",
    ]
    .contains(&stage)
}
fn section_columns(d: &UnitData) -> Vec<String> {
    let mut columns = Vec::new();
    for leaf in &d.minors {
        for a in &leaf.minor.activities {
            if !columns.contains(a) {
                columns.push(a.clone());
            }
        }
    }
    columns
}
pub fn lint(display: &str) -> bool {
    !display.is_empty()
        && display.len() <= 6000
        && display.ends_with(l::CLOSING)
        && !display.ends_with('\n')
}
