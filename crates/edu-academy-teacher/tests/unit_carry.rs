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
