#[path = "support/review_r2.rs"]
mod review_r2;
#[allow(dead_code)]
#[path = "support/unit_gen.rs"]
mod unit_gen;
use unit_gen::*;
#[test]
fn k1_all_topics_and_whole_section_failure() {
    assert!(review_r2::views(&mut Case::new(6)).is_empty());
}
#[test]
fn source_daily_failure_is_per_unit() {
    assert!(review_r2::views(&mut Case::new(1)).is_empty());
}
#[test]
fn source_prestudy_failure_is_per_unit() {
    assert!(review_r2::views(&mut Case::new(2)).is_empty());
}
#[test]
fn k2_book_course_choices_and_unknown_fact() {
    for s in 1..=3 {
        assert!(review_r2::courses(&mut Case::new(s)).is_empty());
    }
}
#[test]
fn m22e_header_outage_is_bounded_counted_and_actionable() {
    assert!(review_r2::unknowns(&mut Case::new(1)).is_empty());
}
#[test]
fn m22a_cached_tree_once_minute_form() {
    let mut c = Case::new(1);
    c.scope(1, "DailyTest");
    c.faults.page_at = Some("2026-11-11T10:00:01+09:00".into());
    let o = c.run();
    assert!(!o.display.contains("2026-11-11T09:00"));
    assert!(
        o.display
            .contains("판본 전체 목차: 원래 읽은 시각 11월 11일 09:00")
    );
    let mut c = Case::new(1);
    c.scope(1, "DailyTest");
    c.faults.page_at = Some("2026-11-11T09:00:51+09:00".into());
    let o = c.run();
    assert!(!o.display.contains("읽은 시각"));
}
#[test]
fn m22b_missing_activity_is_not_applicable() {
    assert!(review_r2::views(&mut Case::new(4)).is_empty());
}
#[test]
fn stage_default_remainder_is_compact_and_qualified() {
    let mut c = Case::new(1);
    c.scope(6, "전체");
    c.stage("1-1", "clinic_stage", "original_clinic_solving", true);
    let o = c.run();
    assert!(o.stop.is_none());
    let rows = o
        .display
        .lines()
        .filter(|l| l.contains("오답클리닉 풀이") && l.contains("교사만 확인"))
        .count();
    assert_eq!(rows, 1);
    assert!(o.display.contains("그 외"));
}
