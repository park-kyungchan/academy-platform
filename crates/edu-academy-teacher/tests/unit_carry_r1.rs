#[allow(dead_code)]
#[path = "support/unit_gen.rs"]
mod unit_gen;
use unit_gen::*;

#[test]
fn r1_successful_name_ambiguity_has_its_own_prestudy_cell() {
    for topic in ["예습", "전체"] {
        let mut c = Case::new(1);
        c.scope(1, topic);
        c.scenario.students.truncate(1);
        let mut second = c.scenario.students[0].clone();
        second.key = "1002".into();
        second.class_name = "가상반B".into();
        c.scenario.students.push(second);
        let ask = c.run().ask.expect("same-name student asks");
        c.request.student = ask
            .options
            .iter()
            .find(|o| o.value.contains("가상반A"))
            .unwrap()
            .value
            .clone();
        let out = c.run();
        assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
        assert!(
            out.coverage
                .unknowns
                .iter()
                .any(|u| u == "prestudy_name_ambiguous")
        );
        assert_eq!(
            out.display
                .lines()
                .find(|line| line.starts_with("1-1 예습영상: "))
                .unwrap(),
            "1-1 예습영상: 미확인(같은 이름의 학생을 구분하지 못함)"
        );
    }
}

fn daily_cell<'a>(
    out: &'a edu_academy_teacher::envelope::Envelope,
    count: usize,
    topic: &str,
) -> &'a str {
    let prefix = if count == 1 || topic == "전체" {
        "1-1 원천: "
    } else {
        "1-1 가상주제1 | "
    };
    out.display
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap()
}

#[test]
fn r1_successful_daily_mapping_and_missing_score_are_not_read_failures() {
    for count in [1, 2] {
        for topic in ["DailyTest", "전체"] {
            let mut c = Case::new(1);
            c.scope(count, topic);
            let mut attempt = c.scenario.dailytest_frames[0].attempts[0].clone();
            attempt.edition_mapping = None;
            c.own_daily(vec![attempt.clone()]);
            let out = c.run();
            assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
            assert_eq!(
                daily_cell(&out, count, topic),
                "미확인(일일시험의 교재·단원 연결 불가)"
            );
            attempt.edition_mapping = c.request.book.clone();
            attempt.score = None;
            c.own_daily(vec![attempt]);
            let out = c.run();
            assert_eq!(daily_cell(&out, count, topic), "미확인(원천 점수 없음)");
        }
    }
}

#[test]
fn r1_daily_header_and_partial_failures_name_only_the_failed_source() {
    for topic in ["DailyTest", "전체"] {
        for (header, expected) in [
            (true, "미확인(일일시험 제목을 읽지 못함)"),
            (false, "미확인(일일시험의 일부 관측 페이지를 읽지 못함)"),
        ] {
            let mut c = Case::new(1);
            c.scope(2, topic);
            c.faults.header_fail = header;
            if !header {
                c.faults.daily_fail = Some("11".into());
            }
            let out = c.run();
            let prefix = if topic == "전체" {
                "1-2 원천: "
            } else {
                "1-2 가상주제2 | "
            };
            assert_eq!(
                out.display
                    .lines()
                    .find_map(|line| line.strip_prefix(prefix))
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn r1_successful_section_index_without_qualified_paper_search_is_not_a_whole_source_failure() {
    for topic in ["섹션", "전체", "클리닉"] {
        let mut c = Case::new(1);
        c.scope(1, topic);
        c.own_sections(vec![]);
        c.faults.paper_fail = true;
        let out = c.run();
        assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
        assert!(
            out.coverage
                .sources
                .iter()
                .any(|s| s.name == "section_index")
        );
        assert_eq!(
            out.display
                .lines()
                .find(|line| line.starts_with("읽지 못한 원천 또는 범위: "))
                .unwrap(),
            "읽지 못한 원천 또는 범위: 시험지 검색의 관측 범위 미확인·시험지 검색의 제목 목록을 유효한 원천 관측으로 확인하지 못했습니다."
        );
        if topic != "클리닉" {
            assert_eq!(
                out.display
                    .lines()
                    .find(|line| line.starts_with("1-1 섹션: "))
                    .unwrap(),
                "1-1 섹션: 미확인(시험지 검색의 관측 범위 미확인)"
            );
        }
    }
}

fn prefix_books(reverse: bool, duplicates: bool) -> Case {
    let mut c = Case::new(1);
    c.scope(2, "섹션");
    c.scenario.sections.attempts.clear();
    let template = c.scenario.students[0].bindings[0].clone();
    c.scenario.students[0].bindings = ["수학", "수학 2"]
        .into_iter()
        .enumerate()
        .flat_map(|(b, name)| {
            let template = template.clone();
            (0..if duplicates { 2 } else { 1 }).map(move |course| {
                let mut binding = template.clone();
                binding.label = name.into();
                binding.course_key = format!("{}", 101 + b * 10 + course);
                binding.book_no = format!("{}", 201 + b * 10 + course);
                binding
            })
        })
        .collect();
    if reverse {
        c.scenario.students[0].bindings.reverse();
    }
    let tree = c.scenario.edition_trees[0].clone();
    c.scenario.edition_trees = ["수학", "수학 2"]
        .into_iter()
        .map(|name| {
            let mut tree = tree.clone();
            tree.edition = name.into();
            tree
        })
        .collect();
    c
}

#[test]
fn r1_longest_registered_book_wins_in_both_registration_orders() {
    for reverse in [false, true] {
        for (text, book, count) in [
            ("수학 1-1", "수학", 1),
            ("수학 2 1-1", "수학 2", 1),
            ("수학 2", "수학 2", 2),
        ] {
            let mut c = prefix_books(reverse, false);
            c.request.book = Some(text.into());
            let out = c.run();
            assert!(
                out.ask.is_none() && out.stop.is_none(),
                "{reverse}/{text}: {}",
                out.display
            );
            assert_eq!(out.data["edition"], book);
            assert_eq!(out.data["minors"].as_array().unwrap().len(), count);
            let expected = if count == 1 {
                format!("가상학생A {book} 대단원 1 소단원 1-1 가상주제1 섹션 결과")
            } else {
                format!("가상학생A {book}  섹션 결과")
            };
            assert_eq!(out.display.lines().next().unwrap(), expected);
        }
    }
}

#[test]
fn r1_course_ask_keeps_only_longest_book_and_its_own_unit_suffix() {
    for reverse in [false, true] {
        let mut c = prefix_books(reverse, true);
        c.request.book = Some("수학 2 1-1".into());
        let out = c.run();
        let ask = out.ask.expect("same longest book registered twice");
        assert_eq!(
            out.display,
            "가상학생A의 교재 등록을 확인했습니다.\n수학 2: 같은 교재가 2개 과정에 등록되어 있습니다.\n어느 교재와 등록 과정을 볼까요?\n- 수학 2 (등록 과정 1) 1-1: 이 교재의 현황을 봅니다.\n- 수학 2 (등록 과정 2) 1-1: 이 교재의 현황을 봅니다."
        );
        assert_eq!(
            ask.options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ["수학 2 (등록 과정 1) 1-1", "수학 2 (등록 과정 2) 1-1"]
        );
        for option in ask.options {
            c.request.book = Some(option.value);
            let out = c.run();
            assert_eq!(out.data["edition"], "수학 2");
            assert_eq!(out.data["minors"].as_array().unwrap().len(), 1);
            assert_eq!(
                out.display.lines().next().unwrap(),
                "가상학생A 수학 2 대단원 1 소단원 1-1 가상주제1 섹션 결과"
            );
        }
    }
}

#[test]
fn r1_course_count_uses_all_registrations_when_context_filters_choices() {
    let mut c = Case::new(1);
    c.scope(1, "전체");
    let first = c.scenario.students[0].bindings[0].clone();
    c.scenario.students[0].bindings = (0..3)
        .map(|i| {
            let mut binding = first.clone();
            binding.course_key = format!("{}", 101 + i);
            binding.book_no = if i == 1 { "202" } else { "201" }.into();
            binding
        })
        .collect();
    c.stage("1-1", "assigned_stages", "", true);
    c.scenario.teacher_fact_reports[0].book_no = Some("201".into());
    c.request.book = None;
    let out = c.run();
    let ask = out.ask.expect("two remaining courses still ask");
    assert_eq!(
        out.display,
        "가상학생A의 교재 등록을 확인했습니다.\n가우스 5-1-1: 같은 교재가 3개 과정에 등록되어 있습니다.\n어느 교재와 등록 과정을 볼까요?\n- 가우스 5-1-1 (등록 과정 1): 이 교재의 현황을 봅니다.\n- 가우스 5-1-1 (등록 과정 3): 이 교재의 현황을 봅니다."
    );
    assert_eq!(
        ask.options
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["가우스 5-1-1 (등록 과정 1)", "가우스 5-1-1 (등록 과정 3)"]
    );
}
