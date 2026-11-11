use edu_academy_teacher::{
    backend::SourceBackend,
    facts::SeededFacts,
    synthetic::{
        Scenario, SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort, TEACHER_ID,
        load_scenario,
    },
    unit::status,
    unit_data::UnitStatusRequest,
    unit_fixture::{ReadClock, SharedFrameClock, SyntheticDailyTestPort},
    unit_sources::DailyTestReader,
};
use serde_json::json;
type Backend = SourceBackend<SyntheticDayPort, SyntheticPrestudyPort, SyntheticTextbookPort>;
fn scenario() -> Scenario {
    load_scenario(include_str!("fixtures/scenario-v3.json")).unwrap()
}
fn setup(
    s: Scenario,
) -> (
    Backend,
    SeededFacts,
    DailyTestReader<SyntheticDailyTestPort>,
    ReadClock,
) {
    let facts = SeededFacts::from_scenario(&s);
    let clock = SharedFrameClock::new(s.dailytest_frames.clone());
    let reader = DailyTestReader::new(
        SyntheticDailyTestPort::new(s.clone(), clock.clone()),
        clock,
        TEACHER_ID,
    );
    let read = ReadClock {
        read_at: s.unit_read_at.clone().unwrap(),
    };
    (Backend::synthetic(s, &read.read_at), facts, reader, read)
}
fn req(value: serde_json::Value) -> UnitStatusRequest {
    serde_json::from_value(value).unwrap()
}
fn assert_display(value: serde_json::Value, target: &str) {
    let (mut b, f, mut d, c) = setup(scenario());
    let out = status(&mut b, &mut d, &f, &req(value), &c);
    assert!(out.stop.is_none(), "{:?}", out.stop);
    assert_eq!(out.display, target);
}
#[test]
fn q11_enumerates_complete_tree_and_scoped_scores() {
    assert_display(
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
        include_str!("fixtures/display-q11.txt"),
    );
}
#[test]
fn q12_two_teacher_turns_latch_distinct_frames() {
    let (mut b, f, mut d, c) = setup(scenario());
    let request = req(json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"}));
    let old = b.read_at().to_string();
    let first = status(&mut b, &mut d, &f, &request, &c);
    assert_eq!(
        first.display,
        include_str!("fixtures/display-q12-first.txt")
    );
    let second = status(&mut b, &mut d, &f, &request, &c);
    assert_eq!(
        second.display,
        include_str!("fixtures/display-q12-later.txt")
    );
    assert_eq!(b.read_at(), old);
    assert_eq!(d.clock.reads("1002"), 2);
}
#[test]
fn q13_clinic_exclusion_and_independent_stages() {
    assert_display(
        json!({"student":"가상학생A","topic":"클리닉"}),
        include_str!("fixtures/display-q13.txt"),
    );
}
#[test]
fn q14_book_ask_labels_round_trip() {
    let (mut b, f, mut d, c) = setup(scenario());
    let mut request = req(json!({"student":"가상학생B","topic":"클리닉"}));
    let ask = status(&mut b, &mut d, &f, &request, &c);
    assert_eq!(ask.display, include_str!("fixtures/display-q14-ask.txt"));
    assert_eq!(d.clock.reads("1002"), 0);
    request.book = Some("가우스 5-1-1".into());
    let out = status(&mut b, &mut d, &f, &request, &c);
    assert_eq!(out.display, include_str!("fixtures/display-q14-result.txt"));
}
#[test]
fn q15_filtered_source_items_and_native_score() {
    assert_display(
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
        include_str!("fixtures/display-q15.txt"),
    );
}
#[test]
fn q15_bare_unit_retains_all_observed_sections() {
    assert_display(
        json!({"student":"가상학생A","unit":"1-2","topic":"섹션"}),
        include_str!("fixtures/display-q15-bare.txt"),
    );
}
#[test]
fn q16_both_sources_absent_is_qualified() {
    assert_display(
        json!({"student":"가상학생A","unit":"2-3","topic":"섹션"}),
        include_str!("fixtures/display-q16.txt"),
    );
}

fn run(s: Scenario, value: serde_json::Value) -> edu_academy_teacher::envelope::Envelope {
    let (mut b, f, mut d, c) = setup(s);
    status(&mut b, &mut d, &f, &req(value), &c)
}
#[test]
fn exact_selection_replays_both_models() {
    let common = [
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
        json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"}),
        json!({"student":"가상학생A","topic":"클리닉"}),
        json!({"student":"가상학생B","topic":"클리닉"}),
        json!({"student":"가상학생A","unit":"2-3","topic":"섹션"}),
    ];
    for _model in ["haiku", "ember"] {
        for value in common.clone() {
            let out = run(scenario(), value);
            assert!(out.stop.is_none());
        }
    }
    let filtered = run(
        scenario(),
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
    );
    let bare = run(
        scenario(),
        json!({"student":"가상학생A","unit":"1-2","topic":"섹션"}),
    );
    assert_eq!(
        filtered.data["sections"]["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        bare.data["sections"]["records"].as_array().unwrap().len(),
        2
    );
    assert_ne!(bare.display, filtered.display); // M17: scope_as_asked differs; both truthful outputs pass.
}
#[test]
fn native_score_ordinal_date_and_value_encoding_are_lossless() {
    let mut s = scenario();
    let a = &mut s.sections.attempts[0];
    a.source_score = 17.5;
    a.source_date = None;
    a.items[0].ordinal = "17".into();
    a.items[0].label = Some("1002".into());
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
    );
    assert!(out.display.contains("원천 점수: 17.5"));
    assert!(out.display.contains("원문 문항 순번 17: X"));
    assert_eq!(
        out.data["sections"]["records"][0]["source_date"],
        json!(null)
    );
    let mut s = scenario();
    s.edition_trees[0].chapters[0].minors[0].title = "가상|주제\nA".into();
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert!(
        out.display
            .contains(&edu_academy_teacher::render::cell("가상|주제\nA"))
    );
    assert_eq!(out.data["minors"][0]["minor"]["title"], "가상|주제\nA");
}
#[test]
fn tree_range_middle_unit_comprehensive_and_limits() {
    let value = json!({"student":"가상학생B","book":"가우스 5-1-1","unit_from":"4-1","unit_to":"대단원 4 대단원총괄","topic":"클리닉"});
    let out = run(scenario(), value);
    assert!(out.stop.is_none());
    assert_eq!(out.data["minors"].as_array().unwrap().len(), 5);
    let reversed = run(
        scenario(),
        json!({"student":"가상학생A","unit_from":"2-3","unit_to":"1-1","topic":"섹션"}),
    );
    assert_eq!(
        reversed.ask.unwrap().field,
        edu_academy_teacher::envelope::AskField::UnitTo
    );
    let mut s = scenario();
    let minor = &mut s.edition_trees[0].chapters[0].minors[1];
    minor.code = "1-2-3".into();
    let moved = s.edition_trees[0].chapters[0].minors.remove(1);
    s.edition_trees[0].chapters[0]
        .middles
        .push(edu_academy_teacher::unit_data::Middle {
            code: "1-2".into(),
            title: "가상중단원".into(),
            minors: vec![moved],
        });
    for a in &mut s.sections.attempts {
        if a.student_key == "1001" {
            a.title = a.title.replace(" 1-2 ", " 1-2-3 ");
        }
    }
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2-3 필수예제","topic":"섹션"}),
    );
    assert!(out.display.contains("중단원 1-2 가상중단원 소단원 1-2-3"));
    for width in [1, 80] {
        let mut s = scenario();
        let template = s.edition_trees[0].chapters[0].minors[0].clone();
        s.edition_trees[0].chapters.truncate(1);
        s.edition_trees[0].chapters[0].minors = (1..=90)
            .map(|i| {
                let mut m = template.clone();
                m.code = format!("1-{i}");
                m.title = "가".repeat(width);
                m
            })
            .collect();
        let out = run(
            s,
            json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
        );
        assert!(out.stop.is_none(), "{:?}", out.stop);
        assert!(out.display.len() <= 6000);
        assert!(out.data["minors"].as_array().unwrap().len() <= 60);
        let omitted = out.data["first_omitted"].as_str().unwrap();
        assert!(out.display.contains(omitted));
        assert!(out.coverage.unknowns.iter().any(|u| u.contains(omitted)));
    }
}
#[test]
fn seeded_judgments_never_write_or_replace_native_score() {
    let mut s = scenario();
    s.teacher_judgments[0].snapshot = Some("old-attempt".into());
    use edu_academy_teacher::facts::TeacherFacts;
    let before_facts = SeededFacts::from_scenario(&s);
    let before = serde_json::to_value(
        before_facts
            .unit_judgments("1001")
            .into_iter()
            .chain(before_facts.unit_judgments("1002"))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let (mut b, f, mut d, c) = setup(s.clone());
    let r = req(json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}));
    for _ in 0..2 {
        let out = status(&mut b, &mut d, &f, &r, &c);
        assert!(out.display.contains("원천 점수 90"));
        assert!(out.display.contains("미확인(판단 필요)"));
        assert!(out.display.contains("완료 판정"));
        assert!(out.ask.is_none());
    }
    assert_eq!(
        before,
        serde_json::to_value(
            f.unit_judgments("1001")
                .into_iter()
                .chain(f.unit_judgments("1002"))
                .collect::<Vec<_>>()
        )
        .unwrap()
    );
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert_eq!(
        out.data["daily"]["attempts"][0]["score"].as_f64(),
        Some(90.0)
    );
}
#[test]
fn unit_asks_round_trip_and_stops_are_actionable() {
    use edu_academy_teacher::envelope::{AskField, StopCode};
    let mut absent = scenario();
    absent.students.retain(|s| s.key != "1006");
    let out = run(absent, json!({"student":"가상학생F","topic":"DailyTest"}));
    assert_eq!(out.stop.unwrap().code, StopCode::StudentNotFound);
    let out = run(
        scenario(),
        json!({"student":"가상학생A","book":"미등록 판본","topic":"DailyTest"}),
    );
    assert_eq!(out.stop.unwrap().code, StopCode::BookNotFound);
    let mut s = scenario();
    s.edition_trees.clear();
    let out = run(s, json!({"student":"가상학생A","topic":"DailyTest"}));
    assert_eq!(out.stop.unwrap().code, StopCode::EditionTreeUnavailable);
    assert_eq!(out.display.matches('.').count(), 2);
    let mut s = scenario();
    s.edition_trees[0].chapters.clear();
    let out = run(s, json!({"student":"가상학생A","topic":"DailyTest"}));
    assert_eq!(out.stop.unwrap().code, StopCode::UnitScopeUnavailable);
    let out = run(
        scenario(),
        json!({"student":"가상학생A","unit":"invalid","topic":"섹션"}),
    );
    let ask = out.ask.unwrap();
    assert_eq!(ask.field, AskField::Unit);
    for option in ask.options {
        let answer = run(
            scenario(),
            json!({"student":"가상학생A","unit":option.value,"topic":"섹션"}),
        );
        assert!(answer.ask.is_none());
    }
    let out = run(
        scenario(),
        json!({"student":"가상학생A","unit":"1-2","topic":"invalid"}),
    );
    assert_eq!(out.ask.unwrap().field, AskField::Topic);
    for value in [
        json!({"student":"가상학생A","extra":1}),
        json!({"student":2}),
        json!({"student":"가상학생A","unit":null}),
    ] {
        assert!(serde_json::from_value::<UnitStatusRequest>(value).is_err());
    }
}
#[test]
fn same_name_students_are_resolved_without_class_fanout() {
    let mut s = scenario();
    s.students[2].name = s.students[0].name.clone();
    let out = run(
        s.clone(),
        json!({"student":"가상학생A","topic":"DailyTest"}),
    );
    let ask = out.ask.unwrap();
    assert_eq!(ask.options.len(), 2);
    for option in ask.options {
        let out = run(
            s.clone(),
            json!({"student":option.value,"book":if option.label_ko.contains("5-1-1"){"가우스 5-1-1"}else{"가우스 5-1-2"},"topic":"DailyTest"}),
        );
        assert!(out.ask.is_none());
        assert!(out.stop.is_none());
        let key = out.data["student_key"].as_str().unwrap();
        assert!(
            out.data["daily"]["attempts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|a| a["score"].as_f64() == Some(if key == "1001" { 90.0 } else { 70.0 }))
        );
    }
}
#[test]
fn book_suffix_normalization_default_topics_and_unknown_rows() {
    let out = run(
        scenario(),
        json!({"student":"가상학생A","book":"가우스 5-1-2 1-2 필수예제","topic":"섹션"}),
    );
    assert_eq!(out.display, include_str!("fixtures/display-q15.txt"));
    let out = run(scenario(), json!({"student":"가상학생A","unit":"1-2"}));
    assert_eq!(out.data["topic"], "전체");
    assert!(out.data["daily"].is_object());
    assert!(out.data["sections"].is_object());
    assert!(out.data["clinic"].as_array().unwrap().len() > 1);
    assert!(!out.data["prestudy"].as_array().unwrap().is_empty());
    let out = run(
        scenario(),
        json!({"student":"가상학생A","unit":"1-2 필수예제"}),
    );
    assert_eq!(out.data["topic"], "섹션");
    let out = run(
        scenario(),
        json!({"student":"가상학생A","unit":"1-2 DailyTest 클리닉"}),
    );
    assert_eq!(
        out.ask.unwrap().field,
        edu_academy_teacher::envelope::AskField::Topic
    );
    let mut s = scenario();
    s.dailytest_frames[0].attempts.clear();
    s.dailytest_frames[0].headers.clear();
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert!(out.stop.is_none());
    assert!(out.data["daily"]["attempts"].as_array().unwrap().is_empty());
    assert!(out.display.contains("미확인(읽은 기간에 없음)"));
}
#[test]
fn cached_tree_keeps_original_read_time() {
    let mut s = scenario();
    s.edition_trees[0].read_at = "2026-11-10T08:00:00+09:00".into();
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert_eq!(
        out.coverage
            .sources
            .iter()
            .find(|s| s.name == "edition_tree")
            .unwrap()
            .read_at,
        "2026-11-10T08:00:00+09:00"
    );
    assert!(
        out.display
            .contains("원래 읽은 시각 2026-11-10T08:00:00+09:00")
    );
}
#[test]
fn clinic_stage_applicability_matrix() {
    use edu_academy_teacher::{unit::dependent_stage, unit_data::UnitVerdict};
    assert_eq!(
        dependent_stage(Some(false), true),
        UnitVerdict::NotYetExpected
    );
    assert_eq!(
        dependent_stage(None, true),
        UnitVerdict::TeacherReportedNoServer
    );
    assert_eq!(dependent_stage(None, false), UnitVerdict::Unknown);
    let mut s = scenario();
    s.edition_trees[0].family = "supplementary".into();
    let out = run(s, json!({"student":"가상학생A","book":"가우스 5-1-2"}));
    assert!(
        out.data["stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| ["dailytest", "chapter_print", "prestudy"]
                .contains(&s["stage"].as_str().unwrap()))
            .all(|s| s["verdict"] == "not_applicable")
    );
    assert!(out.coverage.sources.iter().all(|s| s.name != "dailytest"));
    let mut s = scenario();
    for i in &mut s.sections.attempts[0].items {
        i.scoring = "O".into();
    }
    let out = run(s, json!({"student":"가상학생A","topic":"클리닉"}));
    assert!(
        out.data["clinic"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["unit"] == "1-2"
                && c["activity"] == "필수예제"
                && c["verdict"] == "not_eligible")
    );
}

#[test]
fn unfinished_prerequisite_and_teacher_stages_remain_independent() {
    let mut s = scenario();
    let base = s.teacher_fact_reports[0].clone();
    let mut f = base.clone();
    f.kind = "original_work".into();
    f.student_key = Some("1001".into());
    f.unit = "1-2".into();
    f.completed = Some(false);
    s.teacher_fact_reports.push(f);
    let out = run(
        s.clone(),
        json!({"student":"가상학생A","unit":"1-2","topic":"클리닉"}),
    );
    assert!(out.display.contains("미확인(아직 확인할 단계 아님)"));
    assert!(
        out.data["stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["activity"] == "필수예제")
            .all(|s| s["verdict"] == "not_yet_expected")
    );
    let mut f = base;
    f.kind = "clinic_stage".into();
    f.student_key = Some("1001".into());
    f.unit = "1-2".into();
    f.stage = "original_clinic_solving".into();
    f.completed = Some(true);
    s.teacher_fact_reports.push(f);
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2","topic":"클리닉"}),
    );
    let stages = out.data["stages"].as_array().unwrap();
    assert_eq!(
        stages
            .iter()
            .find(|s| s["stage"] == "original_clinic_solving")
            .unwrap()["verdict"],
        "confirmed"
    );
    assert_eq!(
        stages
            .iter()
            .find(|s| s["stage"] == "similar_clinic")
            .unwrap()["verdict"],
        "not_yet_expected"
    );
}
#[test]
fn clinic_empty_results_and_structural_values_remain_qualified() {
    let mut s = scenario();
    s.sections.attempts.clear();
    s.sections.index_titles.clear();
    s.sections.grades.clear();
    s.excluded_sections.clear();
    s.teacher_judgments.clear();
    s.edition_trees[0].chapters[0].minors[1].title = "가상|주제\nB".into();
    let out = run(s, json!({"student":"가상학생A","topic":"클리닉"}));
    assert!(out.stop.is_none());
    assert!(
        out.data["clinic"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["verdict"] == "unknown")
    );
    assert!(
        out.display
            .contains(&edu_academy_teacher::render::cell("가상|주제\nB"))
    );
    assert!(!out.display.contains("대상 근거"));
}

#[test]
fn scenario_v3_preserves_previous_roots_and_has_exact_target_bytes() {
    let v2: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scenario-v2.json")).unwrap();
    let v3: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scenario-v3.json")).unwrap();
    for (key, value) in v2.as_object().unwrap() {
        if key == "sections" {
            for (k, v) in value.as_object().unwrap() {
                assert_eq!(&v3[key][k], v);
            }
        } else {
            assert_eq!(&v3[key], value);
        }
    }
    for text in [
        include_str!("fixtures/display-q11.txt"),
        include_str!("fixtures/display-q12-first.txt"),
        include_str!("fixtures/display-q12-later.txt"),
        include_str!("fixtures/display-q13.txt"),
        include_str!("fixtures/display-q14-ask.txt"),
        include_str!("fixtures/display-q14-result.txt"),
        include_str!("fixtures/display-q15.txt"),
        include_str!("fixtures/display-q15-bare.txt"),
        include_str!("fixtures/display-q16.txt"),
    ] {
        assert!(!text.ends_with('\n'));
        assert!(text.len() <= 6000);
    }
}
#[test]
fn unequal_section_sets_use_one_tree_ordered_grid() {
    let mut s = scenario();
    let mut other = s.sections.attempts[0].clone();
    other.paper_key = "8877".into();
    other.attempt_key = "8878".into();
    other.title = other.title.replace("1-2 가상주제B", "2-1 가상주제E");
    s.sections.attempts.push(other);
    s.edition_trees[0].chapters[1].minors[0]
        .activities
        .truncate(1);
    let out = run(
        s,
        json!({"student":"가상학생A","unit_from":"1-2","unit_to":"2-1","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    let rows = out
        .display
        .lines()
        .filter(|line| line.contains(" | "))
        .collect::<Vec<_>>();
    assert!(rows.len() > 2);
    let columns = rows[0].split(" | ").count();
    assert!(rows.iter().all(|r| r.split(" | ").count() == columns));
    let a = out.display.find("1-2 가상주제B |").unwrap();
    let b = out.display.find("2-1 가상주제E |").unwrap();
    assert!(a < b);
}

#[test]
fn section_titles_and_teacher_judgment_values_are_encoded_without_changing_data() {
    let mut s = scenario();
    let original = s.sections.attempts[0]
        .title
        .replace("가상주제B", "가상|주제\nB");
    s.sections.attempts[0].title = original.clone();
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    assert_eq!(out.data["sections"]["records"][0]["source_title"], original);
    assert!(
        out.display
            .contains(&edu_academy_teacher::render::cell(&original))
    );
    assert_eq!(out.data["sections"]["records"][0]["course_key"], "101");
    let mut s = scenario();
    s.teacher_judgments[0].recorded_at = Some("가상|날짜\nA".into());
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(
        out.display
            .contains(&edu_academy_teacher::render::cell("가상|날짜\nA"))
    );
    assert_eq!(out.data["judgments"][0]["recorded_at"], "가상|날짜\nA");
}

#[test]
fn all_topics_keep_native_identifiers_in_metadata_and_supplementary_is_explicit() {
    let out = run(scenario(), json!({"student":"가상학생A","unit":"1-1"}));
    assert!(out.stop.is_none());
    for internal in [
        "9001",
        "paper_search",
        "paper_key",
        "sourceScore",
        "section_index",
    ] {
        assert!(
            !out.display.contains(internal),
            "{internal}: {}",
            out.display
        );
    }
    assert_eq!(out.data["daily"]["attempts"][0]["paper_key"], "9001");
    let mut s = scenario();
    s.edition_trees[0].family = "supplementary".into();
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert!(out.stop.is_none());
    assert!(out.display.contains("해당 없음"));
    assert!(!out.display.contains("미확인(원천을 읽지 못함)"));
    assert!(
        out.coverage
            .sources
            .iter()
            .all(|s| !s.name.starts_with("dailytest"))
    );
}

#[test]
fn all_topic_byte_limit_keeps_footer_and_names_omission() {
    let mut s = scenario();
    let template = s.edition_trees[0].chapters[0].minors[0].clone();
    s.edition_trees[0].chapters.truncate(1);
    s.edition_trees[0].chapters[0].minors = (1..=90)
        .map(|i| {
            let mut m = template.clone();
            m.code = format!("1-{i}");
            m
        })
        .collect();
    let out = run(s, json!({"student":"가상학생A","book":"가우스 5-1-2"}));
    assert!(out.stop.is_none(), "{:?}", out.stop);
    assert!(out.display.len() <= 6000);
    assert!(!out.data["minors"].as_array().unwrap().is_empty());
    assert!(
        out.display
            .ends_with(edu_academy_teacher::display_labels::CLOSING)
    );
    assert!(out.data["first_omitted"].is_string());
    assert!(
        out.coverage
            .sources
            .iter()
            .filter(|s| s.name == "prestudy")
            .count()
            == 1
    );
}

// R1 counterexamples: each assertion fails on the reviewed parent ddaa425.
#[test]
fn r1_b3_unmapped_edition_never_places_a_score_under_a_book() {
    let mut s = scenario();
    s.dailytest_frames[0].headers[0].edition_mapping = None;
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert!(!out.display.contains("원천 점수 90"));
    assert!(out.display.contains("연결 미확인"));
}
#[test]
fn r1_b4_prestudy_duplicate_name_is_qualified_unknown() {
    let mut s = scenario();
    s.students[2].name = s.students[0].name.clone();
    s.students[2].class_name = "가상반B".into();
    s.students[2].bindings = s.students[0].bindings.clone();
    s.prestudy.waiting[0].unit_label = "가우스 5-1-2 1-2 가상주제B".into();
    let out = run(
        s,
        json!({"student":"가상학생A (가상반A)","book":"가우스 5-1-2","unit":"1-2","topic":"예습"}),
    );
    assert!(out.stop.is_none());
    assert!(out.display.contains("같은 이름의 학생"));
    assert!(!out.display.contains("업로드됨"));
    assert!(out.coverage.sources.iter().all(|s| s.name != "prestudy"));
}
#[test]
fn r1_b5_grid_retains_unmatched_level_judgment() {
    let mut s = scenario();
    s.dailytest_frames[0].headers[0].level = Some("기본".into());
    s.teacher_judgments[0].level = Some("발전".into());
    s.teacher_judgments[0].verdict = edu_academy_teacher::unit_data::UnitVerdict::Deferred;
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"DailyTest"}),
    );
    assert!(out.display.contains("1-1 발전 교사 판정: 미확인(판정 보류"));
}
#[test]
fn r1_b6_clinic_includes_comprehensive_activity() {
    let mut s = scenario();
    let mut m = s.edition_trees[0].chapters[0].minors[0].clone();
    m.code = "1-5".into();
    m.title = "가상총괄".into();
    m.activities = vec!["대단원총괄".into()];
    m.unit_wide = true;
    s.edition_trees[0].chapters[0].minors.push(m);
    let mut a = s.sections.attempts[0].clone();
    a.paper_key = "8111".into();
    a.attempt_key = "8211".into();
    a.title = "[대단원총괄] [가우스] 5-1-2 1-5 가상총괄 - 가상학생A".into();
    s.sections.attempts.push(a);
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"클리닉"}),
    );
    assert!(out.display.contains("1-5 대단원총괄 대상 근거"));
    assert!(out.display.lines().nth(1).unwrap().contains("대단원총괄"));
}
#[test]
fn r1_b7_section_activity_header_is_encoded() {
    let mut s = scenario();
    let activity = "필수|예제\nA";
    s.edition_trees[0].chapters[0].minors[1].activities[0] = activity.into();
    s.sections.attempts[0].title = s.sections.attempts[0].title.replace("필수예제", activity);
    let out = run(
        s,
        json!({"student":"가상학생A","unit_from":"1-1","unit_to":"1-2","topic":"섹션"}),
    );
    let header = out.display.lines().nth(1).unwrap();
    assert!(header.contains(&edu_academy_teacher::render::cell(activity)));
    assert!(!out.display.contains(activity));
}
fn unfinished_reports(s: &mut Scenario, count: usize) {
    let base = s.teacher_fact_reports[0].clone();
    for i in 1..=count {
        let mut f = base.clone();
        f.kind = "original_work".into();
        f.unit = format!("1-{i}");
        f.student_key = Some("1001".into());
        f.completed = Some(false);
        s.teacher_fact_reports.push(f);
    }
}
#[test]
fn r1_b8_all_topics_do_not_contradict_scoped_stages() {
    let mut s = scenario();
    unfinished_reports(&mut s, 2);
    let mut f = s.teacher_fact_reports[0].clone();
    f.kind = "clinic_stage".into();
    f.unit = "1-2".into();
    f.stage = "original_clinic_solving".into();
    f.student_key = Some("1001".into());
    f.completed = Some(true);
    s.teacher_fact_reports.push(f);
    let out = run(s, json!({"student":"가상학생A","unit":"1-2"}));
    assert!(
        !out.display
            .lines()
            .any(|line| line == "오답클리닉 풀이: 미확인(교사만 확인 가능)")
    );
    assert!(!out.display.contains("유사클리닉: 미확인(읽은 범위에 없음)"));
    assert!(out.display.contains("완료 판정"));
}
#[test]
fn r1_b9_trimming_includes_exceptional_stages() {
    let mut s = scenario();
    let m = s.edition_trees[0].chapters[0].minors[0].clone();
    s.edition_trees[0].chapters.truncate(1);
    s.edition_trees[0].chapters[0].minors = (1..=60)
        .map(|i| {
            let mut m = m.clone();
            m.code = format!("1-{i}");
            m
        })
        .collect();
    unfinished_reports(&mut s, 60);
    let out = run(s, json!({"student":"가상학생A","book":"가우스 5-1-2"}));
    assert!(out.stop.is_none(), "{:?}", out.stop);
    assert!(out.display.len() <= 6000);
    let retained = out.data["minors"].as_array().unwrap();
    assert!(!retained.is_empty());
    let omitted = out.data["first_omitted"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    assert!(
        !out.display
            .lines()
            .any(|line| line.starts_with(&format!("{omitted} 오답클리닉")))
    );
}
#[test]
fn r1_b10_each_scope_ask_option_repairs_the_original_request() {
    use edu_academy_teacher::envelope::AskField;
    for value in [
        json!({"student":"가상학생A","unit_from":"2-3","unit_to":"1-1","topic":"섹션"}),
        json!({"student":"가상학생A","unit_from":"invalid","unit_to":"1-2","topic":"섹션"}),
        json!({"student":"가상학생A","unit_from":"1-1","topic":"섹션"}),
        json!({"student":"가상학생A","book":"가우스 5-1-2 1-2 필수예제 유형다지기","topic":"섹션"}),
        json!({"student":"가상학생A","unit_from":"1-1 필수예제","unit_to":"1-2 유형다지기","topic":"섹션"}),
    ] {
        let (mut b, f, mut d, c) = setup(scenario());
        let request = req(value);
        let out = status(&mut b, &mut d, &f, &request, &c);
        let ask = out.ask.unwrap();
        for option in ask.options {
            let mut answer = request.clone();
            match ask.field {
                AskField::Unit => answer.unit = Some(option.value),
                AskField::Book => answer.book = Some(option.value),
                AskField::UnitFrom => answer.unit_from = Some(option.value),
                AskField::UnitTo => answer.unit_to = Some(option.value),
                _ => panic!("new scope field must be handled in the repair test"),
            };
            let out = status(&mut b, &mut d, &f, &answer, &c);
            assert!(out.ask.is_none(), "{:?}", out.ask);
            assert!(out.stop.is_none());
        }
    }
}
#[test]
fn r1_b11_second_turn_qualifies_original_registry_and_tree_times() {
    let (mut b, f, mut d, c) = setup(scenario());
    let r = req(json!({"student":"가상학생B","unit":"4-4","topic":"DailyTest"}));
    let _ = status(&mut b, &mut d, &f, &r, &c);
    let out = status(&mut b, &mut d, &f, &r, &c);
    assert!(out.display.contains("10:00"));
    assert!(out.display.contains("09:00"));
}
#[test]
fn r1_b12_unit_join_uses_title_position_and_keeps_unmapped_rows() {
    let mut s = scenario();
    s.sections.attempts[0].title = s.sections.attempts[0]
        .title
        .replace("가상주제B", "가상주제B 선수단원 1-1 복습");
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"섹션"}),
    );
    let record = out.data["sections"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["attempt_key"] == "8200")
        .unwrap();
    assert_eq!(record["unit"], "1-2");
    let mut s = scenario();
    s.sections.attempts[0].title = s.sections.attempts[0]
        .title
        .replace("1-2 가상주제B", "9-9 가상주제B");
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"섹션"}),
    );
    assert!(out.display.contains("단원·활동 연결 미확인"));
    assert!(out.display.contains("9-9"));
}
#[test]
fn r1_b13_course_and_activity_stage_snapshot_are_not_erased() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/scenario-v3.json")).unwrap();
    value["teacher_judgments"][0]["course_key"] = json!("999");
    value["teacher_judgments"][0]["book_no"] = json!("201");
    value["excluded_sections"][0]["course_key"] = json!("999");
    value["excluded_sections"][0]["book_no"] = json!("201");
    let s: Scenario = serde_json::from_value(value).unwrap();
    let out = run(
        s.clone(),
        json!({"student":"가상학생A","unit":"1-1","topic":"DailyTest"}),
    );
    assert!(!out.display.contains("완료 판정"));
    let out = run(s, json!({"student":"가상학생A","topic":"클리닉"}));
    assert!(!out.display.contains("교사가 제외한 섹션"));
    let mut s = scenario();
    let mut a = s.teacher_judgments[0].clone();
    a.topic = "클리닉".into();
    a.unit = "1-2".into();
    a.activity = Some("필수예제".into());
    a.stage = Some("original_clinic_solving".into());
    a.snapshot = Some("8200".into());
    let mut b = a.clone();
    b.activity = Some("유형다지기".into());
    b.stage = Some("original_clinic_grading".into());
    b.snapshot = Some("8201".into());
    s.teacher_judgments = vec![a, b];
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2","topic":"클리닉"}),
    );
    assert!(out.display.contains("1-2 필수예제 오답클리닉 풀이"));
    assert!(out.display.contains("1-2 유형다지기 오답클리닉 채점"));
    assert!(!out.display.contains("8200"));
}

#[test]
fn r1_n1_class_and_missing_student_have_a_defined_stop() {
    for class in ["가상반A", ""] {
        let mut s = scenario();
        s.classes[0].roster.clear();
        let out = run(s, json!({"student":class,"topic":"섹션"}));
        assert_eq!(out.stop.unwrap().code.as_str(), "student_required");
        assert!(out.display.contains("학생 이름"));
    }
    assert!(serde_json::from_value::<UnitStatusRequest>(json!({"class_name":"가상반A"})).is_err());
    let out = run(scenario(), json!({"topic":"섹션"}));
    assert_eq!(out.stop.unwrap().code.as_str(), "student_required");
}
#[test]
fn r1_prestudy_reads_once_for_all_selected_leaves() {
    let (mut b, f, mut d, c) = setup(scenario());
    let out = status(
        &mut b,
        &mut d,
        &f,
        &req(json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"예습"})),
        &c,
    );
    assert!(out.stop.is_none());
    assert_eq!(b.prestudy_port_calls(), 5);
    assert_eq!(
        out.coverage
            .sources
            .iter()
            .filter(|s| s.name == "prestudy")
            .count(),
        1
    );
    assert_eq!(out.data["prestudy"].as_array().unwrap().len(), 7);
}

#[test]
fn r1_b13_exclusion_is_limited_to_its_course_and_attempt() {
    let mut s = scenario();
    let mut a = s.sections.attempts[0].clone();
    a.paper_key = "8333".into();
    a.attempt_key = "8444".into();
    a.title = a.title.replace("가상주제B", "가상주제B 재시험");
    for i in &mut a.items {
        if i.scoring == "X" {
            i.label = Some("1009".into());
        }
    }
    s.sections.attempts.push(a);
    s.excluded_sections[0].unit = "1-2".into();
    s.excluded_sections[0].activity = "필수예제".into();
    s.excluded_sections[0].snapshot = Some("8200".into());
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2 필수예제","topic":"클리닉"}),
    );
    assert!(out.stop.is_none());
    let cell = out.data["clinic"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["activity"] == "필수예제")
        .unwrap();
    assert_eq!(cell["verdict"], "eligible");
    assert!(
        cell["labels"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l == "1009")
    );
    assert!(out.display.contains("관측 시도"));
    assert!(!out.display.contains("8200"));
}
#[test]
fn r1_many_unmapped_observations_are_preserved_with_a_bounded_notice() {
    let mut s = scenario();
    let a = s.sections.attempts[0].clone();
    s.sections.attempts = (1..=50)
        .map(|i| {
            let mut a = a.clone();
            a.paper_key = format!("{}", 8500 + i);
            a.attempt_key = format!("{}", 8600 + i);
            a.title = a
                .title
                .replace("1-2 가상주제B", &format!("9-{i} {}", "가상주제".repeat(30)));
            a
        })
        .collect();
    let out = run(
        s,
        json!({"student":"가상학생A","book":"가우스 5-1-2","topic":"섹션"}),
    );
    assert!(out.stop.is_none());
    assert!(out.display.len() <= 6000);
    assert_eq!(
        out.data["sections"]["observations"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    assert!(out.display.contains("제목 50개"));
    assert!(out.display.contains("표시 한도"));
}

#[test]
fn r1_b13_stage_reports_keep_each_observed_attempt_scope() {
    let mut s = scenario();
    let mut second = s.sections.attempts[0].clone();
    second.attempt_key = "8299".into();
    second.paper_key = "8399".into();
    second.title.push_str(" 재시험");
    s.sections.attempts.push(second);
    let mut f = s.teacher_fact_reports[0].clone();
    f.kind = "clinic_stage".into();
    f.student_key = Some("1001".into());
    f.unit = "1-2".into();
    f.stage = "original_clinic_solving".into();
    f.completed = Some(true);
    f.snapshot = Some("8200".into());
    let mut second = f.clone();
    second.snapshot = Some("8299".into());
    second.completed = Some(false);
    second.reported_on = "2026-11-12".into();
    s.teacher_fact_reports.extend([f, second]);
    let out = run(
        s,
        json!({"student":"가상학생A","unit":"1-2","topic":"클리닉"}),
    );
    let stages = out.data["stages"].as_array().unwrap();
    assert!(
        stages
            .iter()
            .any(|s| s["snapshot"] == "8200" && s["verdict"] == "confirmed")
    );
    assert!(
        stages
            .iter()
            .any(|s| s["snapshot"] == "8299" && s["stage"] == "original_clinic_solving")
    );
    assert!(out.display.contains("관측 시도"));
    assert!(!out.display.contains("8299"));
}
