use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_parent::render_parent, record_parse::*};
use serde_json::json;

fn catalog() -> Js {
    codec::parse(json!({"books":[{
        "studentRef":"invented-student", "bookRef":"invented-book",
        "label":"가우스 5-1-2", "edition":"invented-edition", "bindingState":"confirmed",
        "units":[{
            "unitRef":"invented-unit-E", "code":"2-1", "title":"가상주제E", "kind":"minor",
            "activities":["필수예제", "원본 오답 클리닉"],
            "sections":[{"sectionRef":"part-A", "label":"PART A"}, {"sectionRef":"part-B", "label":"PART B"}]
        }, {
            "unitRef":"invented-unit-F", "code":"2-2", "title":"가상주제F", "kind":"minor",
            "activities":["유형다지기"], "sections":[]
        }, {
            "unitRef":"invented-deep-unit", "code":"3-2-1", "title":"가상주제G", "kind":"minor",
            "activities":["필수예제"], "sections":[]
        }]
    }]}).to_string().as_bytes()).unwrap()
}
fn entries(field: RecordField, lines: &[&str], catalog: &Js) -> Vec<RecordEntry> {
    let input = lines.iter().map(|s| (*s).into()).collect::<Vec<_>>();
    let ParsedLines::Ready(entries) = parse_lines(field, &input, catalog).unwrap() else {
        panic!("invented teacher lines must be mapped before rendering");
    };
    entries
}

#[test]
fn parent_lines_from_teacher_facts() {
    let catalog = catalog();
    // Independent literal Q05 haiku/ember expectations; no TS execution or
    // renderer-derived vector hashes. These are invented authoring facts.
    for (progress, expected) in [
        (
            "가우스 5-1-2 2-1 필수예제",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제",
        ),
        (
            "가우스 5-1-2 2-1 필수예제 풀었고",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제 풀이",
        ),
    ] {
        let mut input = entries(RecordField::Progress, &[progress], &catalog);
        input.extend(entries(
            RecordField::Homework,
            &["가우스 5-1-2 2-2 유형다지기"],
            &catalog,
        ));
        let before = input.clone();
        let output = render_parent(&input, &catalog).unwrap();
        assert_eq!(
            input, before,
            "raw supplied teacher facts must be preserved"
        );
        assert_eq!(output.progress.as_deref(), Some(expected));
        assert_eq!(
            output.homework.as_deref(),
            Some("- 가우스 5-1-2, 2-2 가상주제F 유형다지기")
        );
        for field in [
            RecordField::LessonMemo,
            RecordField::Attendance,
            RecordField::DailyTestAssessment,
            RecordField::PreviousHomeworkAssessment,
        ] {
            assert!(output[field].is_none());
        }
    }
    // Q06: the supplied replacement operation belongs to the field disposition,
    // not the parent's learning line. The other five fields are not proposed.
    let input = entries(
        RecordField::Homework,
        &["가우스 5-1-2 2-2 유형다지기로 바꿔"],
        &catalog,
    );
    assert_eq!(input[0].qualifiers, ["교체"]);
    let output = render_parent(&input, &catalog).unwrap();
    assert_eq!(
        output.homework.as_deref(),
        Some("- 가우스 5-1-2, 2-2 가상주제F 유형다지기")
    );
    for field in RecordField::ALL {
        if field != RecordField::Homework {
            assert!(output[field].is_none());
        }
    }
    // No supplied entries means no proposed rewrite of legacy strings/blanks.
    assert_eq!(
        render_parent(&[], &catalog).unwrap(),
        RecordFields::from_fn(|_| None)
    );
    let input = entries(
        RecordField::Progress,
        &["가우스 5-1-2 3-2-1 필수예제"],
        &catalog,
    );
    assert_eq!(
        render_parent(&input, &catalog).unwrap().progress.as_deref(),
        Some("- 가우스 5-1-2, 3-2-1 가상주제G 필수예제")
    );
}

#[test]
fn exact_grouping_deadlines_counts_and_exclusions() {
    let catalog = catalog();
    let input = entries(
        RecordField::Homework,
        &[
            "가우스 5-1-2 2-2 유형다지기 2026-11-18까지",
            "가우스 5-1-2 2-1 필수예제",
            "가우스 5-1-2 2-1 필수예제 2026-11-18까지",
            "가우스 5-1-2 2-2 유형다지기 8문항 8문항 3유형 17번 제외 PART A 제외 11월 19일까지",
            "가우스 5-1-2 2-2 유형다지기 8문항 11월 19일까지",
            "가우스 5-1-2 2-2 유형다지기 8문항 11월 19일까지",
        ],
        &catalog,
    );
    let output = render_parent(&input, &catalog).unwrap();
    let value = output.homework.unwrap();
    assert_eq!(
        value,
        concat!(
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제\n",
            "- 2026-11-18까지\n",
            "- 가우스 5-1-2, 2-2 가상주제F 유형다지기\n",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제\n",
            "- 11월 19일까지\n",
            "- 가우스 5-1-2, 2-2 가상주제F 유형다지기 8문항 8문항 3유형 17번 제외 PART A 제외\n",
            "- 가우스 5-1-2, 2-2 가상주제F 유형다지기 8문항\n",
            "- 가우스 5-1-2, 2-2 가상주제F 유형다지기 8문항"
        )
    );
    assert!(value.lines().all(|line| line.starts_with("- ")));
    assert!(!value.contains('\r'));

    let input = entries(
        RecordField::Progress,
        &[
            "가우스 5-1-2 2-1 필수예제 PART A 풀이",
            "가우스 5-1-2 2-1 필수예제 PART B 풀이",
        ],
        &catalog,
    );
    assert_eq!(
        render_parent(&input, &catalog).unwrap().progress.as_deref(),
        Some("- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART A PART B 풀이")
    );

    let mut input = entries(
        RecordField::Progress,
        &[
            "가우스 5-1-2 2-1 필수예제 PART A 8문항",
            "가우스 5-1-2 2-1 필수예제 PART B 8문항",
            "가우스 5-1-2 2-1 원본 오답 클리닉 PART A 제외",
            "가우스 5-1-2 2-1 필수예제 PART A 풀이",
            "가우스 5-1-2 2-1 필수예제 PART B",
        ],
        &catalog,
    );
    input.extend(entries(
        RecordField::Homework,
        &["가우스 5-1-2 2-1 필수예제 PART A 풀이"],
        &catalog,
    ));
    let output = render_parent(&input, &catalog).unwrap();
    assert_eq!(
        output.progress.as_deref(),
        Some(concat!(
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART A 8문항\n",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART B 8문항\n",
            "- 가우스 5-1-2, 2-1 가상주제E 원본 오답 클리닉 PART A 제외\n",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART A 풀이\n",
            "- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART B"
        ))
    );
    assert_eq!(
        output.homework.as_deref(),
        Some("- 가우스 5-1-2, 2-1 가상주제E 필수예제 PART A 풀이")
    );
}

#[test]
fn parent_mapping_requires_same_catalog_facts() {
    let catalog = catalog();
    let input = entries(
        RecordField::Progress,
        &["가우스 5-1-2 2-1 필수예제 PART A"],
        &catalog,
    );
    for mutate in [
        |e: &mut RecordEntry| e.book_ref = "another-book".into(),
        |e: &mut RecordEntry| e.unit_ref = "another-unit".into(),
        |e: &mut RecordEntry| e.unit_code = "2".into(),
        |e: &mut RecordEntry| e.minor_title = "another-topic".into(),
        |e: &mut RecordEntry| e.activity = "unsupported-activity".into(),
        |e: &mut RecordEntry| e.section_refs = vec!["another-part".into()],
        |e: &mut RecordEntry| e.scope = EntryScope::Unit,
        |e: &mut RecordEntry| e.qualifiers.push("풀이\n추가 문장".into()),
        |e: &mut RecordEntry| e.field = RecordField::LessonMemo,
    ] {
        let mut changed = input.clone();
        mutate(&mut changed[0]);
        assert!(render_parent(&changed, &catalog).is_err());
    }
    let mut duplicate = catalog.get("books").array()[0].clone();
    duplicate.set("bookRef", "another-book".into());
    let mut second = input[0].clone();
    second.book_ref = "another-book".into();
    let mut ambiguous = catalog.clone();
    ambiguous.set(
        "books",
        Js::Arr(vec![
            catalog.get("books").array()[0].clone(),
            duplicate.clone(),
        ]),
    );
    assert!(render_parent(&[input[0].clone(), second.clone()], &ambiguous).is_err());
    duplicate.set("label", "별도 가상교재".into());
    duplicate.set("studentRef", "another-invented-student".into());
    ambiguous.set(
        "books",
        Js::Arr(vec![catalog.get("books").array()[0].clone(), duplicate]),
    );
    assert!(render_parent(&[input[0].clone(), second], &ambiguous).is_err());
}
