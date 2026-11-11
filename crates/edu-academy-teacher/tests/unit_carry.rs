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
