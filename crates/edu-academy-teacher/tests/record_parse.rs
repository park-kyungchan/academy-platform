use edu_academy_core::{js::Js, workflow::codec};
use edu_academy_teacher::{record_data::*, record_parse::*};
use serde_json::{Value, json};

fn catalog() -> Value {
    json!({"books":[{
        "studentRef":"invented-student", "bookRef":"invented-book",
        "label":"가상교재 5-1-2", "edition":"판본A", "bindingState":"confirmed",
        "units":[{
            "unitRef":"invented-unit", "code":"2-1-3", "title":"가상주제A",
            "kind":"minor", "activities":["필수예제", "유형다지기", "원본 오답 클리닉"],
            "sections":[{"sectionRef":"invented-part", "label":"PART A"}]
        }]
    }]})
}
fn parse(field: RecordField, lines: &[&str], catalog: &Value) -> ParsedLines {
    parse_lines(
        field,
        &lines.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        &codec::parse(catalog.to_string().as_bytes()).unwrap(),
    )
    .unwrap()
}
fn ready(result: ParsedLines) -> Vec<RecordEntry> {
    match result {
        ParsedLines::Ready(entries) => entries,
        ParsedLines::Ask(_) => panic!("expected fully mapped teacher lines"),
    }
}
fn ask(result: ParsedLines, field: AnswerField) -> Vec<PendingQuestion> {
    match result {
        ParsedLines::Ask(questions) => {
            assert!(!questions.is_empty());
            assert!(questions.iter().all(|q| q.field == field));
            questions
        }
        ParsedLines::Ready(_) => panic!("mapping uncertainty must ask"),
    }
}

#[test]
fn teacher_lines_preserve_supplied_words() {
    let c = catalog();
    let line = "- 가상교재 5-1-2, 2-1-3 가상주제A 필수예제 PART A solving 12 문항 12 문항 3유형 17번 제외 11월 15일까지";
    let entries = ready(parse(RecordField::Progress, &[line], &c));
    let entry = &entries[0];
    assert_eq!(entry.raw_line, line);
    assert_eq!(entry.field, RecordField::Progress);
    assert_eq!(entry.book_ref, "invented-book");
    assert_eq!(entry.unit_ref, "invented-unit");
    assert_eq!(entry.unit_code, "2-1-3");
    assert_eq!(entry.minor_title, "가상주제A");
    assert_eq!(entry.activity, "필수예제");
    assert_eq!(
        entry.qualifiers,
        [
            "PART A",
            "풀이",
            "12 문항",
            "12 문항",
            "3유형",
            "17번 제외",
            "11월 15일까지"
        ]
    );
    assert_eq!(entry.scope, EntryScope::Sections);
    assert_eq!(entry.section_refs, ["invented-part"]);
    assert_eq!(entry.deadline.as_deref(), Some("11월 15일"));

    let lines = [
        "가상교재 5-1-2 2-1-3 유형다지기로 바꿔",
        "가상교재 5-1-2 2-1-3 원본 오답 클리닉 PART A 제외 2026-11-15까지",
    ];
    let entries = ready(parse(RecordField::Homework, &lines, &c));
    assert_eq!(entries[0].activity, "유형다지기");
    assert_eq!(entries[0].qualifiers, ["교체"]);
    assert_eq!(entries[1].activity, "원본 오답 클리닉");
    assert_eq!(entries[1].qualifiers, ["PART A 제외", "2026-11-15까지"]);
    assert!(
        entries[1].section_refs.is_empty(),
        "excluded PART is not positive scope"
    );

    for (field, answer, label) in [
        (RecordField::Progress, AnswerField::Progress, "진도"),
        (RecordField::Homework, AnswerField::Homework, "숙제"),
    ] {
        for words in [
            "좀",
            "마쳤거든",
            "알아서",
            "풀이했음",
            "2 문항까지",
            "PART Z",
        ] {
            let line = format!("가상교재 5-1-2 2-1-3 필수예제 {words}");
            let questions = ask(parse(field, &[&line], &c), answer);
            assert_eq!(questions[0].reason_code, "record_teacher_words_unmapped");
            assert_eq!(
                questions[0].text,
                format!(
                    "{label}의 “{words}”는 어떤 뜻으로 기록할까요? 해당 항목의 문장을 그대로 답해 주세요."
                )
            );
            assert!(questions[0].choices.is_empty());
        }
    }
    // An unresolved line prevents a partially ready field from being returned.
    ask(
        parse(
            RecordField::Progress,
            &[line, "가상교재 5-1-2 2-1-3 필수예제 좀"],
            &c,
        ),
        AnswerField::Progress,
    );
}

#[test]
fn exact_book_unit_or_question() {
    let mut c = catalog();
    let mut shorter = c["books"][0].clone();
    shorter["label"] = json!("가상교재");
    shorter["bookRef"] = json!("short-book");
    c["books"].as_array_mut().unwrap().push(shorter);
    let line = "가상교재 5-1-2 2-1-3 필수예제 풀이 8문항";
    let entries = ready(parse(RecordField::Progress, &[line], &c));
    assert_eq!(
        entries[0].book_ref, "invented-book",
        "longest exact label wins"
    );
    for bad in [
        "가상교재 5-1-22 2-1-3 필수예제",
        "가상교재 5-1-2 2-1 필수예제",
        "가상교재 5-1-2 2-1-30 필수예제",
        "가상교재 5-1-2 2-1-3 다른제목 필수예제",
    ] {
        ask(
            parse(RecordField::Progress, &[bad], &c),
            AnswerField::Progress,
        );
    }
    let titled = ready(parse(
        RecordField::Homework,
        &["가상교재 5-1-2 가상주제A 유형다지기"],
        &c,
    ));
    assert_eq!(titled[0].unit_code, "2-1-3");

    let mut second = c["books"][0].clone();
    second["bookRef"] = json!("second-book");
    second["edition"] = json!("판본B");
    c["books"].as_array_mut().unwrap().push(second);
    let untouched = "가상교재 2-1-3 유형다지기";
    let questions = ask(
        parse(RecordField::Progress, &[line, untouched], &c),
        AnswerField::Progress,
    );
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].choices.len(), 2);
    for choice in &questions[0].choices {
        let AnswerValue::Lines(replacement) = &choice.answer else {
            panic!("whole field lines required");
        };
        assert_eq!(replacement[1], untouched);
        assert!(replacement[0].ends_with("2-1-3 필수예제 풀이 8문항"));
        let js = codec::parse(c.to_string().as_bytes()).unwrap();
        let chosen = ready(parse_lines(RecordField::Progress, replacement, &js).unwrap());
        assert_eq!(chosen[0].qualifiers, ["풀이", "8문항"]);
    }
    let mut ambiguous_unit = catalog();
    let mut second = ambiguous_unit["books"][0]["units"][0].clone();
    second["unitRef"] = json!("second-unit");
    second["title"] = json!("가상주제B");
    ambiguous_unit["books"][0]["units"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let questions = ask(
        parse(RecordField::Homework, &[line], &ambiguous_unit),
        AnswerField::Homework,
    );
    assert_eq!(questions[0].choices.len(), 2);
    for choice in &questions[0].choices {
        let AnswerValue::Lines(replacement) = &choice.answer else {
            panic!("lines required");
        };
        let js = codec::parse(ambiguous_unit.to_string().as_bytes()).unwrap();
        assert_eq!(
            ready(parse_lines(RecordField::Homework, replacement, &js).unwrap()).len(),
            1
        );
    }
    let mut unresolved = catalog();
    unresolved["books"][0]["bindingState"] = json!("unresolved");
    assert!(
        ask(
            parse(RecordField::Progress, &[line], &unresolved),
            AnswerField::Progress
        )[0]
        .choices
        .is_empty()
    );
    let mut mixed = catalog();
    let mut foreign = mixed["books"][0].clone();
    foreign["studentRef"] = json!("another-invented-student");
    mixed["books"].as_array_mut().unwrap().push(foreign);
    let js = codec::parse(mixed.to_string().as_bytes()).unwrap();
    assert!(parse_lines(RecordField::Progress, &[line.into()], &js).is_err());
    ask(
        parse(RecordField::Progress, &[line], &json!({"books":[]})),
        AnswerField::Progress,
    );
}

#[test]
fn no_inferred_stage_or_count() {
    let c = catalog();
    let simple = "가상교재 5-1-2 2-1-3 필수예제";
    let plain = ready(parse(RecordField::Progress, &[simple], &c));
    assert!(plain[0].qualifiers.is_empty());
    assert!(plain[0].deadline.is_none());
    assert_eq!(plain[0].scope, EntryScope::Unit);
    for stage in ["solving", "풀이", "풀었고"] {
        let line = format!("{simple} {stage}");
        assert_eq!(
            ready(parse(RecordField::Progress, &[&line], &c))[0].qualifiers,
            ["풀이"]
        );
    }
    let ordinal = ready(parse(
        RecordField::Homework,
        &[&format!("{simple} 8유형")],
        &c,
    ));
    assert_eq!(ordinal[0].qualifiers, ["8유형"]);
    let unchanged = ready(parse(
        RecordField::Homework,
        &[&format!("{simple} 12345678901234567890문항")],
        &c,
    ));
    assert_eq!(unchanged[0].qualifiers, ["12345678901234567890문항"]);
    let dates = format!("{simple} 2026-11-15까지 2026-11-16까지");
    assert!(
        ask(
            parse(RecordField::Homework, &[&dates], &c),
            AnswerField::Homework
        )[0]
        .text
        .contains("2026-11-16까지")
    );
    assert!(parse_lines(RecordField::LessonMemo, &[], &Js::Null).is_err());
}
