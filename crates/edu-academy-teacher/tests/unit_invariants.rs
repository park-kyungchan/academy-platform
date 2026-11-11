//! Independent display properties. All 200 seeds run even when an earlier case
//! fails. Diagnostics retain the first (smallest seed) witness per failure kind.
#[path = "support/unit_gen.rs"]
mod unit_gen;

use edu_academy_teacher::{
    envelope::{AskField, Envelope, StopCode},
    synthetic::PrestudyRow,
    unit_data::{ExcludedSection, UnitVerdict},
};
use std::{collections::BTreeMap, time::Instant};
use unit_gen::*;

type Failures = Vec<(&'static str, String)>;
fn require(
    errors: &mut Failures,
    condition: bool,
    tag: &'static str,
    description: impl Into<String>,
) {
    if !condition {
        errors.push((tag, description.into()));
    }
}
fn answer(errors: &mut Failures, out: &Envelope, tag: &'static str) -> bool {
    let ok = out.stop.is_none() && out.ask.is_none() && !out.display.is_empty();
    require(
        errors,
        ok,
        tag,
        format!("expected answer; stop={:?} ask={:?}", out.stop, out.ask),
    );
    ok
}
fn sweep(name: &str, mut check: impl FnMut(&mut Case) -> Failures) {
    let started = Instant::now();
    let mut first = BTreeMap::new();
    let mut failed_cases = 0;
    for seed in 1..=CASES {
        let mut case = Case::new(seed);
        let failures = check(&mut case);
        failed_cases += usize::from(!failures.is_empty());
        for (tag, why) in failures {
            first
                .entry(tag)
                .or_insert_with(|| format!("{}; {why}", case.describe()));
        }
    }
    let witnesses = first
        .iter()
        .map(|(tag, why)| format!("{tag}: {why}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        first.is_empty(),
        "{name}: cases={CASES} failed_cases={failed_cases} elapsed={:?}\n{witnesses}",
        started.elapsed()
    );
}
fn row<'a>(out: &'a Envelope, code: &str) -> Option<&'a str> {
    out.display
        .lines()
        .find(|line| line.starts_with(&format!("{code} ")) && line.contains(" | "))
}
fn score_count(out: &Envelope, score: f64) -> usize {
    // A decimal marker avoids accidentally matching 4 in 41 or a native key.
    out.display.matches(&format!("원천 점수: {score}")).count()
}
fn source_clock(display: &str, label: &str, at: &str) -> bool {
    // Look in this source's clause; an unrelated source's time is not enough.
    // Both the original timestamp and its displayed HH:MM are accepted.
    display.match_indices(label).any(|(start, _)| {
        let tail = &display[start..];
        let end = tail.find([',', '\n']).unwrap_or(tail.len());
        tail[..end].contains(at) || tail[..end].contains(&at[11..16])
    })
}

#[test]
fn i1_every_observation_keeps_its_scope_and_multiplicity() {
    sweep("I1", |c| {
        let mut errors = vec![];
        match c.seed % 6 {
            0 => {
                c.scope(1, "섹션");
                let mut attempts = vec![];
                for i in 0..(2 + c.seed as usize % 3) {
                    let mut a = section(0, 0, "1-1", "필수예제", i);
                    a.source_date = (i % 2 == 0).then(|| format!("2026-11-{:02}", 7 + i));
                    attempts.push(a);
                }
                c.own_sections(attempts.clone());
                let out = c.run();
                if answer(&mut errors, &out, "B2") {
                    // A source seam may refuse ambiguous retests. Permit
                    // separate, scoped unknowns instead of fabricated scores;
                    // one generic error line cannot stand in for every item.
                    let unknown_lines = out
                        .display
                        .lines()
                        .filter(|l| {
                            l.contains("1-1")
                                && l.contains("필수예제")
                                && l.contains("미확인")
                                && !l.contains("읽은 범위에 없음")
                        })
                        .collect::<std::collections::BTreeSet<_>>();
                    let individually_unknown = unknown_lines.len() == attempts.len();
                    for a in attempts {
                        require(
                            &mut errors,
                            score_count(&out, a.source_score) == 1 || individually_unknown,
                            "B2",
                            format!(
                                "same-title attempt score {} lost/repeated (dated={})",
                                a.source_score,
                                a.source_date.is_some()
                            ),
                        );
                    }
                }
            }
            1 => {
                c.scope(2, "DailyTest");
                let mut a = c.scenario.dailytest_frames[0].attempts[0].clone();
                a.level = Some("기본".into());
                c.own_daily(vec![a]);
                c.judgment("DailyTest", None, Some("발전"), None);
                let out = c.run();
                answer(&mut errors, &out, "B5");
                require(
                    &mut errors,
                    out.display
                        .lines()
                        .filter(|l| l.starts_with("1-1 발전 교사 판정: 완료 판정"))
                        .count()
                        == 1,
                    "B5",
                    "unmatched advanced judgment must have exactly one detail line",
                );
                // DailyTest retests use the same title and paper, distinct
                // attempt keys, and mixed known/unknown occurrence dates.
                let mut retest_case = Case::new(c.seed);
                retest_case.scope(1, "DailyTest");
                let template = c.scenario.dailytest_frames[0].attempts[0].clone();
                let attempts: Vec<_> = (0..2 + c.seed as usize % 3)
                    .map(|i| {
                        let mut a = template.clone();
                        a.attempt_key = format!("{}", 93000 + i);
                        a.score = Some(63.125 + i as f64);
                        a.occurred_at = (i % 2 == 1).then(|| format!("2026-11-{:02}", 7 + i));
                        a
                    })
                    .collect();
                retest_case.own_daily(attempts.clone());
                let retests = retest_case.run();
                answer(&mut errors, &retests, "daily_retests");
                for a in attempts {
                    require(
                        &mut errors,
                        retests
                            .display
                            .matches(&format!("원천 점수 {}", a.score.unwrap()))
                            .count()
                            == 1,
                        "daily_retests",
                        "DailyTest same-title attempt lost/repeated",
                    );
                }
            }
            2 => {
                c.scope(1, "클리닉");
                let a = section(0, 0, "1-1", "대단원총괄", 0);
                let label = a.items[0].label.clone().unwrap();
                c.own_sections(vec![a]);
                let out = c.run();
                answer(&mut errors, &out, "B6");
                require(
                    &mut errors,
                    out.display
                        .lines()
                        .any(|l| l.contains("대단원총괄 대상 근거:") && l.contains(&label)),
                    "B6",
                    "edition-defined comprehensive activity and wrong-item basis were dropped",
                );
                c.scenario.excluded_sections.push(ExcludedSection {
                    course_key: None,
                    book_no: None,
                    snapshot: None,
                    student_key: "1001".into(),
                    edition: c.request.book.clone().unwrap(),
                    unit: "1-1".into(),
                    activity: "대단원총괄".into(),
                    state: "active".into(),
                    actor: None,
                    recorded_at: (c.seed % 4 == 0).then(|| "2026-11-10".into()),
                });
                let excluded = c.run();
                require(
                    &mut errors,
                    excluded
                        .display
                        .lines()
                        .filter(|l| {
                            l.starts_with("1-1 대단원총괄:") && l.contains("교사가 제외한 섹션")
                        })
                        .count()
                        == 1,
                    "B6_exclusion",
                    "comprehensive activity exclusion lacks its own detail",
                );
            }
            3 => {
                c.scope(2, "섹션");
                let mut a = section(0, 0, "1-2", "필수예제", 0);
                a.title.push_str(" (선수 단원 1-1 확인)");
                c.own_sections(vec![a]);
                let out = c.run();
                answer(&mut errors, &out, "B12");
                require(
                    &mut errors,
                    row(&out, "1-2").is_some_and(|l| l.contains("선수 단원"))
                        && !row(&out, "1-1").is_some_and(|l| l.contains("선수 단원")),
                    "B12",
                    "1-2 observation placed under prerequisite token 1-1",
                );
            }
            4 => {
                c.scope(2, "섹션");
                let a = section(0, 0, "9-9", "추가활동", 0);
                let title = a.title.clone();
                c.own_sections(vec![a]);
                let out = c.run();
                answer(&mut errors, &out, "B12_unmapped");
                require(
                    &mut errors,
                    out.display.contains(&title) && out.display.contains("미확인"),
                    "B12_unmapped",
                    "observed off-tree activity needs a source-qualified unknown retaining its title",
                );
            }
            _ => {
                c.scope(1, "클리닉");
                c.judgment("클리닉", Some("필수예제"), None, Some("82000"));
                c.judgment("클리닉", Some("유형다지기"), None, None);
                c.scenario.teacher_judgments[0].stage = Some("original_clinic_solving".into());
                c.scenario.teacher_judgments[1].stage = Some("original_clinic_grading".into());
                let out = c.run();
                answer(&mut errors, &out, "B13");
                for (activity, stage) in [
                    ("필수예제", "오답클리닉 풀이"),
                    ("유형다지기", "오답클리닉 채점"),
                ] {
                    require(
                        &mut errors,
                        out.display
                            .lines()
                            .filter(|l| {
                                l.contains("교사 판정") && l.contains(activity) && l.contains(stage)
                            })
                            .count()
                            == 1,
                        "B13",
                        format!("judgment lost activity/stage association: {activity}/{stage}"),
                    );
                }
            }
        }
        errors
    });
}

#[test]
fn i2_absence_and_read_claims_require_the_exact_successful_source() {
    sweep("I2", |c| {
        let mut errors = vec![];
        c.scope(1, "섹션");
        let first = section(0, 0, "1-1", "필수예제", 0);
        let second = section(0, 0, "1-1", "유형다지기", 1);
        c.own_sections(vec![first.clone(), second.clone()]);
        match c.seed % 6 {
            0 | 1 => {
                c.faults.detail_fail.push(first.attempt_key.clone());
                if c.seed % 6 == 0 {
                    c.faults.detail_fail.push(second.attempt_key.clone());
                }
            }
            2 => c.faults.notes_fail = true,
            3 => c.faults.timeout_once = true,
            4 => c.own_sections(vec![]),
            _ => {
                c.request.topic = Some("DailyTest".into());
                c.faults.daily_fail = Some("1".into());
                c.faults.relative_rows = true;
            }
        }
        let mut session = c.session();
        let out = session.call(&c.request);
        let audit = session.audit.borrow().clone();
        if c.seed % 6 == 5
            && !audit
                .iter()
                .any(|e| e.source == "daily_page" && e.scope == "11" && e.succeeded)
        {
            // History returns immediately on primary-page failure, before
            // asking for the relative page. Its actual result must be unread.
            require(
                &mut errors,
                out.stop
                    .as_ref()
                    .is_some_and(|s| s.code == StopCode::SourceUnavailable)
                    && !out.display.contains("두 원천 모두에 없음"),
                "source_failure",
                "unread DailyTest must stop without absence claims",
            );
            // Independently exercise the valid partial-read renderer input.
            // This tests source truth even before a status injection seam exists.
            let partial = c.render_relative_only();
            require(
                &mut errors,
                !partial.contains("현재 필터 첫 관측 페이지") && partial.contains("상대기간"),
                "B11_scope",
                "relative-only renderer input falsely claims the primary filtered page was read",
            );
            let expected = c.scenario.dailytest_frames[0].attempts[0].score.unwrap();
            require(
                &mut errors,
                partial.matches(&format!("원천 점수 {expected}")).count() == 1,
                "relative_neighbour",
                "successful relative-only score lost or repeated",
            );
        } else if c.seed % 6 == 2 {
            require(
                &mut errors,
                out.stop.is_some() || !out.display.contains("두 원천 모두에 없음"),
                "source_failure",
                "failed index cannot establish absence",
            );
        } else if answer(&mut errors, &out, "I2_answer") {
            if c.seed % 6 <= 1 {
                require(
                    &mut errors,
                    !out.display.contains("1-1 섹션: 미확인(읽은 범위에 없음)"),
                    "B1",
                    "observed title with unread grade called not observed",
                );
                require(
                    &mut errors,
                    out.display.contains("성적을 읽지 못함") && out.display.contains("1-1"),
                    "B1",
                    "failed detail lacks scoped unread reason",
                );
                require(
                    &mut errors,
                    score_count(&out, first.source_score) == 0,
                    "B1",
                    "failed grade still printed",
                );
                if c.seed % 6 == 1 {
                    require(
                        &mut errors,
                        score_count(&out, second.source_score) == 1,
                        "neighbour",
                        "successful neighbouring grade lost",
                    );
                }
                // The same unread item must stay unread in the clinic grid.
                let mut request = c.request.clone();
                request.topic = Some("클리닉".into());
                let clinic = session.call(&request);
                require(
                    &mut errors,
                    row(&clinic, "1-1").is_some_and(|line| {
                        line.split(" | ")
                            .nth(1)
                            .is_some_and(|cell| cell.contains("읽지 못"))
                    }),
                    "B1_clinic",
                    "failed grade represented by absence dash in clinic cell",
                );
            }
            if c.seed % 6 == 3 {
                require(
                    &mut errors,
                    score_count(&out, first.source_score) == 1
                        && score_count(&out, second.source_score) == 1,
                    "retry",
                    "one timeout must preserve both observed grades without duplication",
                );
                require(
                    &mut errors,
                    audit.iter().any(|e| e.source == "notes" && !e.succeeded)
                        && audit.iter().any(|e| e.source == "notes" && e.succeeded),
                    "retry",
                    "timeout fault was not followed by a successful read",
                );
            }
            if c.seed % 6 == 5 {
                require(
                    &mut errors,
                    !out.display.contains("현재 필터 첫 관측 페이지"),
                    "B11_scope",
                    "failed primary page described as successfully read",
                );
                require(
                    &mut errors,
                    out.display.contains("상대기간"),
                    "B11_scope",
                    "successful relative page scope absent",
                );
            }
            for e in audit
                .iter()
                .filter(|e| e.source == "detail" && !e.succeeded)
            {
                require(
                    &mut errors,
                    !out.coverage
                        .sources
                        .iter()
                        .any(|s| s.name == "section_grades" && s.scope == e.scope),
                    "source_failure",
                    "failed attempt represented as successful grade coverage",
                );
            }
        }
        errors
    });
}

#[test]
fn i3_student_edition_and_attempt_identity_never_mix() {
    sweep("I3", |c| {
        let mut errors = vec![];
        c.scope(2, "DailyTest");
        if c.seed % 2 == 0 {
            let mut a = c.scenario.dailytest_frames[0].attempts[0].clone();
            let score = a.score.unwrap();
            a.edition_mapping = None;
            c.own_daily(vec![a]);
            let out = c.run();
            answer(&mut errors, &out, "B3");
            require(
                &mut errors,
                !row(&out, "1-1").is_some_and(|l| l.contains(&format!("원천 점수 {score}"))),
                "B3",
                "unresolved edition score assigned to requested edition/unit",
            );
        } else {
            let mut other = c.scenario.students[0].clone();
            other.key = "1002".into();
            other.class_name = "가상반B".into();
            c.scenario.students = vec![c.scenario.students[0].clone(), other];
            c.scenario
                .dailytest_frames
                .retain(|f| f.student_key == "1001");
            let mut frame = c.scenario.dailytest_frames[0].clone();
            frame.student_key = "1002".into();
            frame.attempts[0].score = Some(72.75);
            frame.headers[0].score = Some(72.75);
            c.scenario.dailytest_frames.push(frame);
            c.judgment("DailyTest", None, None, None);
            c.scenario.teacher_judgments[0].verdict = UnitVerdict::Deferred;
            let mut other_judgment = c.scenario.teacher_judgments[0].clone();
            other_judgment.student_key = "1002".into();
            other_judgment.verdict = UnitVerdict::Confirmed;
            c.scenario.teacher_judgments.push(other_judgment);
            let ask = c.run();
            if let Some(ask) = ask.ask {
                require(
                    &mut errors,
                    ask.field == AskField::Student && ask.options.len() == 2,
                    "student_identity",
                    "same-name candidates must remain distinct",
                );
                for option in ask.options {
                    let mut r = c.request.clone();
                    r.student = option.value.clone();
                    let out = c.session().call(&r);
                    if answer(&mut errors, &out, "student_identity") {
                        let own_confirmed = out.data["student_key"] == "1002";
                        require(
                            &mut errors,
                            out.display.contains("완료 판정") == own_confirmed
                                && out.display.contains("판정 보류") != own_confirmed,
                            "judgment_identity",
                            "same-name students inherited each other's teacher judgment",
                        );
                        let expected = if out.data["student_key"] == "1002" {
                            72.75
                        } else {
                            c.scenario.dailytest_frames[0].attempts[0].score.unwrap()
                        };
                        require(
                            &mut errors,
                            out.data["daily"]["attempts"]
                                .as_array()
                                .is_some_and(|rows| rows.iter().all(|a| a["score"] == expected)),
                            "student_identity",
                            "same-name DailyTest rows crossed student identity",
                        );
                    }
                    // Native prestudy rows expose a name and a row key, not a
                    // student key. The safe answer is qualified unknown here.
                    let prestudy = PrestudyRow {
                        student: NAMES[0].into(),
                        unit_label: format!("{} 1-1", c.request.book.as_ref().unwrap()),
                        date: "2026-11-10".into(),
                        key: "77001".into(),
                    };
                    c.scenario.prestudy.waiting.clear();
                    c.scenario.prestudy.completed.clear();
                    if c.seed % 4 == 1 {
                        c.scenario.prestudy.waiting.push(prestudy);
                    } else {
                        c.scenario.prestudy.completed.push(prestudy);
                    }
                    r.topic = Some("예습".into());
                    r.unit = Some("1-1".into());
                    let out = c.session().call(&r);
                    require(
                        &mut errors,
                        !out.data["prestudy"].as_array().is_some_and(|rows| {
                            rows.iter()
                                .any(|a| a["verdict"] == "waiting" || a["verdict"] == "completed")
                        }),
                        "B4",
                        "name-only prestudy row asserted for a resolved same-name student",
                    );
                }
            } else {
                require(
                    &mut errors,
                    false,
                    "student_identity",
                    "same-name request did not ask",
                );
            }
        }
        errors
    });
}

#[test]
fn i4_dynamic_cells_and_headers_cannot_change_table_structure() {
    sweep("I4", |c| {
        let mut errors = vec![];
        c.scope(2, "섹션");
        let payload = match c.seed % 4 {
            0 => "추가|활동",
            1 => "추가\nHEADER_TAIL",
            2 => "추가<!--comment-->활동",
            _ => "추가\r\nHEADER_TAIL",
        };
        c.scenario.edition_trees[0].chapters[0].minors[0]
            .activities
            .push(payload.into());
        c.scenario.edition_trees[0].chapters[0].minors[0].title = format!("가상주제{payload}");
        // Header safety must not depend on whether any attempt uses it.
        let out = c.run();
        if answer(&mut errors, &out, "I4_answer") {
            let header = out
                .display
                .lines()
                .find(|l| l.starts_with("소단원 | "))
                .unwrap_or("");
            let count = header.split(" | ").count();
            for line in out.display.lines().filter(|l| l.contains(" | ")) {
                require(
                    &mut errors,
                    line.split(" | ").count() == count,
                    "B7",
                    "header/body cell counts differ",
                );
                require(
                    &mut errors,
                    !line.replace(" | ", "").contains('|'),
                    "B7",
                    "raw dynamic pipe in table cell/header",
                );
            }
            require(
                &mut errors,
                !out.display.lines().any(|l| l.starts_with("HEADER_TAIL")),
                "B7",
                "dynamic header introduced a raw line break",
            );
            require(
                &mut errors,
                !out.display.contains("<!--") && !out.display.contains("-->"),
                "comment_delimiter",
                "dynamic title/header contains an HTML comment delimiter",
            );
            require(
                &mut errors,
                !row(&out, "1-1").is_some_and(|line| line.contains("<!--") || line.contains("-->")),
                "comment_delimiter_body",
                "encoded body title still contains a raw HTML comment delimiter, independently of header encoding",
            );
        }
        errors
    });
}

#[test]
fn i5_legitimate_large_inputs_produce_useful_bounded_complete_unit_blocks() {
    sweep("I5", |c| {
        let mut errors = vec![];
        let count = 1 + (c.seed as usize * 17) % 70;
        c.scope(
            count,
            if c.seed % 2 == 0 {
                "DailyTest"
            } else {
                "클리닉"
            },
        );
        if c.seed % 2 == 1 {
            c.own_sections(vec![]);
            for i in 1..=count {
                c.stage(&format!("1-{i}"), "original_work", "", false);
            }
        }
        let out = c.run();
        let usable = answer(&mut errors, &out, "B9");
        require(
            &mut errors,
            out.display.len() <= 6000,
            "B9",
            "display exceeds 6000 UTF-8 bytes",
        );
        require(
            &mut errors,
            out.stop
                .as_ref()
                .is_none_or(|s| s.code != StopCode::DisplayInvalid),
            "B9",
            "legitimate classroom input yielded display_invalid",
        );
        if usable {
            let retained = out.data["minors"].as_array().map_or(0, Vec::len);
            require(
                &mut errors,
                retained > 0 && retained <= 60,
                "B9",
                "answer lost every unit or exceeded 60",
            );
            require(
                &mut errors,
                out.display.contains("읽은 원천")
                    && out
                        .display
                        .ends_with("이번 결과는 읽기만 했고 저장한 것은 없습니다."),
                "B9",
                "bounded answer lost source footer or closing statement",
            );
            if retained < count {
                let code = format!("1-{}", retained + 1);
                require(
                    &mut errors,
                    out.data["first_omitted"]
                        .as_str()
                        .is_some_and(|s| s.starts_with(&format!("{code} ")))
                        && out
                            .display
                            .contains(&format!("표시 한도로 생략한 첫 소단원: {code} ")),
                    "B9",
                    "truncation notice does not name first omitted tree unit",
                );
                for i in retained + 1..=count {
                    require(
                        &mut errors,
                        !out.display
                            .lines()
                            .any(|l| l.starts_with(&format!("1-{i} "))),
                        "B9_blocks",
                        format!("omitted unit 1-{i} still has stage/detail rows"),
                    );
                }
            }
        }
        errors
    });
}

#[test]
fn i6_every_choice_exits_its_ask_and_every_stop_offers_a_remedy() {
    sweep("I6", |c| {
        let mut errors = vec![];
        c.scope(2, "섹션");
        match c.seed % 9 {
            0 => {
                c.request.unit_from = Some("1-2".into());
                c.request.unit_to = Some("1-1".into());
            }
            1 => {
                c.request.unit_from = Some("1-1".into());
            }
            2 => {
                c.request.book = Some(format!(
                    "{} 필수예제 유형다지기",
                    c.request.book.as_ref().unwrap()
                ));
            }
            3 => {
                c.request.topic = Some("unresolved".into());
            }
            4 => {
                c.request.unit = Some("unresolved".into());
            }
            5 => {
                c.scenario.edition_trees.clear();
            }
            6 => {
                c.request.book = Some("미등록 판본".into());
            }
            7 => {
                if c.scenario.students[0].bindings.len() == 1 {
                    let mut binding = c.scenario.students[0].bindings[0].clone();
                    binding.label = "가우스 5-1-2".into();
                    binding.course_key = "102".into();
                    binding.book_no = "202".into();
                    c.scenario.students[0].bindings.push(binding.clone());
                    let mut tree = c.scenario.edition_trees[0].clone();
                    tree.edition = binding.label;
                    c.scenario.edition_trees.push(tree);
                }
                c.request.book = None;
                c.request.topic = Some("클리닉".into());
            }
            _ => {
                c.faults.notes_fail = true;
            }
        }
        let out = c.run();
        if let Some(ask) = &out.ask {
            require(
                &mut errors,
                !ask.options.is_empty(),
                "ask_empty",
                "ask offered no choice",
            );
            for option in &ask.options {
                for value in [&option.value, &option.label_ko] {
                    let mut next = c.request.clone();
                    match ask.field {
                        AskField::Student => next.student = value.clone(),
                        AskField::Book => next.book = Some(value.clone()),
                        AskField::Unit => next.unit = Some(value.clone()),
                        AskField::UnitFrom => next.unit_from = Some(value.clone()),
                        AskField::UnitTo => next.unit_to = Some(value.clone()),
                        AskField::Topic => next.topic = Some(value.clone()),
                        _ => {
                            require(
                                &mut errors,
                                false,
                                "missing_ask_seam",
                                "offered field cannot be supplied through UnitStatusRequest",
                            );
                            continue;
                        }
                    }
                    let answer = c.session().call(&next);
                    require(
                        &mut errors,
                        answer.stop.is_none()
                            && !answer.display.is_empty()
                            && answer.ask.as_ref() != Some(ask),
                        "B10",
                        format!(
                            "choice repeats same {:?} ask; original endpoints/book suffix preserved",
                            ask.field
                        ),
                    );
                }
            }
        } else if let Some(stop) = &out.stop {
            require(
                &mut errors,
                out.display.matches('.').count() == 2 && !out.display.contains("읽은 원천"),
                "stop_form",
                "stop must contain exactly two sentences and no partial result",
            );
            require(
                &mut errors,
                stop.message_ko.contains("확인해") || stop.message_ko.contains("요청해"),
                "stop_remedy",
                "stop lacks concrete teacher remedy",
            );
        } else {
            require(
                &mut errors,
                false,
                "ask_setup",
                "ambiguous/invalid request unexpectedly answered",
            );
        }
        errors
    });
}

#[test]
fn i7_each_source_retains_its_own_observation_time() {
    sweep("I7", |c| {
        let mut errors = vec![];
        c.scope(
            2,
            if c.seed % 2 == 0 {
                "전체"
            } else {
                "DailyTest"
            },
        );
        let minute = c.seed % 60;
        c.faults.page_at = Some(format!("2026-11-11T10:{minute:02}:00+09:00"));
        c.faults.header_at = Some(format!("2026-11-11T08:{minute:02}:00+09:00"));
        let out = c.run();
        if answer(&mut errors, &out, "I7_answer") {
            for (name, clock) in [
                ("dailytest", format!("10:{minute:02}")),
                ("dailytest_header", format!("08:{minute:02}")),
                ("edition_tree", "09:00".into()),
                ("student_book_bindings", "09:00".into()),
            ] {
                let observed = out.coverage.sources.iter().find(|s| s.name == name);
                require(
                    &mut errors,
                    observed.is_some(),
                    "B11_time",
                    format!("missing source provenance: {name}"),
                );
                require(
                    &mut errors,
                    out.display.contains(&clock),
                    "B11_time",
                    format!("{name} own time {clock} missing from display"),
                );
            }
            for (label, at) in [
                ("판본 전체 목차", NOW),
                ("학생·교재 등록", NOW),
                ("일일시험 제목", c.faults.header_at.as_deref().unwrap()),
            ] {
                require(
                    &mut errors,
                    source_clock(&out.display, label, at),
                    "B11_association",
                    format!("{label} lacks its own {at}; another source's clock cannot qualify it"),
                );
            }
        }
        errors
    });
}

#[test]
fn i8_summary_never_denies_a_scoped_detail() {
    sweep("I8", |c| {
        let mut errors = vec![];
        c.scope(1 + c.seed as usize % 3, "전체");
        if c.seed % 2 == 0 {
            c.stage("1-1", "original_work", "", false);
        } else {
            c.stage("1-1", "clinic_stage", "original_clinic_solving", true);
            c.stage("1-1", "clinic_stage", "similar_clinic", true);
        }
        let out = c.run();
        if answer(&mut errors, &out, "I8_answer") {
            for (stage, label) in [
                ("original_clinic_solving", "오답클리닉 풀이"),
                ("original_clinic_grading", "오답클리닉 채점"),
                ("similar_clinic", "유사클리닉"),
                ("advanced_clinic", "발전클리닉"),
                ("cism_support", "CISM 지원"),
            ] {
                let known = c.seed % 2 == 0
                    || ["original_clinic_solving", "similar_clinic"].contains(&stage);
                if known {
                    let expected = if c.seed % 2 == 0 {
                        "미확인(아직 확인할 단계 아님)"
                    } else {
                        "완료 판정"
                    };
                    require(
                        &mut errors,
                        out.display
                            .lines()
                            .filter(|l| {
                                l.starts_with(&format!("1-1 {label}: ")) && l.contains(expected)
                            })
                            .count()
                            == 1,
                        "stage_rows",
                        format!("generated 1-1/{label} fact missing or repeated"),
                    );
                    require(
                        &mut errors,
                        !out.display
                            .lines()
                            .any(|l| l.starts_with(&format!("{label}: 미확인("))),
                        "B8",
                        format!(
                            "global {label} unknown/teacher-only contradicts confirmed/not-yet-expected detail"
                        ),
                    );
                }
            }
        }
        errors
    });
}

/// B13's public types cannot represent course identity on a judgment, or
/// course/attempt identity on an exclusion. This counterexample tests the
/// representable half: an exclusion cannot silently suppress every retest.
#[test]
fn b13_unscoped_exclusion_does_not_silently_exclude_every_attempt() {
    let mut c = Case::new(13);
    c.scope(1, "클리닉");
    c.own_sections(vec![
        section(0, 0, "1-1", "필수예제", 0),
        section(0, 0, "1-1", "필수예제", 1),
    ]);
    c.scenario.excluded_sections.push(ExcludedSection {
        course_key: None,
        book_no: None,
        snapshot: None,
        student_key: "1001".into(),
        edition: c.request.book.clone().unwrap(),
        unit: "1-1".into(),
        activity: "필수예제".into(),
        state: "active".into(),
        actor: None,
        recorded_at: None,
    });
    let out = c.run();
    let suppressed = out.data["clinic"].as_array().is_some_and(|rows| {
        rows.iter().any(|r| {
            r["activity"] == "필수예제"
                && r["verdict"] == serde_json::to_value(UnitVerdict::Excluded).unwrap()
        })
    });
    assert!(
        !suppressed
            || out
                .display
                .lines()
                .any(|l| l.contains("필수예제") && l.contains("제외") && l.contains("미확인")),
        "B13 seed=13 unit=1-1 attempts=2 exclusion has no course/attempt scope; cannot silently apply to both"
    );
}
