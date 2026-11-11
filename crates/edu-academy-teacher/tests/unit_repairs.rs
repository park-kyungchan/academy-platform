//! Named generated witnesses, retained separately from the independent invariants.
#[allow(dead_code)]
#[path = "support/unit_gen.rs"]
mod unit_gen;
use unit_gen::*;

#[test]
fn seed_6_ambiguous_section_attempts_each_have_a_scoped_unknown() {
    let mut c = Case::new(6);
    c.scope(1, "섹션");
    c.own_sections(vec![
        section(0, 0, "1-1", "필수예제", 0),
        section(0, 0, "1-1", "필수예제", 1),
    ]);
    let out = c.run();
    let lines: std::collections::BTreeSet<_> = out
        .display
        .lines()
        .filter(|l| l.contains("1-1") && l.contains("필수예제") && l.contains("미확인"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l.contains("관측 시도")));
}
#[test]
fn seed_2_comment_delimiters_cannot_survive_any_display_value() {
    let mut c = Case::new(2);
    c.scope(2, "섹션");
    c.scenario.edition_trees[0].chapters[0].minors[0].title = "가상주제<!--comment-->".into();
    c.scenario.edition_trees[0].chapters[0].minors[0]
        .activities
        .push("추가<!--comment-->활동".into());
    let out = c.run();
    assert!(!out.display.contains("<!--") && !out.display.contains("-->"));
    assert!(
        out.data["minors"][0]["minor"]["title"]
            .as_str()
            .unwrap()
            .contains("<!--")
    );
    let mut c = Case::new(2);
    c.scope(1, "섹션");
    let mut a = section(0, 0, "1-1", "필수예제", 0);
    a.title.push_str(" <!--source--> ");
    a.source_date = Some("<!--date-->".into());
    c.own_sections(vec![a]);
    let out = c.run();
    assert!(out.ask.is_none() && out.stop.is_none());
    assert!(!out.display.contains("<!--") && !out.display.contains("-->"));
    assert!(
        out.data["sections"]["records"][0]["source_title"]
            .as_str()
            .unwrap()
            .contains("<!--")
    );

    let mut c = Case::new(2);
    c.scope(1, "DailyTest");
    c.judgment(
        "DailyTest",
        Some("<!--activity-->"),
        Some("<!--level-->"),
        None,
    );
    c.scenario.teacher_judgments[0].stage = Some("<!--stage-->".into());
    c.scenario.teacher_judgments[0].recorded_at = Some("<!--date-->".into());
    c.scenario.dailytest_frames[0]
        .scope
        .page_label
        .push_str("<!--page-->");
    let out = c.run();
    assert!(out.ask.is_none() && out.stop.is_none());
    assert!(!out.display.contains("<!--") && !out.display.contains("-->"));
    assert!(out.display.contains("＜!--stage--＞"));
    assert!(out.display.contains("＜!--page--＞"));
}
#[test]
fn seed_1_unit_wide_stage_fact_has_one_unit_scoped_line() {
    let mut c = Case::new(1);
    c.scope(2, "전체");
    c.stage("1-1", "clinic_stage", "original_clinic_solving", true);
    let out = c.run();
    assert_eq!(
        out.display
            .lines()
            .filter(|l| l.starts_with("1-1 오답클리닉 풀이: ") && l.contains("완료 판정"))
            .count(),
        1
    );
}
#[test]
fn seed_13_exclusion_without_attempt_scope_is_explicitly_unknown() {
    let mut c = Case::new(13);
    c.scope(1, "클리닉");
    c.own_sections(vec![
        section(0, 0, "1-1", "필수예제", 0),
        section(0, 0, "1-1", "필수예제", 1),
    ]);
    c.scenario.excluded_sections.push(serde_json::from_value(serde_json::json!({"student_key":"1001","edition":c.request.book,"unit":"1-1","activity":"필수예제","state":"active"})).unwrap());
    let out = c.run();
    assert!(
        out.data["clinic"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["activity"] != "필수예제" || r["verdict"] != "excluded")
    );
    assert!(
        out.display
            .lines()
            .any(|l| l.contains("필수예제") && l.contains("제외") && l.contains("미확인"))
    );
}
