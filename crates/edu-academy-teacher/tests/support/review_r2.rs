use crate::unit_gen::*;
type Failures = Vec<(&'static str, String)>;
fn check(e: &mut Failures, ok: bool, tag: &'static str) {
    if !ok {
        e.push((tag, tag.into()));
    }
}
pub fn views(c: &mut Case) -> Failures {
    let mut e = vec![];
    c.scope(2, "전체");
    match c.seed % 6 {
        0 => c.faults.notes_fail = true,
        1 => c.faults.daily_fail = Some("all".into()),
        2 => c.faults.prestudy_fail = true,
        3 => {
            c.scope(2, "클리닉");
            for a in &mut c.scenario.sections.attempts {
                for i in &mut a.items {
                    i.scoring = "O".into();
                }
            }
        }
        4 => {
            c.scope(2, "섹션");
            c.scenario.edition_trees[0].chapters[0].minors[0]
                .activities
                .pop();
        }
        _ => {}
    }
    let o = c.run();
    check(&mut e, o.stop.is_none() && o.ask.is_none(), "answer");
    if matches!(c.seed % 6, 0 | 5) {
        check(
            &mut e,
            o.display
                .lines()
                .any(|l| l.starts_with("소단원 | 필수예제 | 유형다지기 | 실력쌓기 | 대단원총괄")),
            "all_header",
        );
    }
    if o.display.contains("—") {
        check(
            &mut e,
            o.display.contains("—: 미확인(읽은 섹션 결과에 없음)"),
            "dash_legend",
        );
    }
    match c.seed % 6 {
        0 => {
            let row = o
                .display
                .lines()
                .find(|l| l.starts_with("1-1 ") && l.contains(" | "))
                .unwrap_or("");
            check(
                &mut e,
                !row.contains("—") && row.contains("읽지 못"),
                "section_whole_failure",
            );
        }
        1 => check(
            &mut e,
            o.display
                .lines()
                .any(|l| l.starts_with("1-1") && l.contains("원천:") && l.contains("읽지 못")),
            "daily_whole_failure",
        ),
        2 => check(
            &mut e,
            o.display
                .lines()
                .any(|l| l.starts_with("1-1") && l.contains("예습") && l.contains("읽지 못")),
            "prestudy_whole_failure",
        ),
        3 => check(
            &mut e,
            o.display.contains("클리닉 대상 아님"),
            "not_eligible",
        ),
        4 => check(
            &mut e,
            o.display
                .lines()
                .any(|l| l.starts_with("1-1 ") && l.ends_with("해당 없음")),
            "not_applicable",
        ),
        _ => {}
    }
    e
}
pub fn courses(c: &mut Case) -> Failures {
    let mut e = vec![];
    c.scope(2, "클리닉");
    let first = c.scenario.students[0].bindings[0].clone();
    let mut second = first.clone();
    second.course_key = "303".into();
    second.book_no = "403".into();
    c.scenario.students[0].bindings = vec![first, second];
    c.judgment("클리닉", Some("필수예제"), None, None);
    if c.seed % 3 != 0 {
        c.request.book = None;
    }
    if c.seed % 3 == 2 {
        c.request.topic = Some("전체".into());
        c.stage("1-1", "assigned_stages", "", true);
    }
    let o = c.run();
    let Some(ask) = o.ask else {
        check(&mut e, false, "duplicate_course_ask");
        return e;
    };
    let labels: std::collections::HashSet<_> = ask.options.iter().map(|a| &a.label_ko).collect();
    check(
        &mut e,
        labels.len() == 2 && ask.options.len() == 2,
        "distinct_course_labels",
    );
    for a in ask.options {
        for v in [a.value, a.label_ko] {
            let mut r = c.request.clone();
            r.book = Some(v);
            let o = c.session().call(&r);
            check(
                &mut e,
                o.ask.is_none() && o.stop.is_none(),
                "course_choice_exits",
            );
            check(
                &mut e,
                o.display.contains("과정")
                    && o.display.contains("기록되지")
                    && o.display.contains("지정해"),
                "unscoped_fact_retained",
            );
        }
    }
    e
}
pub fn unknowns(c: &mut Case) -> Failures {
    let mut e = vec![];
    c.scope(1, "DailyTest");
    let template = c.scenario.dailytest_frames[0].attempts[0].clone();
    let count = if c.seed % 20 == 1 { 160 } else { 2 } + c.seed as usize % 21;
    let rows = (0..count)
        .map(|i| {
            let mut a = template.clone();
            a.paper_key = format!("{}", 91000 + i);
            a.attempt_key = format!("{}", 92000 + i);
            if c.seed % 2 == 0 {
                a.edition_mapping = None;
            }
            a
        })
        .collect();
    c.own_daily(rows);
    c.faults.header_fail = c.seed % 2 == 1;
    let o = c.run();
    check(
        &mut e,
        o.ask.is_none() && o.stop.is_none() && o.display.len() <= 6000,
        "bounded_unknowns",
    );
    let lines: Vec<_> = o
        .display
        .lines()
        .filter(|l| l.starts_with(edu_academy_teacher::display_labels::UNIT_SOURCE_UNKNOWN))
        .collect();
    check(&mut e, lines.len() == 1, "unknowns_once");
    check(
        &mut e,
        lines.iter().any(|l| {
            l.contains(&format!("{count}건")) && (l.contains("조회해") || l.contains("지정해"))
        }),
        "unknown_count_remedy",
    );
    e
}
