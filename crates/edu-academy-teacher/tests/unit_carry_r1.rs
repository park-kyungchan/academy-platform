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
        c.request.student = ask.options.iter().find(|o| o.value.contains("가상반A")).unwrap().value.clone();
        let out = c.run();
        assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
        assert!(out.coverage.unknowns.iter().any(|u| u == "prestudy_name_ambiguous"));
        assert_eq!(out.display.lines().find(|line| line.starts_with("1-1 예습영상: ")).unwrap(),
            "1-1 예습영상: 미확인(같은 이름의 학생을 구분하지 못함)");
    }
}

fn daily_cell<'a>(out: &'a edu_academy_teacher::envelope::Envelope, count: usize, topic: &str) -> &'a str {
    let prefix = if count == 1 || topic == "전체" { "1-1 원천: " } else { "1-1 가상주제1 | " };
    out.display.lines().find_map(|line| line.strip_prefix(prefix)).unwrap()
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
            assert_eq!(daily_cell(&out, count, topic), "미확인(일일시험의 교재·단원 연결 불가)");
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
        for (header, expected) in [(true, "미확인(일일시험 제목을 읽지 못함)"), (false, "미확인(일일시험의 일부 관측 페이지를 읽지 못함)")] {
            let mut c = Case::new(1);
            c.scope(2, topic);
            c.faults.header_fail = header;
            if !header { c.faults.daily_fail = Some("11".into()); }
            let out = c.run();
            let prefix = if topic == "전체" { "1-2 원천: " } else { "1-2 가상주제2 | " };
            assert_eq!(out.display.lines().find_map(|line| line.strip_prefix(prefix)).unwrap(), expected);
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
        assert!(out.coverage.sources.iter().any(|s| s.name == "section_index"));
        assert_eq!(out.display.lines().find(|line| line.starts_with("읽지 못한 원천 또는 범위: ")).unwrap(),
            "읽지 못한 원천 또는 범위: 시험지 검색의 관측 범위 미확인·시험지 검색의 제목 목록을 유효한 원천 관측으로 확인하지 못했습니다.");
        if topic != "클리닉" {
            assert_eq!(out.display.lines().find(|line| line.starts_with("1-1 섹션: ")).unwrap(),
                "1-1 섹션: 미확인(시험지 검색의 관측 범위 미확인)");
        }
    }
}
