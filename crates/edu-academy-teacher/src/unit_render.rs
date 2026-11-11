//! Tree-ordered display, qualified evidence and complete-leaf byte bounds.
use crate::{
    display_labels as l,
    envelope::Coverage,
    render::cell,
    unit::{exclusion_applies, exclusion_scope_ambiguous, leaf_label},
    unit_data::*,
};
pub fn verdict(v: UnitVerdict) -> &'static str {
    l::unit_verdict(v)
}
fn time(at: &str) -> String {
    l::observation_time(at)
}
fn same_minute(a: &str, b: &str) -> bool {
    time(a) == time(b)
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
                && j.course_key.as_deref() == Some(&d.course_key)
                && j.book_no.as_deref() == Some(&d.book_no)
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
                .filter(|a| {
                    a.edition_mapping.as_deref() == Some(&d.edition)
                        && a.unit_mapping.as_deref() == Some(code)
                })
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
            daily_status(d, code, None)
        } else {
            attempts
                .iter()
                .map(|a| l::unit_source(a.score))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let js = judgments(d, code, l::UNIT_TOPICS[1]);
        let mut matched = std::collections::HashSet::new();
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
                    && j.activity.is_none()
                    && j.stage.is_none()
                    && j.snapshot.as_ref().map_or(
                        attempts
                            .iter()
                            .filter(|other| other.level == a.level)
                            .count()
                            == 1,
                        |s| s == &a.attempt_key,
                    )
            });
            if let Some(j) = current {
                matched.insert(*j as *const TeacherJudgment);
            }
            details.push(l::unit_judgment_line(
                &current.map(|j| judgment_scope(d, j)).unwrap_or(scope),
                &current
                    .map(|j| cell(&l::unit_judgment(j)))
                    .unwrap_or_else(|| l::UNJUDGED.into()),
            ));
        }
        for j in js
            .iter()
            .filter(|j| !matched.contains(&(**j as *const TeacherJudgment)))
        {
            details.push(l::unit_judgment_line(
                &judgment_scope(d, j),
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
                let mut matched = std::collections::HashSet::new();
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
                        j.activity.is_none()
                            && j.stage.is_none()
                            && j.snapshot
                                .as_ref()
                                .map_or(matching.len() == 1, |s| s == &a.attempt_key)
                    });
                    if let Some(j) = current {
                        matched.insert(*j as *const TeacherJudgment);
                    }
                    lines.push(l::unit_judgment_line(
                        &current
                            .map(|j| judgment_scope(d, j))
                            .unwrap_or(scope.clone()),
                        &current
                            .map(|j| cell(&l::unit_judgment(j)))
                            .unwrap_or_else(|| l::UNJUDGED.into()),
                    ));
                }
                if matching.len() > 1 {
                    lines.push(l::UNIT_SAME_ATTEMPT.into());
                }
                for j in js
                    .iter()
                    .filter(|j| !matched.contains(&(**j as *const TeacherJudgment)))
                {
                    lines.push(l::unit_judgment_line(
                        &judgment_scope(d, j),
                        &cell(&l::unit_judgment(j)),
                    ));
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
                        &daily_status(d, code, level.as_deref()),
                    ));
                }
                if js.is_empty() {
                    lines.push(l::unit_judgment_line(&scope, l::UNJUDGED));
                } else {
                    for j in js {
                        lines.push(l::unit_judgment_line(
                            &judgment_scope(d, j),
                            &cell(&l::unit_judgment(j)),
                        ));
                    }
                }
            }
        }
        (lines.join("\n"), details)
    }
}
fn clinic_block(d: &UnitData, leaf: &Leaf, printed_dash: &mut bool) -> (String, Vec<String>) {
    let code = &leaf.minor.code;
    let mut fields = vec![cell(&leaf_label(leaf))];
    let mut details = Vec::new();
    for a in section_columns(d) {
        let c = d.clinic.iter().find(|c| c.unit == *code && c.activity == a);
        let value: String = if !leaf.minor.activities.contains(&a) {
            l::NOT_APPLICABLE.into()
        } else if d.sections.is_none() {
            l::UNIT_SECTION_UNREAD.into()
        } else {
            c.map(|c| {
                if c.verdict == UnitVerdict::Unknown {
                    section_issue(d, code, Some(&a)).unwrap_or(l::UNIT_DASH)
                } else {
                    verdict(c.verdict)
                }
            })
            .unwrap_or(l::UNIT_DASH)
            .into()
        };
        *printed_dash |= value == l::UNIT_DASH;
        fields.push(value);
        if let Some(c) = c
            && c.verdict == UnitVerdict::Eligible
        {
            details.push(l::unit_clinic_basis(
                &cell(code),
                &cell(&a),
                &c.labels
                    .iter()
                    .map(|v| cell(v))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }
    for e in d.excluded.iter().filter(|e| {
        exclusion_applies(d, e, code, &e.activity)
            || exclusion_scope_ambiguous(d, e, code, &e.activity)
    }) {
        if exclusion_scope_ambiguous(d, e, code, &e.activity) {
            details.push(format!(
                "{} {}: {}",
                cell(code),
                cell(&e.activity),
                l::UNIT_EXCLUSION_SCOPE_UNKNOWN
            ));
        } else {
            details.push(l::unit_clinic_excluded(
                &exclusion_scope(d, e),
                &cell(&e.activity),
            ));
        }
    }
    for j in judgments(d, code, l::UNIT_TOPICS[2]) {
        details.push(l::unit_clinic_judgment(
            &judgment_scope(d, j),
            &cell(&l::unit_judgment(j)),
        ));
    }
    (fields.join(" | "), details)
}
fn section_block(d: &UnitData, leaf: &Leaf, grid: bool, printed_dash: &mut bool) -> String {
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
            let value: String = if !leaf.minor.activities.contains(&activity) {
                l::NOT_APPLICABLE.into()
            } else if titles.is_empty() {
                if let Some(issue) = section_issue(d, code, Some(&activity)) {
                    issue.into()
                } else {
                    *printed_dash = true;
                    l::UNIT_DASH.into()
                }
            } else {
                titles.join(" / ")
            };
            fields.push(value);
        }
        return fields.join(" | ");
    }
    if records.is_empty() {
        return l::unit_absent(
            &cell(code),
            if let Some(issue) = section_issue(d, code, d.activity.as_deref()) {
                issue
            } else if d
                .sections
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
            } else if d.sections.as_ref().is_some_and(|s| s.index_observed) {
                l::UNIT_PAPER_SCOPE_UNKNOWN
            } else {
                l::UNIT_SOURCE_NOT_OBSERVED
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
    let mut facts = vec![if !same_minute(&d.tree_read_at, &d.read_at) {
        l::unit_cached_tree(&time(&d.tree_read_at))
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
                let mut description = l::unit_fact_judgment(&judgment_scope(d, j), None, j);
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
            if d.minors.iter().any(|l| l.minor.code == e.unit)
                && exclusion_applies(d, e, &e.unit, &e.activity)
            {
                facts.push(l::unit_exclusion(
                    &exclusion_scope(d, e),
                    &cell(&e.activity),
                ));
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
        return daily_source_scope(d);
    }

    if d.topic == l::UNIT_TOPICS[2] {
        if d.sections.is_none() {
            return l::UNIT_SECTION_UNREAD.into();
        }
        return l::unit_section_source(
            &who,
            &book,
            section_inventory_scope(d),
            d.sections.as_ref().is_none_or(|r| {
                r.records.is_empty()
                    && !r
                        .observations
                        .iter()
                        .any(|o| o.state == SectionState::Absent)
            }),
        );
    }
    if d.topic == l::UNIT_TOPICS[3] {
        if d.sections.is_none() {
            return l::UNIT_SECTION_UNREAD.into();
        }
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
            section_inventory_scope(d).into()
        };
        return l::unit_section_source(
            &who,
            &book,
            &scope,
            absent
                || d.sections.as_ref().is_none_or(|r| {
                    r.records.is_empty()
                        && !r
                            .observations
                            .iter()
                            .any(|o| o.state == SectionState::Absent)
                }),
        );
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
fn compose(d: &UnitData, coverage: &Coverage, tree_scope: &[Leaf]) -> String {
    let who = cell(&d.student);
    let book = cell(&d.edition);
    let small = d.minors.len() == 1;
    let mut lines = Vec::new();
    let mut details = Vec::new();
    let mut printed_dash = false;
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
            let mut headers = vec![l::UNIT_MINOR.to_string()];
            headers.extend(section_columns(d).iter().map(|a| cell(a)));
            lines.push(headers.join(" | "));
            for leaf in &d.minors {
                let (block, extra) = clinic_block(d, leaf, &mut printed_dash);
                lines.push(block);
                details.extend(extra);
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
                headers.extend(section_columns(d).iter().map(|a| cell(a)));
                lines.push(headers.join(" | "));
            }
            for leaf in &d.minors {
                lines.push(section_block(d, leaf, !small, &mut printed_dash));
            }
        }
        _ => {
            lines.push(format!("{who} {book}"));
            for leaf in &d.minors {
                lines.push(cell(&leaf_label(leaf)));
                if d.daily.is_some() || d.topic == l::UNIT_TOPICS[0] {
                    let (block, extra) = daily_block(d, leaf, false);
                    lines.push(block);
                    lines.extend(extra);
                }
                if d.sections.is_some() || d.topic == l::UNIT_TOPICS[0] {
                    lines.push(section_block(d, leaf, false, &mut printed_dash));
                }
            }
            if d.topic == l::UNIT_TOPICS[0] {
                lines.push(section_header(d));
                for leaf in &d.minors {
                    let (block, extra) = clinic_block(d, leaf, &mut printed_dash);
                    lines.push(block);
                    lines.extend(extra);
                }
            }
            for stage in d.prestudy.iter().filter(|s| retained(d, &s.unit)) {
                lines.push(format!(
                    "{} {}: {}",
                    cell(&stage.unit),
                    cell(&stage.stage),
                    if stage.verdict != UnitVerdict::Unknown {
                        verdict(stage.verdict)
                    } else if coverage.unknowns.iter().any(|u| u == "prestudy_unavailable") {
                        l::UNIT_PRESTUDY_UNREAD
                    } else if coverage.unknowns.iter().any(|u| u == "prestudy_name_ambiguous") {
                        l::UNIT_PRESTUDY_NAME_AMBIGUOUS
                    } else {
                        l::UNIT_PRESTUDY_OTHER_BOOK
                    }
                ));
            }
        }
    }
    if printed_dash {
        lines.push(l::UNIT_DASH_LEGEND.into());
    }
    lines.extend(details);
    lines.extend(unscoped_fact_lines(d));
    lines.extend(ambiguous_attempt_lines(d));
    lines.extend(unmapped_lines(d));
    if d.topic == l::UNIT_TOPICS[2] || d.topic == l::UNIT_TOPICS[0] {
        lines.extend(stage_lines(d));
    }
    for stage in d
        .stages
        .iter()
        .filter(|s| retained(d, &s.unit) && !is_clinic_stage(&s.stage))
    {
        lines.push(format!(
            "{} {}: {}",
            cell(&stage.unit),
            cell(l::unit_stage_name(&stage.stage)),
            verdict(stage.verdict)
        ));
    }
    lines.extend(unknown_lines(d, coverage, tree_scope));
    if let Some(omitted) = &d.first_omitted {
        lines.push(l::unit_truncation(&cell(omitted)));
    }
    let mut registered = if d.topic == l::UNIT_TOPICS[1] {
        format!("{who}·{book}")
    } else {
        who
    };
    if let Some(source) = coverage
        .sources
        .iter()
        .find(|s| s.name == "student_book_bindings" && !same_minute(&s.read_at, &d.read_at))
    {
        registered = l::unit_timed_scope(&registered, &time(&source.read_at));
    }
    let mut facts = fact_scope(d);
    if let Some(source) = coverage
        .sources
        .iter()
        .find(|s| s.name == "teacher_facts" && !same_minute(&s.read_at, &d.read_at))
    {
        facts = l::unit_timed_scope(&facts, &time(&source.read_at));
    }
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
        &facts,
    ));
    lines.push(l::unit_next_line(&next(d)));
    lines.push(l::CLOSING.into());
    lines.join("\n")
}
pub fn render(data: &mut UnitData, coverage: &mut Coverage) -> String {
    let tree_scope = data.minors.clone();
    loop {
        let display = compose(data, coverage, &tree_scope);
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
        "dailytest" => daily_source_scope(d),
        "dailytest_relative_page" => l::UNIT_RELATIVE_PAGE.into(),
        "dailytest_header" => cell(&d.edition),
        "section_index" => format!(
            "{}·{}",
            cell(&d.edition),
            if d.sections.as_ref().is_some_and(|r| r.paper_observed) {
                l::UNIT_INDEX_SCOPE
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
        for a in leaf
            .minor
            .activities
            .iter()
            .filter(|a| d.activity.as_ref().is_none_or(|filter| filter == *a))
        {
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

fn retained(d: &UnitData, unit: &str) -> bool {
    d.minors.iter().any(|l| l.minor.code == unit)
}
fn section_issue(d: &UnitData, unit: &str, activity: Option<&str>) -> Option<&'static str> {
    if d.sections.is_none() {
        return Some(l::UNIT_SECTION_UNREAD);
    }
    if (d.topic == l::UNIT_TOPICS[0] || d.topic == l::UNIT_TOPICS[2])
        && d.excluded.iter().any(|e| {
            activity.is_none_or(|a| e.activity == a)
                && exclusion_scope_ambiguous(d, e, unit, &e.activity)
        })
    {
        return Some(l::UNIT_EXCLUSION_SCOPE_UNKNOWN);
    }
    let observations = &d.sections.as_ref()?.observations;
    if observations.iter().any(|o| {
        o.unit.as_deref() == Some(unit)
            && activity.is_none_or(|a| o.activity.as_deref() == Some(a))
            && o.state == SectionState::Unread
    }) {
        Some(l::UNIT_GRADE_UNREAD)
    } else if observations.iter().any(|o| {
        o.unit.as_deref() == Some(unit)
            && activity.is_none_or(|a| o.activity.as_deref() == Some(a))
            && o.state == SectionState::Ambiguous
    }) {
        Some(l::UNIT_ATTEMPTS_AMBIGUOUS)
    } else if observations.iter().any(|o| {
        o.unit.as_deref() == Some(unit)
            && activity.is_none_or(|a| o.activity.as_deref() == Some(a))
            && o.state == SectionState::Absent
    }) {
        Some(l::NOT_IN_EITHER)
    } else {
        None
    }
}
fn ambiguous_attempt_lines(d: &UnitData) -> Vec<String> {
    d.sections
        .as_ref()
        .map(|s| {
            s.observations
                .iter()
                .filter(|o| {
                    o.state == SectionState::Ambiguous
                        && o.unit.as_deref().is_some_and(|u| retained(d, u))
                })
                .map(|o| {
                    format!(
                        "{} {} {}: {}·{}",
                        cell(o.unit.as_deref().unwrap_or("")),
                        cell(o.activity.as_deref().unwrap_or("")),
                        snapshot_scope(d, &o.attempt_key, false),
                        l::UNIT_ATTEMPTS_AMBIGUOUS,
                        l::unit_unknown_remedy("section_ambiguous_attempts")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn snapshot_scope(d: &UnitData, snapshot: &str, daily_topic: bool) -> String {
    let section = d.sections.as_ref().and_then(|s| {
        s.records
            .iter()
            .enumerate()
            .find(|(_, r)| r.attempt_key == snapshot)
            .map(|(i, r)| (i, r.source_date.as_deref()))
    });
    let daily = d.daily.as_ref().and_then(|r| {
        r.attempts
            .iter()
            .enumerate()
            .find(|(_, a)| a.attempt_key == snapshot)
            .map(|(i, a)| (i, a.occurred_at.as_deref()))
    });
    let section = section.or_else(|| {
        d.sections.as_ref().and_then(|s| {
            s.observations
                .iter()
                .enumerate()
                .find(|(_, o)| o.attempt_key == snapshot)
                .map(|(i, _)| (i, None))
        })
    });
    l::unit_snapshot_scope(
        if daily_topic { daily } else { section }.map(|(i, date)| (i + 1, date.map(cell))),
    )
}
fn judgment_scope(d: &UnitData, j: &TeacherJudgment) -> String {
    let mut scope = scoped(&j.unit, j.level.as_deref());
    if let Some(a) = &j.activity {
        scope.push_str(&format!(" {}", cell(a)));
    }
    if let Some(stage) = j.stage.as_ref().filter(|stage| !stage.is_empty()) {
        scope.push_str(&format!(" {}", cell(l::unit_stage_name(stage))));
    }
    if let Some(snapshot) = &j.snapshot {
        scope.push_str(&format!(
            " {}",
            snapshot_scope(d, snapshot, j.topic == l::UNIT_TOPICS[1])
        ));
    }
    scope
}
fn exclusion_scope(d: &UnitData, e: &ExcludedSection) -> String {
    let mut scope = cell(&e.unit);
    if let Some(snapshot) = &e.snapshot {
        scope.push_str(&format!(" {}", snapshot_scope(d, snapshot, false)));
    }
    scope
}
fn stage_lines(d: &UnitData) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, name) in [
        "original_clinic_solving",
        "original_clinic_grading",
        "similar_clinic",
        "advanced_clinic",
        "cism_support",
        "printed",
        "handout",
    ]
    .iter()
    .enumerate()
    {
        let stages: Vec<_> = d
            .stages
            .iter()
            .filter(|s| retained(d, &s.unit) && s.stage == *name)
            .collect();
        let default = if index < 2 {
            UnitVerdict::TeacherReportedNoServer
        } else {
            UnitVerdict::Unknown
        };
        if index < 3 && !stages.is_empty() && stages.iter().all(|s| s.verdict == default) {
            lines.push(format!(
                "{}: {}",
                l::unit_stage_name(name),
                if index == 2 {
                    l::UNIT_SOURCE_NOT_OBSERVED
                } else {
                    verdict(default)
                }
            ));
        } else {
            if stages.iter().any(|s| s.verdict == default)
                && stages.iter().any(|s| s.verdict != default)
            {
                lines.push(l::unit_stage_remainder(
                    l::unit_stage_name(name),
                    if default == UnitVerdict::Unknown {
                        l::UNIT_SOURCE_NOT_OBSERVED
                    } else {
                        verdict(default)
                    },
                ));
            }
            let mut grouped = std::collections::HashSet::new();
            let mut by_unit: std::collections::HashMap<&str, Vec<_>> =
                std::collections::HashMap::new();
            for s in &stages {
                by_unit.entry(&s.unit).or_default().push(*s);
            }
            for s in stages.iter().filter(|s| s.verdict != default) {
                let unit_stages = &by_unit[s.unit.as_str()];
                let complete_unit = d.activity.is_none()
                    && d.minors
                        .iter()
                        .find(|l| l.minor.code == s.unit)
                        .is_some_and(|leaf| {
                            unit_stages.len() == leaf.minor.activities.len()
                                && leaf.minor.activities.iter().all(|a| {
                                    unit_stages
                                        .iter()
                                        .any(|other| other.activity.as_ref() == Some(a))
                                })
                        })
                    && unit_stages
                        .iter()
                        .all(|other| other.snapshot.is_none() && other.verdict == s.verdict);
                if complete_unit && !grouped.insert(&s.unit) {
                    continue;
                }
                if s.verdict != default || index < 3 {
                    lines.push(format!(
                        "{} {}: {}",
                        if complete_unit {
                            cell(&s.unit)
                        } else {
                            stage_scope(d, s)
                        },
                        l::unit_stage_name(name),
                        if s.verdict == UnitVerdict::Unknown {
                            l::UNIT_SOURCE_NOT_OBSERVED
                        } else {
                            verdict(s.verdict)
                        }
                    ));
                }
            }
        }
    }
    lines
}

fn stage_scope(d: &UnitData, s: &crate::unit::Stage) -> String {
    let mut scope = cell(&s.unit);
    if let Some(a) = &s.activity {
        scope.push_str(&format!(" {}", cell(a)));
    }
    if let Some(snapshot) = &s.snapshot {
        scope.push_str(&format!(" {}", snapshot_scope(d, snapshot, false)));
    }
    scope
}

fn section_inventory_scope(d: &UnitData) -> &'static str {
    match &d.sections {
        Some(read)
            if !read.observations.is_empty()
                && read
                    .observations
                    .iter()
                    .all(|o| o.state == SectionState::Absent) =>
        {
            l::UNIT_GRADES_ABSENT_SCOPE
        }
        Some(read) if !read.records.is_empty() && read.paper_observed => l::UNIT_SECTION_SCOPE,
        Some(read) if read.paper_observed => l::UNIT_TITLES_OBSERVED,
        Some(read) if read.index_observed => l::UNIT_WRONG_NOTE_TITLES,
        _ => l::UNIT_SOURCE_NOT_OBSERVED,
    }
}

fn daily_source_scope(d: &UnitData) -> String {
    let Some(read) = &d.daily else {
        return l::UNAVAILABLE.into();
    };
    let primary = read.sources.iter().any(|s| s.name == "dailytest");
    let mut scopes = vec![if primary {
        l::unit_daily_range(
            &read.scope.from,
            &read.scope.to,
            &cell(&read.scope.selector_label),
            &cell(&read.scope.page_label),
        )
    } else {
        l::UNIT_RELATIVE_PAGE.into()
    }];
    let mut seen = std::collections::HashSet::new();
    for source in read
        .sources
        .iter()
        .filter(|s| !same_minute(&s.read_at, &d.read_at) && seen.insert((&s.name, &s.read_at)))
    {
        let scope = if source.name == "dailytest_relative_page" {
            l::UNIT_RELATIVE_PAGE.to_string()
        } else {
            cell(&d.edition)
        };
        scopes.push(l::unit_source_observation(
            l::unit_source_name(&source.name),
            &time(&source.read_at),
            &scope,
        ));
    }
    scopes.join("·")
}
fn unmapped_lines(d: &UnitData) -> Vec<String> {
    let Some(read) = &d.sections else {
        return Vec::new();
    };
    let mut counts = std::collections::BTreeMap::new();
    for o in read
        .observations
        .iter()
        .filter(|o| o.state == SectionState::Unmapped)
    {
        *counts.entry(&o.source_title).or_insert(0usize) += 1;
    }
    let total = counts.values().sum();
    let titles: Vec<_> = counts
        .into_iter()
        .map(|(title, count)| l::unit_counted_unknown(&l::unit_unmapped_title(&cell(title)), count))
        .collect();
    if titles.iter().map(|t| t.len() + 1).sum::<usize>() > 1500 {
        vec![l::unit_unmapped_summary(total)]
    } else {
        titles
    }
}

fn daily_status(d: &UnitData, unit: &str, level: Option<&str>) -> String {
    if d.daily.is_none() && d.family != "supplementary" {
        l::UNIT_DAILY_UNREAD.into()
    } else if daily_state(d, unit, level) == UnitVerdict::Unknown {
        let reasons = &d.daily.as_ref().expect("daily read exists").unknowns;
        if reasons.iter().any(|u| u == "daily_header_unavailable") {
            l::UNIT_DAILY_HEADER_UNREAD
        } else if reasons.iter().any(|u| u == "daily_partial_page") {
            l::UNIT_DAILY_PARTIAL_UNREAD
        } else if reasons.iter().any(|u| u == "daily_mapping_unavailable") {
            l::UNIT_DAILY_MAPPING_UNKNOWN
        } else {
            l::UNIT_SOURCE_NOT_OBSERVED
        }.into()
    } else {
        verdict(daily_state(d, unit, level)).into()
    }
}
fn section_header(d: &UnitData) -> String {
    std::iter::once(l::UNIT_MINOR.to_string())
        .chain(section_columns(d).iter().map(|a| cell(a)))
        .collect::<Vec<_>>()
        .join(" | ")
}
fn unscoped_fact_lines(d: &UnitData) -> Vec<String> {
    let mut lines = Vec::new();
    for j in d.judgments.iter().filter(|j| {
        (j.course_key.is_none() || j.book_no.is_none())
            && retained(d, &j.unit)
            && (d.topic == l::UNIT_TOPICS[0] || d.topic == j.topic)
    }) {
        lines.push(l::unit_unscoped_judgment(
            &judgment_scope(d, j),
            &cell(&l::unit_judgment(j)),
        ));
    }
    for e in d.excluded.iter().filter(|e| {
        (e.course_key.is_none() || e.book_no.is_none())
            && retained(d, &e.unit)
            && (d.topic == l::UNIT_TOPICS[0] || d.topic == l::UNIT_TOPICS[2])
    }) {
        lines.push(l::unit_unscoped_exclusion(
            &cell(&e.unit),
            &cell(&e.activity),
        ));
    }
    let mut counts = std::collections::BTreeMap::new();
    for line in lines {
        *counts.entry(line).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(line, count)| l::unit_counted_unknown(&line, count))
        .collect()
}
fn unknown_lines(d: &UnitData, coverage: &Coverage, tree_scope: &[Leaf]) -> Vec<String> {
    let mut counts = std::collections::BTreeMap::new();
    for reason in &coverage.unknowns {
        let (code, scope) = reason.split_once(':').unwrap_or((reason.as_str(), ""));
        if matches!(
            code,
            "first_omitted" | "section_ambiguous_attempts" | "section_mapping_unavailable"
        ) {
            continue;
        }
        // Match the longest tree identifier, including identifiers containing
        // spaces. Keep a parent scope while any of its descendants is retained.
        let mut matches = Vec::new();
        for leaf in tree_scope {
            let major = format!("{} {}", l::CHAPTER_SCOPE, leaf.chapter);
            for (id, level) in [
                (Some(leaf.chapter.as_str()), 0),
                (Some(major.as_str()), 0),
                (leaf.middle.as_deref(), 1),
                (leaf.middle_code.as_deref(), 1),
                (Some(leaf.minor.code.as_str()), 2),
            ] {
                if let Some(id) = id.filter(|id| !id.is_empty())
                    && (scope == id
                        || scope
                            .strip_prefix(id)
                            .is_some_and(|tail| tail.starts_with(char::is_whitespace)))
                {
                    matches.push(((id.len(), level), retained(d, &leaf.minor.code)));
                }
            }
        }
        if let Some(specificity) = matches.iter().map(|(key, _)| *key).max()
            && !matches
                .iter()
                .any(|(key, kept)| *key == specificity && *kept)
        {
            continue;
        }
        *counts.entry(reason).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(reason, count)| {
            let (code, scope) = reason.split_once(':').unwrap_or((reason.as_str(), ""));
            let count = if count > 1 {
                format!("·{count}건")
            } else {
                String::new()
            };
            l::unit_unknown_line(&format!(
                "{}{}{}·{}",
                if scope.is_empty() {
                    String::new()
                } else {
                    format!("{}·", cell(scope))
                },
                l::unit_unknown_reason(code),
                count,
                l::unit_unknown_remedy(code)
            ))
        })
        .collect()
}
