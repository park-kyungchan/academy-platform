#[allow(dead_code)]
#[path = "support/unit_gen.rs"]
mod unit_gen;
use edu_academy_teacher::display_labels as l;
use unit_gen::*;

#[test]
fn m23a_remedies_describe_missing_source_information() {
    for (code, expected) in [
        ("daily_mapping_unavailable", "원천에 시험지와 교재 판본·단원의 연결이 없습니다"),
        ("prestudy_name_ambiguous", "예습 원천에 같은 이름의 학생을 구분할 식별자가 없습니다"),
        ("section_mapping_unavailable", "원문 제목에 판본 목차와 연결할 단원·활동 정보가 없습니다"),
        ("section_ambiguous_attempts", "원천의 동일 제목만으로 각 시험 시도의 성적을 연결할 수 없습니다"),
    ] {
        assert_eq!(l::unit_unknown_remedy(code), expected, "{code}");
    }
    assert!(l::UNIT_COURSE_UNKNOWN.contains("교사 기록에"));
    assert!(l::UNIT_EXCLUSION_SCOPE_UNKNOWN.contains("섹션 제외 기록에"));
    assert!(l::unit_unmapped_title("가상제목").contains(l::unit_unknown_remedy("section_mapping_unavailable")));
    assert!(l::unit_unmapped_summary(2).contains(l::unit_unknown_remedy("section_mapping_unavailable")));
}

#[test]
fn m23b_duplicate_course_ordinals_restart_within_each_book() {
    let mut c = Case::new(1);
    c.scope(2, "클리닉");
    let mut first = c.scenario.students[0].bindings[0].clone();
    let mut second = first.clone();
    second.label = "가우스 5-1-2".into();
    second.course_key = "202".into();
    second.book_no = "302".into();
    let mut third = second.clone();
    third.course_key = "203".into();
    third.book_no = "303".into();
    first.label = "가우스 5-1-1".into();
    c.scenario.students[0].bindings = vec![first, second, third];
    c.request.book = None;
    let out = c.run();
    let ask = out.ask.unwrap();
    let values: Vec<_> = ask.options.iter().map(|a| a.value.as_str()).collect();
    assert_eq!(values, ["가우스 5-1-1", "가우스 5-1-2 (등록 과정 1)", "가우스 5-1-2 (등록 과정 2)"]);
    assert!(ask.question_ko.contains("가우스 5-1-2: 같은 교재가 2개 과정에 등록되어 있습니다"));
}

#[test]
fn m23c_book_choice_retains_the_unit_suffix() {
    let mut c = Case::new(1);
    c.scope(3, "클리닉");
    let first = c.scenario.students[0].bindings[0].clone();
    let mut second = first.clone();
    second.course_key = "202".into();
    second.book_no = "302".into();
    c.scenario.students[0].bindings = vec![first.clone(), second];
    c.request.book = Some(format!("{} 1-2", first.label));
    let ask = c.run().ask.expect("duplicate book asks for course");
    for option in ask.options {
        for value in [option.value, option.label_ko] {
            let mut answered = c.request.clone();
            answered.book = Some(value);
            let out = c.session().call(&answered);
            assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
            let minors = out.data["minors"].as_array().unwrap();
            assert_eq!(minors.len(), 1);
            assert_eq!(minors[0]["minor"]["code"], "1-2");
        }
    }
}

fn render_data(c: &Case) -> edu_academy_teacher::unit_data::UnitData {
    use edu_academy_teacher::unit_data::UnitData;
    let tree = &c.scenario.edition_trees[0];
    UnitData {
        course_key: "101".into(), book_no: "201".into(), student_key: "1001".into(),
        student: NAMES[0].into(), edition: tree.edition.clone(), family: tree.family.clone(),
        topic: "예습".into(), default_read_at: NOW.into(), read_at: NOW.into(), tree_read_at: NOW.into(),
        minors: tree.leaves(), activity: None, level: None, daily: None, sections: None,
        judgments: vec![], excluded: vec![], stages: vec![], prestudy: vec![], clinic: vec![], first_omitted: None,
    }
}

#[test]
fn m23d_trimmed_unknowns_follow_major_middle_and_minor_identifiers() {
    use edu_academy_teacher::{envelope::Coverage, unit_data::{Chapter, Middle, Minor}, unit_render};
    let mut c = Case::new(1);
    c.scenario.edition_trees[0].chapters = ["A", "Z"].into_iter().map(|id| Chapter {
        code: format!("major {id}"), title: format!("가상대단원{id}"), minors: vec![],
        middles: vec![Middle { code: format!("middle {id}"), title: format!("가상중단원{id}"),
            minors: vec![Minor { code: format!("minor {id}"), title: "가상소단원".into(),
                activities: vec!["필수예제".into()], unit_wide: false }] }],
    }).collect();
    for level in ["major", "middle", "minor"] {
        let mut d = render_data(&c);
        d.topic = "섹션".into();
        d.sections = Some(edu_academy_teacher::unit_data::SectionRead {
            index_observed: true, paper_observed: true,
            records: d.minors.iter().map(|leaf| edu_academy_teacher::unit_data::Section {
                student_key: "1001".into(), student_name: NAMES[0].into(), course_key: "101".into(), book_no: "201".into(),
                unit: leaf.minor.code.clone(), activity: "필수예제".into(), source_title: "가".repeat(1100),
                paper_key: "1".into(), attempt_key: "2".into(), score: 70.0, items: vec![], source_date: None, date_precision: "unknown".into(),
            }).collect(), ..Default::default()
        });
        let mut coverage = Coverage { sources: vec![], unknowns: vec![
            format!("section_grade_unavailable:{level} A 필수예제"),
            format!("section_grade_unavailable:{level} Z 필수예제"),
        ] };
        let display = unit_render::render(&mut d, &mut coverage);
        assert_eq!(d.minors.len(), 1, "{level}");
        assert!(display.contains(&format!("{level} A 필수예제·")), "{level}");
        assert!(!display.contains(&format!("{level} Z 필수예제·")), "{level}");
    }
}

#[test]
fn m23e_only_cell_affecting_teacher_report_kinds_need_course_scope() {
    for kind in ["original_work", "clinic_stage", "assigned_stages", "notebook_check", "homework_stage_check", "follow_up"] {
        for topic in ["전체", "클리닉", "섹션", "DailyTest", "예습"] {
            let mut c = Case::new(1);
            c.scope(1, topic);
            let first = c.scenario.students[0].bindings[0].clone();
            let mut second = first.clone();
            second.course_key = "202".into(); second.book_no = "302".into();
            c.scenario.students[0].bindings = vec![first.clone(), second];
            c.request.book = Some(format!("{} (등록 과정 1)", first.label));
            c.stage("1-1", kind, "original_clinic_solving", kind != "original_work");
            let out = c.run();
            assert!(out.ask.is_none() && out.stop.is_none(), "{}", out.display);
            let expected = ["original_work", "clinic_stage"].contains(&kind) && ["전체", "클리닉"].contains(&topic);
            assert_eq!(out.coverage.unknowns.iter().any(|u| u.starts_with("teacher_fact_course_unknown:")), expected, "{kind}/{topic}");
            c.scenario.teacher_fact_reports[0].course_key = Some("101".into());
            c.scenario.teacher_fact_reports[0].book_no = Some("201".into());
            let scoped = c.run();
            assert!(!scoped.coverage.unknowns.iter().any(|u| u.starts_with("teacher_fact_course_unknown:")));
            assert_eq!(out.data["stages"] != scoped.data["stages"], ["original_work", "clinic_stage"].contains(&kind) && ["전체", "클리닉", "섹션"].contains(&topic), "stage effect {kind}/{topic}");
        }
    }
}

#[test]
fn m23f_legend_tracks_printed_dash_values_not_source_titles() {
    let mut c = Case::new(1);
    c.scope(2, "섹션");
    for minor in &mut c.scenario.edition_trees[0].chapters[0].minors {
        minor.activities = vec!["필수예제".into()];
    }
    let rows: Vec<_> = ["1-1", "1-2"].into_iter().enumerate().map(|(i, unit)| {
        let mut a = section(0, 0, unit, "필수예제", i);
        a.title = a.title.replace("가상주제", "가상주제 — 제목");
        a
    }).collect();
    c.own_sections(rows.clone());
    let out = c.run();
    assert!(out.ask.is_none() && out.stop.is_none());
    assert!(out.display.contains("가상주제 — 제목"));
    assert_eq!(out.display.matches(l::UNIT_DASH_LEGEND).count(), 0);
    c.own_sections(vec![rows[0].clone()]);
    let out = c.run();
    assert!(out.display.lines().any(|line| line.ends_with(" | —")));
    assert_eq!(out.display.matches(l::UNIT_DASH_LEGEND).count(), 1);
    c.scope(1, "클리닉");
    for item in &mut c.scenario.sections.attempts[0].items { item.scoring = "O".into(); }
    let out = c.run();
    assert_eq!(out.display.matches(l::UNIT_DASH_LEGEND).count(), 0);
}

fn lesson_data() -> (edu_academy_teacher::lesson::LessonData, edu_academy_teacher::envelope::Coverage) {
    use edu_academy_teacher::{backend::SourceBackend, facts::SeededFacts, lesson::PrepareRequest, load_scenario, prepare};
    let scenario = load_scenario(include_str!("fixtures/scenario-v2.json")).unwrap();
    let facts = SeededFacts::from_scenario(&scenario);
    let mut backend = SourceBackend::synthetic(scenario, NOW);
    let out = prepare(&mut backend, &facts, &PrepareRequest { student: Some(NAMES[0].into()), ..Default::default() }, chrono::NaiveDate::from_ymd_opt(2026, 11, 11).unwrap());
    assert!(out.ask.is_none() && out.stop.is_none());
    (serde_json::from_value(out.data).unwrap(), out.coverage)
}

#[test]
fn m23g_unsupported_server_codes_have_explicit_display_labels() {
    let (mut data, coverage) = lesson_data();
    data.teacher_fact_reports[0].stages = vec!["invented_stage_739".into()];
    let display = edu_academy_teacher::render::lesson_prepare(&data, &coverage);
    assert!(!display.contains("invented_stage_739"));
    assert!(display.contains("미확인(지원하지 않는 단계)"));
    assert_eq!(l::unit_stage_name("invented_stage_739"), "미확인(지원하지 않는 단계)");
    assert_eq!(l::unit_source_name("invented_source_739"), "미확인 원천");
}

#[test]
fn m23h_unread_grade_sentences_encode_names_for_one_and_many_students() {
    use edu_academy_teacher::render::{self, Verdict};
    for count in [1, 2] {
        let (mut data, coverage) = lesson_data();
        data.students[0].student = "가상학생A|가상학생B\n가상학생C".into();
        for check in &mut data.students[0].checks {
            if check.kind == "section" { check.verdict = Verdict::NotObserved; }
        }
        if count == 2 { data.students.push(data.students[0].clone()); }
        let display = render::lesson_prepare(&data, &coverage);
        let unknown = display.split("미확인 사항\n").nth(1).unwrap().split("다음 확인\n").next().unwrap();
        assert!(unknown.contains("가상학생A｜가상학생B / 가상학생C"));
        assert!(!unknown.contains('|'));
        assert!(!display.lines().any(|line| line.starts_with("가상학생C")));
        assert!(render::display_lint(&display).is_ok(), "{}", display);
    }
}

#[test]
fn m23h_source_time_and_date_slots_are_structurally_encoded() {
    use edu_academy_teacher::render;
    let (mut data, mut coverage) = lesson_data();
    data.read_at = "x|y\nz".into();
    data.reminders[0].at = "2026-11-11T1|2\n3".into();
    for source in &mut coverage.sources { source.read_at = data.read_at.clone(); }
    let display = render::lesson_prepare(&data, &coverage);
    assert!(display.contains("x｜y / z"));
    assert!(!display.lines().any(|line| line.starts_with("z")));
    assert!(render::display_lint(&display).is_ok(), "{}", display);
    assert_eq!(l::date("x|y\nz"), "x｜y / z");
}
