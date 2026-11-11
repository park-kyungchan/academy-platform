//! E10a-2 synthetic tests for the textbook section index, merge, title
//! pattern and grading source layer. Invented names, keys, titles and pages
//! only; fake ports; no network.
use edu_academy_sources::{
    data::{Error, Result},
    http,
    student_course::read_contracts,
    textbook::{FetchPolicy, TextbookResponse},
    textbook_index::*,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const PRINCIPAL: &str = "Fictional Teacher";
const KEY: &str = "77001";
const NAME: &str = "Invented Learner";
const WRONG_TITLE: &str = "[필수예제] [가우스] 5-3 진분수의 뺄셈 - Invented Learner";
const PERFECT: &str = "[필수예제] [가우스] 5-4 대분수의 뺄셈 - Invented Learner";
const OTHER_BOOK: &str = "[필수예제] [다른책] 5-4 대분수의 뺄셈 - Invented Learner";
const OTHER_ACTIVITY: &str = "[유형다지기] [가우스] 5-4 대분수의 뺄셈 - Invented Learner";
const PROGRESS_SINGLE: &str = include_str!("fixtures/textbook_index/progress-single.html");
const PROGRESS_TWO: &str = include_str!("fixtures/textbook_index/progress-two-books.html");
const NOTES: &str = include_str!("fixtures/textbook_index/wrong-notes.json");
const DETAIL_82003: &str = include_str!("fixtures/textbook_index/detail-82003.json");
const DETAIL_82004: &str = include_str!("fixtures/textbook_index/detail-82004.json");

type Log = Rc<RefCell<Vec<String>>>;
type Seen = Rc<RefCell<Vec<(String, usize, FetchPolicy)>>>;
type Tweak = Box<dyn Fn(&str, TextbookResponse) -> Result<TextbookResponse>>;

fn code(error: Error) -> String {
    error.message
}
fn resp(body: &str) -> TextbookResponse {
    TextbookResponse {
        status: 200,
        headers: vec![],
        bytes: body.as_bytes().to_vec(),
        final_url: None,
        redirected: false,
        credentials_sent: true,
        redirect_followed: false,
    }
}
fn student() -> TextbookStudent {
    TextbookStudent::new(KEY, NAME).unwrap()
}
fn scope(course: &str, book: &str) -> BookScope {
    BookScope::new(course, book).unwrap()
}
fn titles(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_string()).collect()
}
fn graded(outcome: GradeOutcome) -> GradedSections {
    match outcome {
        GradeOutcome::Graded(g) => g,
        other => panic!("expected graded, got {other:?}"),
    }
}

type PaperRowSpec = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);
const PAPER_ROWS: [PaperRowSpec; 5] = [
    ("81003", "82003", WRONG_TITLE, Some("2026-11-03 10:00:00")),
    ("81004", "82004", PERFECT, Some("2026-11-03 10:05:00")),
    ("81005", "82005", OTHER_BOOK, Some("2026-11-03 10:06:00")),
    (
        "81006",
        "82006",
        OTHER_ACTIVITY,
        Some("2026-11-03 10:07:00"),
    ),
    ("81007", "82007", PERFECT, None),
];
/// One observed paper-search result page (rows of any paper type).
fn paper_page(rows: &[PaperRowSpec]) -> String {
    let list: Vec<Value> = rows
        .iter()
        .map(|(paper, attempt, title, date)| {
            json!({"priNo": KEY, "userName": NAME, "pNo": paper, "testingNo": attempt,
                "score": 100, "_id": format!("r{attempt}"), "testingName": title, "applyDate": date})
        })
        .collect();
    let raw = Value::Array(list)
        .to_string()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    format!(
        "<form name=userForm><input name=pri_no value=99001></form><script>const results={{testResultList:JSON.parse('{raw}')}};</script>"
    )
}
fn notes_json(rows: &[(u64, u64, &str)]) -> String {
    let list: Vec<Value> = rows
        .iter()
        .map(|(paper, attempt, title)| {
            json!({"pri_no": 77001, "name": NAME, "p_no": paper, "testing_no": attempt,
                "testing_name": title, "apply_date": "11-03"})
        })
        .collect();
    json!({"result": "ok", "noteResultList": list}).to_string()
}
fn detail_json(title: &str, score: Value, items: Value) -> String {
    json!({"result": "OK", "print_list": [{"pri_no": 77001, "student_name": NAME,
        "sub_title": title, "score": score, "exam_list": items}]})
    .to_string()
}

struct Fake {
    progress: String,
    notes: HashMap<(String, String), String>,
    details: HashMap<String, String>,
    paper: Option<String>,
    log: Log,
    seen: Seen,
    tweak: Option<Tweak>,
}
impl Fake {
    fn single() -> Self {
        let mut notes = HashMap::new();
        notes.insert(("693".to_string(), "2517".to_string()), NOTES.to_string());
        let mut details = HashMap::new();
        details.insert("82003".to_string(), DETAIL_82003.to_string());
        details.insert("82004".to_string(), DETAIL_82004.to_string());
        Self {
            progress: PROGRESS_SINGLE.to_string(),
            notes,
            details,
            paper: Some(paper_page(&PAPER_ROWS[..4])),
            log: Rc::new(RefCell::new(vec![])),
            seen: Rc::new(RefCell::new(vec![])),
            tweak: None,
        }
    }
    fn finish(&self, kind: &str, body: String) -> Result<TextbookResponse> {
        let response = resp(&body);
        match &self.tweak {
            Some(t) => t(kind, response),
            None => Ok(response),
        }
    }
    fn note(&self, kind: &str, max: usize, policy: FetchPolicy, entry: String) {
        self.seen.borrow_mut().push((kind.into(), max, policy));
        self.log.borrow_mut().push(entry);
    }
}
impl TextbookSectionPort for Fake {
    fn progress(
        &mut self,
        student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        assert_eq!(student.student_key(), KEY);
        self.note("progress", max_bytes, policy, "progress".into());
        self.finish("progress", self.progress.clone())
    }
    fn wrong_notes(
        &mut self,
        _student: &TextbookStudent,
        scope: &BookScope,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.note(
            "notes",
            max_bytes,
            policy,
            format!("notes:{}/{}", scope.course_key(), scope.book_no()),
        );
        let body = self
            .notes
            .get(&(scope.course_key().to_string(), scope.book_no().to_string()))
            .cloned()
            .ok_or_else(|| Error::refusal("fake_notes_missing"))?;
        self.finish("notes", body)
    }
    fn detail(
        &mut self,
        _student: &TextbookStudent,
        attempt_key: &str,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.note("detail", max_bytes, policy, format!("detail:{attempt_key}"));
        let body = self
            .details
            .get(attempt_key)
            .cloned()
            .ok_or_else(|| Error::refusal("fake_detail_missing"))?;
        self.finish("detail", body)
    }
    fn paper_search(
        &mut self,
        _student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.note("paper", max_bytes, policy, "paper".into());
        match self.paper.clone() {
            Some(page) => self.finish("paper", page),
            None => Err(Error::refusal("fake_paper_unavailable")),
        }
    }
}
fn source(fake: Fake) -> TextbookIndexSource<Fake> {
    TextbookIndexSource::new(fake, PRINCIPAL)
}
fn run_grade(fake: Fake, wanted: &[&str]) -> Result<GradeOutcome> {
    source(fake).grade(
        PRINCIPAL,
        &student(),
        &scope("693", "2517"),
        &titles(wanted),
    )
}
fn run_index(fake: Fake) -> Result<TextbookIndex> {
    source(fake).index(PRINCIPAL, &student(), &scope("693", "2517"))
}

// ------------------------------------------------ pure TS vector ports

/// Ports textbookSectionIndex.test.ts:39 (pattern anchors on the book token
/// and the student suffix) plus the JavaScript whitespace between brackets.
#[test]
fn title_pattern_anchors_on_book_token_and_student_suffix() {
    let pattern = derive_title_pattern(&titles(&[WRONG_TITLE]), NAME).unwrap();
    assert_eq!(
        pattern.pairs,
        [("필수예제".to_string(), "가우스".to_string())]
    );
    assert_eq!(
        pattern.student_suffix.as_deref(),
        Some(" - Invented Learner")
    );
    assert!(is_section_paper_title(PERFECT, &pattern));
    assert!(!is_section_paper_title(OTHER_BOOK, &pattern));
    assert!(!is_section_paper_title(OTHER_ACTIVITY, &pattern));
    assert!(!is_section_paper_title(
        "[필수예제] [가우스] 5-4 대분수의 뺄셈 - Other Learner",
        &pattern
    ));
    assert!(derive_title_pattern(&[], NAME).is_none());
    assert!(derive_title_pattern(&titles(&["No brackets here"]), NAME).is_none());
    // A bare template echo is not a section; the suffix only counts when every
    // wrong-note title carries it.
    assert!(!is_section_paper_title(
        "[필수예제] [가우스] - Invented Learner",
        &pattern
    ));
    assert!(!is_section_paper_title("[필수예제] [가우스]", &pattern));
    let no_suffix =
        derive_title_pattern(&titles(&[WRONG_TITLE, "[필수예제] [가우스] 5-5 x"]), NAME).unwrap();
    assert_eq!(no_suffix.student_suffix, None);
    // JavaScript whitespace (NBSP, ideographic space) separates bracket groups.
    let spaced = derive_title_pattern(
        &titles(&["[필수예제]\u{00A0}[가우스]\u{3000}5-3 x - Invented Learner"]),
        NAME,
    )
    .unwrap();
    assert_eq!(spaced.pairs.len(), 1);
    // U+0085 is not JavaScript whitespace: the second group is not reached.
    assert!(
        derive_title_pattern(
            &titles(&["[필수예제]\u{0085}[가우스] x - Invented Learner"]),
            NAME
        )
        .is_none()
    );
}

/// Ports textbookSectionIndex.test.ts:50 (an activity/book pair never
/// observed together does not join, even when both tokens appeared apart).
#[test]
fn pair_never_observed_together_does_not_join() {
    let a = "[필수예제] [BookA] 1-1 x - Invented Learner";
    let b = "[유형다지기] [BookB] 1-1 x - Invented Learner";
    let pattern = derive_title_pattern(&titles(&[a, b]), NAME).unwrap();
    assert!(!is_section_paper_title(
        "[필수예제] [BookB] 9-9 z - Invented Learner",
        &pattern
    ));
    assert!(is_section_paper_title(
        "[유형다지기] [BookB] 9-9 z - Invented Learner",
        &pattern
    ));
    let wrong = [WrongNoteRow {
        source_title: a.into(),
        paper_key: "1".into(),
        attempt_key: "1".into(),
        source_date_literal: None,
    }];
    let paper = [PaperSearchRow {
        testing_name: "[필수예제] [BookB] 9-9 z - Invented Learner".into(),
        paper_key: "2".into(),
        attempt_key: "2".into(),
        apply_date_literal: None,
    }];
    assert_eq!(
        merge_section_rows(&wrong, &paper, Some(&pattern))
            .unwrap()
            .len(),
        1
    );
}

fn wrong_row() -> WrongNoteRow {
    WrongNoteRow {
        source_title: WRONG_TITLE.into(),
        paper_key: "81003".into(),
        attempt_key: "82003".into(),
        source_date_literal: Some("11-03".into()),
    }
}
fn paper_row(spec: &PaperRowSpec) -> PaperSearchRow {
    PaperSearchRow {
        testing_name: spec.2.into(),
        paper_key: spec.0.into(),
        attempt_key: spec.1.into(),
        apply_date_literal: spec.3.map(str::to_string),
    }
}

/// Ports textbookSectionIndex.test.ts:60 (merge keeps wrong-note rows, adds
/// all-correct paper rows, refuses an attempt/title conflict).
#[test]
fn merge_keeps_wrong_rows_adds_all_correct_and_refuses_conflict() {
    let pattern = derive_title_pattern(&titles(&[WRONG_TITLE]), NAME).unwrap();
    let four: Vec<PaperSearchRow> = PAPER_ROWS[..4].iter().map(paper_row).collect();
    let merged = merge_section_rows(&[wrong_row()], &four, Some(&pattern)).unwrap();
    assert_eq!(
        merged
            .iter()
            .map(|r| r.source_title.as_str())
            .collect::<Vec<_>>(),
        [WRONG_TITLE, PERFECT]
    );
    assert_eq!(merged[0].attempt_key, "82003");
    assert_eq!(
        merged[0].sources,
        [
            IndexSource::WrongAnswerNote,
            IndexSource::StudentPaperSearch
        ]
    );
    assert_eq!(merged[1].attempt_key, "82004");
    assert_eq!(merged[1].paper_key, "81004");
    assert_eq!(merged[1].sources, [IndexSource::StudentPaperSearch]);
    assert_eq!(
        merged[1].source_date_literal.as_deref(),
        Some("2026-11-03 10:05:00")
    );
    // The same title on a second attempt stays a separate row.
    let five: Vec<PaperSearchRow> = PAPER_ROWS.iter().map(paper_row).collect();
    let all = merge_section_rows(&[wrong_row()], &five, Some(&pattern)).unwrap();
    assert_eq!(all.iter().filter(|r| r.source_title == PERFECT).count(), 2);
    let conflict = PaperSearchRow {
        testing_name: PERFECT.into(),
        paper_key: "81003".into(),
        attempt_key: "82003".into(),
        apply_date_literal: None,
    };
    assert_eq!(
        code(merge_section_rows(&[wrong_row()], &[conflict], Some(&pattern)).unwrap_err()),
        "textbook_section_title_conflict_for_attempt"
    );
    // Without a derivable pattern paper rows never join a book.
    assert_eq!(
        merge_section_rows(&[wrong_row()], &five, None)
            .unwrap()
            .len(),
        1
    );
}

// ------------------------------------------- grading by key (R07-2/R07-3)

/// Ports textbookSectionIndex.test.ts:107 (the 100-point section is found
/// through the paper search; the wrong-note section still grades) and the
/// source order: progress, wrong note, paper search, then one detail read per
/// requested title in request order.
#[test]
fn grading_finds_the_100_point_section_through_paper_search() {
    let fake = Fake::single();
    let log = fake.log.clone();
    let outcome = run_grade(fake, &[PERFECT, WRONG_TITLE]).unwrap();
    let g = graded(outcome);
    assert_eq!(
        *log.borrow(),
        [
            "progress",
            "notes:693/2517",
            "paper",
            "detail:82004",
            "detail:82003"
        ]
    );
    // Sorted by title: "5-3" before "5-4" although "5-4" was requested first.
    assert_eq!(g.records.len(), 2);
    assert_eq!(g.records[0].source_title, WRONG_TITLE);
    assert_eq!(g.records[1].source_title, PERFECT);
    let perfect = &g.records[1];
    assert_eq!(
        (
            perfect.score,
            perfect.attempt_key.as_str(),
            perfect.paper_key.as_str()
        ),
        (100.0, "82004", "81004")
    );
    assert_eq!(
        perfect
            .items
            .iter()
            .map(|i| i.scoring.as_str())
            .collect::<Vec<_>>(),
        ["O", "O"]
    );
    assert_eq!((perfect.correct_count(), perfect.item_count()), (2, 2));
    assert_eq!(g.records[0].score, 84.0);
    assert_eq!(g.records[0].attempt_key, "82003");
    assert_eq!(g.records[0].source_date.literal.as_deref(), Some("11-03"));
    assert_eq!(g.records[0].source_date.precision, "month_day");
    assert_eq!(g.records[1].source_date.precision, "unknown");
    assert_eq!(g.records[0].course_key, "693");
    assert_eq!(g.records[0].book_no, "2517");
    assert!(g.paper_search_observed);
    assert_eq!(g.unknown_state, UNKNOWN_STATE_PAPER_SEARCH_OBSERVED);
    assert_eq!(g.coverage, "exact_selected_existing_records_not_whole_book");
}

/// Ports textbook-sections.test.ts / textbookSectionIndex.test.ts:121: a
/// title absent from both sources is a typed outcome; an unavailable paper
/// search never licenses that claim; without a paper search the wrong-note
/// section still grades.
#[test]
fn absent_title_is_distinct_and_unavailable_search_never_licenses_it() {
    let missing = "[필수예제] [가우스] 9-9 없는 단원 - Invented Learner";
    match run_grade(Fake::single(), &[missing]).unwrap() {
        GradeOutcome::NotObservedInEither(n) => assert_eq!(n.source_title, missing),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        NOT_IN_EITHER_CODE,
        "textbook_section_not_in_either_observed_source"
    );
    let mut unavailable = Fake::single();
    unavailable.paper = None;
    assert_eq!(
        code(run_grade(unavailable, &[missing]).unwrap_err()),
        "source_selection_missing_or_ambiguous"
    );
    let mut wrong_only = Fake::single();
    wrong_only.paper = None;
    let g = graded(run_grade(wrong_only, &[WRONG_TITLE]).unwrap());
    assert_eq!(g.records[0].score, 84.0);
    assert!(!g.paper_search_observed);
    assert_eq!(g.unknown_state, UNKNOWN_STATE_PAPER_SEARCH_UNAVAILABLE);
    // A paper-search page that fails to parse also degrades.
    let mut broken = Fake::single();
    broken.paper = Some("<script>const results={testResultList:JSON.parse('{')};</script>".into());
    assert_eq!(
        code(run_grade(broken, &[missing]).unwrap_err()),
        "source_selection_missing_or_ambiguous"
    );
}

/// Ports textbookSectionIndex.test.ts:131: without a derivable title pattern
/// absence is selection ambiguity, never the not-in-either claim, and no
/// detail is read.
#[test]
fn without_a_derivable_pattern_absence_is_ambiguity() {
    let mut fake = Fake::single();
    fake.notes.insert(
        ("693".into(), "2517".into()),
        notes_json(&[(81003, 82003, "Invented existing section")]),
    );
    fake.paper = Some(paper_page(&[(
        "81004",
        "82004",
        "Missing exact title",
        None,
    )]));
    let log = fake.log.clone();
    assert_eq!(
        code(run_grade(fake, &["Missing exact title"]).unwrap_err()),
        "source_selection_missing_or_ambiguous"
    );
    // The paper search is still asked for in a grading read (TS hook).
    assert_eq!(*log.borrow(), ["progress", "notes:693/2517", "paper"]);
}

/// A present title is graded before a later absent title is reported, and an
/// absent first title reads no detail (TS reads in request order).
#[test]
fn absent_title_position_decides_which_details_were_read() {
    let missing = "[필수예제] [가우스] 9-9 없는 단원 - Invented Learner";
    let fake = Fake::single();
    let log = fake.log.clone();
    assert!(matches!(
        run_grade(fake, &[WRONG_TITLE, missing]).unwrap(),
        GradeOutcome::NotObservedInEither(_)
    ));
    assert_eq!(log.borrow().last().unwrap(), "detail:82003");
    let fake = Fake::single();
    let log = fake.log.clone();
    assert!(matches!(
        run_grade(fake, &[missing, WRONG_TITLE]).unwrap(),
        GradeOutcome::NotObservedInEither(_)
    ));
    assert_eq!(*log.borrow(), ["progress", "notes:693/2517", "paper"]);
}

/// Two merged attempts with the same title are ambiguity, not a guess.
#[test]
fn two_attempts_with_one_title_are_ambiguous() {
    let mut fake = Fake::single();
    fake.paper = Some(paper_page(&PAPER_ROWS));
    assert_eq!(
        code(run_grade(fake, &[PERFECT]).unwrap_err()),
        "source_selection_missing_or_ambiguous"
    );
}

/// An attempt that appears under a different title in the paper search is
/// `textbook_section_title_conflict_for_attempt` in the grading read and in
/// the index read (rethrown even though other paper failures degrade).
#[test]
fn title_conflict_for_attempt_fails_grading_and_index() {
    let conflicting = paper_page(&[("81003", "82003", PERFECT, None)]);
    let mut fake = Fake::single();
    fake.paper = Some(conflicting.clone());
    assert_eq!(
        code(run_grade(fake, &[WRONG_TITLE]).unwrap_err()),
        "textbook_section_title_conflict_for_attempt"
    );
    let mut fake = Fake::single();
    fake.paper = Some(conflicting);
    assert_eq!(
        code(run_index(fake).unwrap_err()),
        "textbook_section_title_conflict_for_attempt"
    );
}

/// Detail rows are ordinal, lecture key, outer lecture key and scoring,
/// sorted by NUMERIC ordinal (10 after 2), and a null key stays null.
#[test]
fn detail_items_sort_by_numeric_ordinal() {
    let mut fake = Fake::single();
    fake.details.insert(
        "82003".into(),
        detail_json(
            WRONG_TITLE,
            json!(84),
            json!([
                {"exam_no": 10, "lecture_key": "300", "outer_lecture_key": 900, "scoring_result": "X"},
                {"exam_no": 2, "lecture_key": null, "scoring_result": "O"},
                {"exam_no": "1", "lecture_key": 100, "outer_lecture_key": null, "scoring_result": "O"}
            ]),
        ),
    );
    let g = graded(run_grade(fake, &[WRONG_TITLE]).unwrap());
    let items = &g.records[0].items;
    assert_eq!(
        items.iter().map(|i| i.ordinal.as_str()).collect::<Vec<_>>(),
        ["1", "2", "10"]
    );
    assert_eq!(items[0].lecture_key.as_deref(), Some("100"));
    assert_eq!(items[0].outer_lecture_key, None);
    assert_eq!(items[1].lecture_key, None);
    assert_eq!(items[2].outer_lecture_key.as_deref(), Some("900"));
    assert_eq!(items[2].scoring, Scoring::Wrong);
    assert_eq!(
        (g.records[0].correct_count(), g.records[0].item_count()),
        (2, 3)
    );
}

/// Records are re-sorted by title as JavaScript does (UTF-16 code units):
/// U+10000 (surrogates D800 DC00) sorts BEFORE U+FF21, while by code point
/// it would sort after it.
#[test]
fn records_sort_by_utf16_code_unit_not_code_point() {
    let high = "[필수예제] [가우스] \u{10000} 단원 - Invented Learner";
    let low = "[필수예제] [가우스] \u{FF21} 단원 - Invented Learner";
    assert!(low < high, "code point order puts U+FF21 first");
    let mut fake = Fake::single();
    fake.notes.insert(
        ("693".into(), "2517".into()),
        notes_json(&[(1, 11, high), (2, 12, low)]),
    );
    fake.paper = None;
    for (attempt, t) in [("11", high), ("12", low)] {
        fake.details.insert(
            attempt.into(),
            detail_json(t, json!(90), json!([{"exam_no": 1, "scoring_result": "O"}])),
        );
    }
    let g = graded(run_grade(fake, &[low, high]).unwrap());
    assert_eq!(g.records[0].source_title, high);
    assert_eq!(g.records[1].source_title, low);
}

/// Grading refusals before and after the reads, each with its exact TS code.
#[test]
fn grading_refusals_carry_the_exact_codes() {
    let t = |values: &[&str]| titles(values);
    let many: Vec<String> = (0..13).map(|i| format!("title {i}")).collect();
    let grade = |wanted: &[String]| {
        let fake = Fake::single();
        let log = fake.log.clone();
        let result = source(fake).grade(PRINCIPAL, &student(), &scope("693", "2517"), wanted);
        (result, log.borrow().len())
    };
    for (wanted, expected) in [
        (vec![], "invalid_source_selection"),
        (many, "invalid_source_selection"),
        (t(&["a", "a"]), "invalid_source_selection"),
        (t(&["   "]), "invalid_read_name"),
        (vec!["x".repeat(201)], "invalid_read_name"),
    ] {
        let (result, reads) = grade(&wanted);
        assert_eq!(code(result.unwrap_err()), expected);
        assert_eq!(reads, 0, "no read happens before request validation");
    }
    // 12 titles are within the bound: the request proceeds to the reads.
    let twelve: Vec<String> = (0..12).map(|i| format!("title {i}")).collect();
    let (result, reads) = grade(&twelve);
    assert!(matches!(
        result.unwrap(),
        GradeOutcome::NotObservedInEither(_)
    ));
    assert_eq!(reads, 3);
    assert_eq!(
        code(
            source(Fake::single())
                .grade(
                    "Someone Else",
                    &student(),
                    &scope("693", "2517"),
                    &t(&[WRONG_TITLE])
                )
                .unwrap_err()
        ),
        "teacher_scope_conflict"
    );
    assert_eq!(
        code(
            source(Fake::single())
                .index("Someone Else", &student(), &scope("693", "2517"))
                .unwrap_err()
        ),
        "textbook_index_scope_required"
    );
    assert_eq!(
        code(
            source(Fake::single())
                .bindings("Someone Else", &student())
                .unwrap_err()
        ),
        "textbook_binding_principal_rejected"
    );
    assert_eq!(
        code(TextbookStudent::new("0777", NAME).unwrap_err()),
        "invalid_read_key"
    );
    assert_eq!(
        code(TextbookStudent::new(KEY, " ").unwrap_err()),
        "invalid_read_name"
    );
    assert_eq!(
        code(BookScope::new("693", "x").unwrap_err()),
        "invalid_read_key"
    );
    // The requested scope must be bound on the progress page.
    let mut fake = Fake::single();
    fake.notes.insert(("1".into(), "1".into()), NOTES.into());
    assert_eq!(
        code(
            source(fake)
                .grade(PRINCIPAL, &student(), &scope("1", "1"), &t(&[WRONG_TITLE]))
                .unwrap_err()
        ),
        "exact_textbook_scope_not_observed"
    );
}

/// Wrong-note and detail contract refusals with the exact TS codes.
#[test]
fn wrong_note_and_detail_contract_refusals() {
    let notes = |body: &str| {
        let mut fake = Fake::single();
        fake.notes
            .insert(("693".into(), "2517".into()), body.into());
        run_grade(fake, &[WRONG_TITLE])
    };
    let row = |extra: Value| {
        let mut base = json!({"pri_no": 77001, "name": NAME, "p_no": 81003, "testing_no": 82003,
            "testing_name": WRONG_TITLE, "apply_date": "11-03"});
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        json!({"result": "ok", "noteResultList": [base]}).to_string()
    };
    assert_eq!(
        code(notes(r#"{"result":"error","noteResultList":[]}"#).unwrap_err()),
        "wrong_note_contract_rejected"
    );
    assert_eq!(
        code(notes(r#"{"result":"ok"}"#).unwrap_err()),
        "wrong_note_contract_rejected"
    );
    assert_eq!(code(notes("[]").unwrap_err()), "invalid_read_request");
    assert_eq!(code(notes("{").unwrap_err()), "source_json_rejected");
    assert_eq!(
        code(notes(r#"{"result":"ok","noteResultList":[1]}"#).unwrap_err()),
        "invalid_read_request"
    );
    assert_eq!(
        code(notes(&row(json!({"pri_no": 77002}))).unwrap_err()),
        "source_subject_conflict"
    );
    assert_eq!(
        code(notes(&row(json!({"name": "Other Learner"}))).unwrap_err()),
        "source_subject_conflict"
    );
    assert_eq!(
        code(notes(&row(json!({"p_no": "007"}))).unwrap_err()),
        "invalid_read_key"
    );
    assert_eq!(
        code(notes(&row(json!({"testing_no": 1.5}))).unwrap_err()),
        "invalid_read_key"
    );
    assert_eq!(
        code(notes(&row(json!({"testing_name": "  "}))).unwrap_err()),
        "invalid_read_name"
    );
    // A wrong-note row with a string key is accepted as TS accepts it.
    assert!(notes(&row(json!({"testing_no": "82003"}))).is_ok());

    let detail = |body: String| {
        let mut fake = Fake::single();
        fake.details.insert("82003".into(), body);
        run_grade(fake, &[WRONG_TITLE])
    };
    let one = |item: Value| detail_json(WRONG_TITLE, json!(84), json!([item]));
    assert_eq!(
        code(detail(r#"{"result":"OK","print_list":[]}"#.into()).unwrap_err()),
        "ambiguous_captured_result"
    );
    assert_eq!(
        code(detail(r#"{"result":"ok","print_list":[{}]}"#.into()).unwrap_err()),
        "ambiguous_captured_result"
    );
    assert_eq!(
        code(detail(r#"{"result":"OK","print_list":[1]}"#.into()).unwrap_err()),
        "invalid_capture_shape"
    );
    assert_eq!(
        code(detail("[]".into()).unwrap_err()),
        "invalid_read_request"
    );
    assert_eq!(
        code(detail(detail_json(WRONG_TITLE, json!(84), json!([]))).unwrap_err()),
        "captured_items_missing"
    );
    let other = detail_json(
        WRONG_TITLE,
        json!(84),
        json!([{"exam_no": 1, "scoring_result": "O"}]),
    );
    assert_eq!(
        code(detail(other.replace("77001", "77002")).unwrap_err()),
        "captured_subject_mismatch"
    );
    assert_eq!(
        code(detail(other.replace(NAME, "Other Learner")).unwrap_err()),
        "captured_subject_mismatch"
    );
    assert_eq!(
        code(detail(other.replace("77001", "\"abc\"")).unwrap_err()),
        "invalid_source_key"
    );
    assert_eq!(
        code(
            detail(detail_json(
                WRONG_TITLE,
                json!(101),
                json!([{"exam_no": 1, "scoring_result": "O"}])
            ))
            .unwrap_err()
        ),
        "invalid_captured_score"
    );
    assert_eq!(
        code(
            detail(detail_json(
                WRONG_TITLE,
                json!(-1),
                json!([{"exam_no": 1, "scoring_result": "O"}])
            ))
            .unwrap_err()
        ),
        "invalid_captured_score"
    );
    assert_eq!(
        code(
            detail(detail_json(
                WRONG_TITLE,
                json!("84"),
                json!([{"exam_no": 1, "scoring_result": "O"}])
            ))
            .unwrap_err()
        ),
        "invalid_captured_score"
    );
    assert_eq!(code(detail(detail_json(WRONG_TITLE, json!(84), json!([{"exam_no": 1, "scoring_result": "O"}, {"exam_no": "1", "scoring_result": "X"}]))).unwrap_err()), "duplicate_captured_item");
    assert_eq!(
        code(detail(one(json!({"exam_no": 1, "scoring_result": "o"}))).unwrap_err()),
        "unknown_captured_scoring"
    );
    assert_eq!(
        code(detail(one(json!({"exam_no": 0, "scoring_result": "O"}))).unwrap_err()),
        "invalid_source_key"
    );
    assert_eq!(
        code(detail(one(json!(5))).unwrap_err()),
        "invalid_capture_shape"
    );
    // The lenient score-view key accepts "07"; the strict item key then refuses it.
    assert_eq!(
        code(detail(one(json!({"exam_no": "07", "scoring_result": "O"}))).unwrap_err()),
        "invalid_read_key"
    );
    assert_eq!(
        code(
            detail(one(
                json!({"exam_no": 1, "lecture_key": "01", "scoring_result": "O"})
            ))
            .unwrap_err()
        ),
        "invalid_read_key"
    );
    // The detail sub-title must equal the requested title.
    assert_eq!(
        code(
            detail(detail_json(
                "[필수예제] [가우스] 5-3 진분수의 뺄셈 - Invented Learner ",
                json!(84),
                json!([{"exam_no": 1, "scoring_result": "O"}])
            ))
            .unwrap_err()
        ),
        "source_title_conflict"
    );
}

// ------------------------------------------- source execution (step 1)

fn tweak(
    f: impl Fn(&str, TextbookResponse) -> Result<TextbookResponse> + 'static,
) -> Option<Tweak> {
    Some(Box::new(f))
}

/// Session pages, transport refusals and the applied policy, each with the
/// exact TS code and for every one of the three reads.
#[test]
fn session_page_and_transport_refusals() {
    for kind in ["progress", "notes", "detail"] {
        for (body, expected) in [
            ("<p>webUserPass</p>", "authentication_required"),
            ("WEBUSERPASS", "authentication_required"),
            ("세션이 만료되었습니다", "authentication_required"),
            (
                "<INPUT class=x TYPE = 'password'>",
                "authentication_required",
            ),
            ("<form id=a NAME=loginForm>", "authentication_required"),
        ] {
            let mut fake = Fake::single();
            let body = body.to_string();
            fake.tweak = tweak(move |k, r| {
                Ok(if k == kind {
                    resp(&format!("{}{body}", String::from_utf8(r.bytes).unwrap()))
                } else {
                    r
                })
            });
            assert_eq!(
                code(run_grade(fake, &[WRONG_TITLE]).unwrap_err()),
                expected,
                "{kind}"
            );
        }
        type Change = fn(&mut TextbookResponse);
        let cases: [(Change, &str); 8] = [
            (|r| r.status = 500, "source_transport_failed"),
            (|r| r.status = 302, "source_transport_failed"),
            (|r| r.status = 199, "source_transport_failed"),
            (|r| r.redirected = true, "source_transport_failed"),
            (|r| r.credentials_sent = false, "source_transport_failed"),
            (|r| r.redirect_followed = true, "source_transport_failed"),
            (
                |r| r.bytes = vec![0xff, 0xfe],
                "source_response_encoding_rejected",
            ),
            (
                |r| r.bytes = vec![b'a'; MAX_SECTION_BODY_BYTES + 1],
                "source_response_too_large",
            ),
        ];
        for (change, expected) in cases {
            let mut fake = Fake::single();
            fake.tweak = tweak(move |k, mut r| {
                if k == kind {
                    change(&mut r);
                }
                Ok(r)
            });
            assert_eq!(
                code(run_grade(fake, &[WRONG_TITLE]).unwrap_err()),
                expected,
                "{kind}"
            );
        }
    }
    // Any 2xx is OK in TS (`response.ok`).
    let mut fake = Fake::single();
    fake.tweak = tweak(|_, mut r| {
        r.status = 203;
        Ok(r)
    });
    assert!(run_grade(fake, &[WRONG_TITLE]).is_ok());
}

/// A port error propagates unchanged with its code and the request count
/// does not include the failed call.
#[test]
fn port_error_propagates_and_requests_are_counted() {
    let mut fake = Fake::single();
    fake.tweak = tweak(|k, r| {
        if k == "notes" {
            Err(Error::refusal("port_failure"))
        } else {
            Ok(r)
        }
    });
    let mut s = source(fake);
    assert_eq!(
        code(
            s.grade(
                PRINCIPAL,
                &student(),
                &scope("693", "2517"),
                &titles(&[WRONG_TITLE])
            )
            .unwrap_err()
        ),
        "port_failure"
    );
    assert_eq!(
        s.requests(),
        1,
        "only the answered progress read is counted"
    );
    let mut s = source(Fake::single());
    s.grade(
        PRINCIPAL,
        &student(),
        &scope("693", "2517"),
        &titles(&[WRONG_TITLE]),
    )
    .unwrap();
    assert_eq!(s.requests(), 4);
}

/// Every read passes its named bound and request policy to the port.
#[test]
fn every_read_receives_its_bound_and_policy() {
    let fake = Fake::single();
    let seen = fake.seen.clone();
    run_grade(fake, &[WRONG_TITLE]).unwrap();
    for (kind, max, policy) in seen.borrow().iter() {
        assert_eq!(*policy, SECTION_FETCH, "{kind}");
        assert_eq!(
            *max,
            if kind == "paper" {
                MAX_UNIT_BODY_BYTES
            } else {
                MAX_SECTION_BODY_BYTES
            },
            "{kind}"
        );
    }
    assert_eq!(seen.borrow().len(), 4);
    let mut fake = Fake::single();
    fake.progress = PROGRESS_TWO.to_string();
    let seen = fake.seen.clone();
    source(fake).bindings(PRINCIPAL, &student()).unwrap();
    assert_eq!(
        seen.borrow()[0],
        ("progress".to_string(), MAX_UNIT_BODY_BYTES, SECTION_FETCH)
    );
    const { assert!(SECTION_FETCH.send_credentials && !SECTION_FETCH.follow_redirects) };
}

/// The bounds are the real constants (named, with the unit each counts).
#[test]
fn bounds_are_the_real_constants() {
    assert_eq!(MAX_SECTION_BODY_BYTES, 2_000_000);
    assert_eq!(MAX_BODY_UNITS, 2_000_000);
    assert_eq!(MAX_UNIT_BODY_BYTES, 6_000_000);
    assert_eq!(MAX_SOURCE_TITLES, 12);
    assert_eq!(MAX_WRONG_NOTE_ROWS, 1000);
    assert_eq!((MIN_BOOK_LABEL_UNITS, MAX_BOOK_LABEL_UNITS), (2, 200));
    assert_eq!((MAX_HINT_ROWS, MAX_HINT_TITLE_UNITS), (1000, 2000));
    assert_eq!(MAX_SPAN, 1000);
    assert_eq!(
        TESTPAGE_SELECT_PATH,
        "/servlet/controller.common.TestpageSelectExServlet"
    );
}

fn padded(base: &str, wide: &str, units_or_bytes_target: usize, by_bytes: bool) -> String {
    // base + "<!--" + padding + "-->", padded with `wide` (a multi-byte char)
    // and ASCII to reach the target exactly.
    let fixed = if by_bytes {
        base.len()
    } else {
        base.encode_utf16().count()
    } + 7;
    let mut out = format!("{base}<!--");
    let (wide_cost, ascii_cost) = if by_bytes {
        (wide.len(), 1)
    } else {
        (wide.encode_utf16().count(), 1)
    };
    let remaining = units_or_bytes_target - fixed;
    for _ in 0..remaining / wide_cost {
        out.push_str(wide);
    }
    for _ in 0..remaining % wide_cost / ascii_cost {
        out.push('x');
    }
    out.push_str("-->");
    out
}

/// Keyed reads count BYTES (TS `#readRequest` streams and refuses above
/// 2,000,000 bytes, `source_response_too_large`): a multi-byte body of exactly
/// 2,000,000 bytes (about 667 thousand units) is accepted and one byte more is
/// refused although its unit count is far below 2,000,000.
#[test]
fn keyed_bound_counts_bytes_at_the_bound_and_one_over() {
    for (extra, ok) in [(0usize, true), (1, false)] {
        let mut fake = Fake::single();
        fake.progress = padded(PROGRESS_SINGLE, "가", MAX_SECTION_BODY_BYTES + extra, true);
        assert_eq!(fake.progress.len(), MAX_SECTION_BODY_BYTES + extra);
        assert!(fake.progress.encode_utf16().count() < 700_000);
        let result = run_index(fake);
        if ok {
            assert_eq!(result.unwrap().rows.len(), 2);
        } else {
            assert_eq!(code(result.unwrap_err()), "source_response_too_large");
        }
    }
}

/// The binding progress read counts UTF-16 UNITS (TS `text.length > 2000000`,
/// `textbook_binding_source_rejected`): 2,000,000 units made of 3-byte
/// characters (about 6 MB) are accepted, one more unit is refused, and a
/// surrogate-pair body is refused by units although it is under the byte
/// ceiling.
#[test]
fn binding_bound_counts_utf16_units_at_the_bound_and_one_over() {
    let bindings = |html: String| {
        let mut fake = Fake::single();
        fake.progress = html;
        source(fake).bindings(PRINCIPAL, &student())
    };
    let at = padded(PROGRESS_TWO, "가", MAX_BODY_UNITS, false);
    assert_eq!(at.encode_utf16().count(), MAX_BODY_UNITS);
    assert!(at.len() > 5_000_000 && at.len() <= MAX_UNIT_BODY_BYTES);
    assert_eq!(bindings(at.clone()).unwrap().len(), 2);
    let over = padded(PROGRESS_TWO, "가", MAX_BODY_UNITS + 1, false);
    assert_eq!(
        code(bindings(over).unwrap_err()),
        "textbook_binding_source_rejected"
    );
    let emoji = padded(PROGRESS_TWO, "😀", MAX_BODY_UNITS + 2, false);
    assert!(emoji.len() < MAX_UNIT_BODY_BYTES);
    assert_eq!(
        code(bindings(emoji).unwrap_err()),
        "textbook_binding_source_rejected"
    );
    assert_eq!(
        code(bindings(format!("{at}{at}")).unwrap_err()),
        "textbook_binding_source_rejected"
    );
}

/// Invalid UTF-8 is refused with one catalogued code (TS decodes lossily).
#[test]
fn binding_read_refuses_invalid_utf8_and_transport_failures() {
    type Change = Box<dyn Fn(&mut TextbookResponse)>;
    let cases: Vec<(Change, &str)> = vec![
        (
            Box::new(|r: &mut TextbookResponse| r.bytes = vec![0xff]),
            "source_response_encoding_rejected",
        ),
        (
            Box::new(|r: &mut TextbookResponse| r.status = 503),
            "source_transport_failed",
        ),
        (
            Box::new(|r: &mut TextbookResponse| r.redirected = true),
            "source_transport_failed",
        ),
        (
            Box::new(|r: &mut TextbookResponse| r.credentials_sent = false),
            "source_transport_failed",
        ),
    ];
    for (change, expected) in cases {
        let mut fake = Fake::single();
        fake.tweak = tweak(move |_, mut r| {
            change(&mut r);
            Ok(r)
        });
        assert_eq!(
            code(source(fake).bindings(PRINCIPAL, &student()).unwrap_err()),
            expected
        );
    }
    // The binding read has no broad session-page scan: a login form is not
    // refused there, the narrower prestudy expression is.
    let mut fake = Fake::single();
    fake.progress = format!("<form name=loginForm></form>{PROGRESS_SINGLE}");
    assert_eq!(
        source(fake).bindings(PRINCIPAL, &student()).unwrap().len(),
        1
    );
    for page in [
        "popup_session_error.jsp",
        "세션이 만료",
        "<input type=password>",
    ] {
        let mut fake = Fake::single();
        fake.progress = format!("{page}{PROGRESS_SINGLE}");
        assert_eq!(
            code(source(fake).bindings(PRINCIPAL, &student()).unwrap_err()),
            "prestudy_authentication_required"
        );
    }
}

/// `parseTextbookScopeBinding`: the exact (course, book, student) binding
/// must be observed; the raw onclick scan accepts both quote forms and the
/// entity-escaped quotes; TS keeps the matches in a Set, so an identical
/// button repeated is still one binding.
#[test]
fn scope_binding_must_be_observed() {
    let progress = |html: &str| {
        let mut fake = Fake::single();
        fake.progress = html.to_string();
        run_index(fake)
    };
    assert!(progress(PROGRESS_SINGLE).is_ok());
    for page in [
        "",
        "<button onclick=\"createBookIncorrectPapers(693,2517,77002)\">x</button>",
        "<button onclick=\"createBookIncorrectPapers(693,2518,77001)\">x</button>",
        "<button onclick=\"createBookIncorrectPapers(0693,2517,77001)\">x</button>",
        "<button onclick=\"createBookIncorrectPapers(693,2517,77001\">x</button>",
        "<button onclick=\"createBookIncorrectPapers(693,2517,77001)",
    ] {
        assert_eq!(
            code(progress(page).unwrap_err()),
            "exact_textbook_scope_not_observed",
            "{page}"
        );
    }
    for page in [
        "<b onclick='createBookIncorrectPapers(\"693\",\"2517\",\"77001\")'>x</b>",
        "<b ONCLICK = \"createBookIncorrectPapers( &#39;693&#39; , &apos;2517&apos; , &#39;77001&#39; )\">x</b>",
        "<b onclick=\"createBookIncorrectPapers(&quot;693&quot;,&quot;2517&quot;,&quot;77001&quot;)\">x</b>",
        // the TS expression scans raw text with `\b`, so `data-onclick` matches too
        "<button data-onclick=\"createBookIncorrectPapers(693,2517,77001)\">x</button>",
        // a first attribute that does not match does not hide a later one
        "<b onclick=\"x()\" onclick=\"createBookIncorrectPapers(693,2517,77001)\">x</b>",
        // an identical button repeated is one binding (Set semantics)
        "<b onclick=\"createBookIncorrectPapers(693,2517,77001)\"></b><b onclick=\"createBookIncorrectPapers(693,2517,77001)\"></b>",
    ] {
        assert!(progress(page).is_ok(), "{page}");
    }
}

// ----------------------------------------------------- index (step 3)

/// Ports the merged-index vectors (R07-2): wrong rows plus the all-correct
/// row from the one observed paper page, `sources`, the basis and the coverage
/// literal; the paper page holds rows of all types and only template titles
/// join.
#[test]
fn index_merges_all_correct_rows_with_sources_basis_and_coverage() {
    let fake = Fake::single();
    let log = fake.log.clone();
    let index = run_index(fake).unwrap();
    assert_eq!(*log.borrow(), ["progress", "notes:693/2517", "paper"]);
    assert_eq!(index.rows.len(), 2);
    assert_eq!(index.rows[0].source_title, WRONG_TITLE);
    assert_eq!(
        index.rows[0]
            .sources
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        ["wrong_answer_note", "student_paper_search"]
    );
    assert_eq!(index.rows[1].source_title, PERFECT);
    assert_eq!(index.rows[1].sources, [IndexSource::StudentPaperSearch]);
    assert_eq!(
        index.index_basis.as_str(),
        "wrong_answer_note_plus_student_paper_search"
    );
    assert_eq!(
        index.coverage,
        "wrong_answer_note_index_plus_one_observed_student_paper_search_page_not_complete_history"
    );
    assert_eq!(
        index.paper_search,
        PaperSearchStatus::Observed {
            matched_row_count: 2
        }
    );
    assert_eq!(index.scope.course_key(), "693");
    assert_eq!(
        index.index_basis.by_name_coverage(),
        BY_NAME_COVERAGE_WITH_PAPER_SEARCH
    );
}

/// A failed paper search degrades the basis exactly as TS does; no pattern
/// means no paper read at all.
#[test]
fn index_degrades_to_wrong_note_only_basis() {
    let mut fake = Fake::single();
    fake.paper = None;
    let index = run_index(fake).unwrap();
    assert_eq!(index.rows.len(), 1);
    assert_eq!(index.index_basis, IndexBasis::WrongAnswerNoteOnly);
    assert_eq!(index.index_basis.as_str(), "wrong_answer_note_only");
    assert_eq!(index.coverage, INDEX_COVERAGE_WRONG_NOTE_ONLY);
    assert_eq!(
        index.coverage,
        "wrong_answer_note_index_only_paper_search_not_observed_not_complete_history"
    );
    assert_eq!(index.paper_search, PaperSearchStatus::Unavailable);
    assert_eq!(
        index.index_basis.by_name_coverage(),
        BY_NAME_COVERAGE_WRONG_NOTE_ONLY
    );
    // Transport-level failure of the paper page degrades too.
    let mut fake = Fake::single();
    fake.tweak = tweak(|k, mut r| {
        if k == "paper" {
            r.status = 500;
        }
        Ok(r)
    });
    assert_eq!(
        run_index(fake).unwrap().paper_search,
        PaperSearchStatus::Unavailable
    );
    // No derivable pattern: no paper read, the status says so.
    let mut fake = Fake::single();
    fake.notes.insert(
        ("693".into(), "2517".into()),
        notes_json(&[(1, 2, "plain title")]),
    );
    let log = fake.log.clone();
    let index = run_index(fake).unwrap();
    assert_eq!(
        index.paper_search,
        PaperSearchStatus::NoDerivableSectionTitlePattern
    );
    assert_eq!(index.index_basis, IndexBasis::WrongAnswerNoteOnly);
    assert_eq!(*log.borrow(), ["progress", "notes:693/2517"]);
}

/// The index read caps wrong-note rows at 1000 and refuses a repeated
/// identity; the grading read does neither (TS parity).
#[test]
fn index_caps_rows_and_refuses_repeated_identity() {
    let many: Vec<(u64, u64, &str)> = (0..1001).map(|i| (i + 1, i + 1, "plain title")).collect();
    let thousand: Vec<(u64, u64, &str)> = many[..1000].to_vec();
    let mut fake = Fake::single();
    fake.notes
        .insert(("693".into(), "2517".into()), notes_json(&many));
    assert_eq!(
        code(run_index(fake).unwrap_err()),
        "wrong_note_contract_rejected"
    );
    let mut fake = Fake::single();
    fake.notes
        .insert(("693".into(), "2517".into()), notes_json(&thousand));
    assert_eq!(run_index(fake).unwrap().rows.len(), 1000);
    let repeated = notes_json(&[(1, 2, "plain title"), (1, 2, "plain title two")]);
    let mut fake = Fake::single();
    fake.notes
        .insert(("693".into(), "2517".into()), repeated.clone());
    assert_eq!(
        code(run_index(fake).unwrap_err()),
        "ambiguous_textbook_index"
    );
    // The grading read has no repeated-identity or row-cap check (TS parity).
    let mut fake = Fake::single();
    fake.notes.insert(("693".into(), "2517".into()), repeated);
    fake.paper = None;
    fake.details.insert(
        "2".into(),
        detail_json(
            "plain title",
            json!(70),
            json!([{"exam_no": 1, "scoring_result": "X"}]),
        ),
    );
    let g = graded(run_grade(fake, &["plain title"]).unwrap());
    assert_eq!(g.records[0].score, 70.0);
}

// -------------------------------------------- bindings and interview (R07-1)

fn two_books_fake() -> Fake {
    let mut fake = Fake::single();
    fake.progress = PROGRESS_TWO.to_string();
    fake.notes.clear();
    fake.notes.insert(
        ("5".into(), "7".into()),
        notes_json(&[(1, 82003, "Unit for book 7")]),
    );
    fake.notes.insert(
        ("6".into(), "8".into()),
        notes_json(&[(2, 82004, "Unit for book 8")]),
    );
    fake.details.clear();
    fake.details.insert(
        "82004".into(),
        detail_json(
            "Unit for book 8",
            json!(80),
            json!([{"exam_no": 1, "lecture_key": 8101, "scoring_result": "O"}]),
        ),
    );
    fake.paper = None;
    fake
}

/// Ports native-dailyrecord-backend.test.ts:141 (R07-1): several bindings and
/// no label return the interview with label, book number and the first
/// observed section title as hint, and execute no grading read.
#[test]
fn multiple_bindings_return_interview_with_hints_and_no_grading() {
    let fake = two_books_fake();
    let log = fake.log.clone();
    let mut s = source(fake);
    let result = s
        .sections_by_name(PRINCIPAL, &student(), None, None)
        .unwrap();
    let SectionsByName::Interview(interview) = result else {
        panic!("interview expected")
    };
    assert_eq!(INTERVIEW_REASON, "multiple_book_bindings_select_one");
    assert_eq!(
        interview.bindings,
        [
            InterviewCandidate {
                book_label: "Book One".into(),
                book_no: "7".into(),
                hint: Some("Unit for book 7".into())
            },
            InterviewCandidate {
                book_label: "Book Two".into(),
                book_no: "8".into(),
                hint: Some("Unit for book 8".into())
            },
        ]
    );
    assert!(log.borrow().iter().all(|e| !e.starts_with("detail")));
    assert_eq!(
        *log.borrow(),
        ["progress", "progress", "notes:5/7", "progress", "notes:6/8"]
    );
    // Titles are ignored for an interview: still no grading read.
    let mut s = source(two_books_fake());
    let result = s
        .sections_by_name(
            PRINCIPAL,
            &student(),
            None,
            Some(&titles(&["Unit for book 8"])),
        )
        .unwrap();
    assert!(matches!(result, SectionsByName::Interview(_)));
}

/// A failed index read gives no hint for that binding.
#[test]
fn interview_hint_is_absent_when_its_index_read_fails() {
    let mut fake = two_books_fake();
    fake.notes.remove(&("5".to_string(), "7".to_string()));
    let result = source(fake)
        .sections_by_name(PRINCIPAL, &student(), None, None)
        .unwrap();
    let SectionsByName::Interview(interview) = result else {
        panic!()
    };
    assert_eq!(interview.bindings[0].hint, None);
    assert_eq!(
        interview.bindings[1].hint.as_deref(),
        Some("Unit for book 8")
    );
    // A hint violating the rule (title over 2000 units; over 1000 rows) is absent.
    let mut fake = two_books_fake();
    let long = "t".repeat(2001);
    fake.notes.insert(
        ("5".into(), "7".into()),
        notes_json(&[(1, 1, long.as_str())]),
    );
    // wrong-note titles are capped at 200 units by readLiteralName: that read fails too.
    let SectionsByName::Interview(interview) = source(fake)
        .sections_by_name(PRINCIPAL, &student(), None, None)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(interview.bindings[0].hint, None);
}

/// Ports native-dailyrecord-backend.test.ts:153: an exact label selects its
/// binding for the index and for grading; an unknown label fails closed with
/// no further read.
#[test]
fn exact_label_selects_and_unknown_label_fails_closed() {
    let fake = two_books_fake();
    let log = fake.log.clone();
    let mut s = source(fake);
    let result = s
        .sections_by_name(PRINCIPAL, &student(), Some("Book Two"), None)
        .unwrap();
    let SectionsByName::Index(by_name) = result else {
        panic!("index expected")
    };
    assert_eq!(
        (by_name.scope.course_key(), by_name.scope.book_no()),
        ("6", "8")
    );
    assert_eq!(by_name.book_label, "Book Two");
    assert_eq!(by_name.index.rows[0].source_title, "Unit for book 8");
    assert_eq!(log.borrow().last().unwrap(), "notes:6/8");
    let graded = s
        .sections_by_name(
            PRINCIPAL,
            &student(),
            Some("Book Two"),
            Some(&titles(&["Unit for book 8"])),
        )
        .unwrap();
    let SectionsByName::Graded(g) = graded else {
        panic!("graded expected")
    };
    assert_eq!((g.scope.course_key(), g.scope.book_no()), ("6", "8"));
    let GradeOutcome::Graded(sections) = g.outcome else {
        panic!()
    };
    assert_eq!(sections.records[0].score, 80.0);
    assert_eq!(
        (
            sections.records[0].correct_count(),
            sections.records[0].item_count()
        ),
        (1, 1)
    );
    assert_eq!(sections.records[0].course_key, "6");

    let fake = two_books_fake();
    let log = fake.log.clone();
    assert_eq!(
        code(
            source(fake)
                .sections_by_name(PRINCIPAL, &student(), Some("No Such Book"), None)
                .unwrap_err()
        ),
        "textbook_binding_missing_or_ambiguous"
    );
    assert_eq!(*log.borrow(), ["progress"], "no index and no grading read");
}

/// Ports native-dailyrecord-backend.test.ts:169: unqualified labels fail
/// closed before candidates; duplicate labels stay in the interview and an
/// exact duplicate label is ambiguous; a single binding is unchanged.
#[test]
fn unqualified_and_duplicate_labels_fail_closed() {
    let page = |a: &str, b: &str| {
        format!(
            "<table><tr><td>x</td><td>{a}</td><td><b onclick=\"createBookIncorrectPapers(5,7,77001)\"></b></td></tr><tr><td>x</td><td>{b}</td><td><b onclick=\"createBookIncorrectPapers(6,8,77001)\"></b></td></tr></table>"
        )
    };
    let by_name = |html: String, label: Option<&str>| {
        let mut fake = two_books_fake();
        fake.progress = html;
        let log = fake.log.clone();
        let result = source(fake).sections_by_name(PRINCIPAL, &student(), label, None);
        (result, log.borrow().len())
    };
    // The percent cell does not qualify; the row has no other label.
    let (result, reads) = by_name(page("88.8 %", "Book Two"), None);
    assert_eq!(
        code(result.unwrap_err()),
        "textbook_binding_source_unqualified"
    );
    assert_eq!(reads, 1);
    let (result, _) = by_name(page("Same Book", "Same Book"), None);
    let SectionsByName::Interview(i) = result.unwrap() else {
        panic!()
    };
    assert_eq!(
        i.bindings
            .iter()
            .map(|b| (b.book_label.as_str(), b.book_no.as_str()))
            .collect::<Vec<_>>(),
        [("Same Book", "7"), ("Same Book", "8")]
    );
    let (result, _) = by_name(page("Same Book", "Same Book"), Some("Same Book"));
    assert_eq!(
        code(result.unwrap_err()),
        "textbook_binding_missing_or_ambiguous"
    );
    // A single binding without label is unchanged.
    let mut fake = two_books_fake();
    fake.progress = "<table><tr><td>x</td><td>Book One</td><td><b onclick=\"createBookIncorrectPapers(5,7,77001)\"></b></td></tr></table>".into();
    let SectionsByName::Index(i) = source(fake)
        .sections_by_name(PRINCIPAL, &student(), None, None)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (i.scope.course_key(), i.book_label.as_str()),
        ("5", "Book One")
    );
}

fn bind(html: &str) -> Result<Vec<TextbookBinding>> {
    parse_textbook_bindings(html, KEY)
}
fn labels(html: &str) -> Vec<String> {
    bind(html)
        .unwrap()
        .into_iter()
        .map(|b| b.book_label)
        .collect()
}
fn button(course: u32, book: u32) -> String {
    format!("<button onclick=\"createBookIncorrectPapers({course},{book},77001)\">x</button>")
}

/// The logical-grid label rules of `parseTextbookBindings`: header-named
/// column first, then a branded label, then the first non-UI title cell;
/// rowspans keep their logical columns; a nested input/image attribute can
/// carry the label.
#[test]
fn binding_label_selection_follows_the_logical_grid() {
    let header = format!(
        "<table><thead><tr><th>구분</th><th>교재명</th><th>비고</th><th></th></tr></thead><tbody><tr><td>중1</td><td>Alpha Book</td><td>가우스 심화</td><td>{}</td></tr></tbody></table>",
        button(1, 2)
    );
    assert_eq!(labels(&header), ["Alpha Book"]);
    let branded = format!(
        "<table><tr><td>중1</td><td>일반 교재</td><td>가우스 수학</td><td>{}</td></tr></table>",
        button(1, 2)
    );
    assert_eq!(labels(&branded), ["가우스 수학"]);
    let ui = format!(
        "<table><tr><td>중1</td><td>생성</td><td>Plain Title</td><td>{}</td></tr></table>",
        button(1, 2)
    );
    assert_eq!(labels(&ui), ["Plain Title"]);
    let only_ui = format!(
        "<table><tr><td>중1</td><td>생성</td><td>{}</td></tr></table>",
        button(1, 2)
    );
    assert_eq!(
        labels(&only_ui),
        ["생성"],
        "falls back to the first qualified cell"
    );
    // The carried rowspan cell keeps column 0, so Book B is not shifted.
    let span = format!(
        "<table><tr><td rowspan=2>중1</td><td>Book A</td><td>{}</td></tr><tr><td>Book B</td><td>{}</td></tr></table>",
        button(1, 1),
        button(1, 2)
    );
    assert_eq!(labels(&span), ["Book A", "Book B"]);
    let colspan = format!(
        "<table><tr><td>중1</td><td colspan=2>Wide Book</td><td>{}</td></tr></table>",
        button(1, 1)
    );
    assert_eq!(labels(&colspan), ["Wide Book"]);
    // A fractional or non-numeric span follows JavaScript Number().
    let fractional = format!(
        "<table><tr><td>중1</td><td colspan='2.5'>Wide Book</td><td colspan='abc'>{}</td></tr></table>",
        button(1, 1)
    );
    assert_eq!(labels(&fractional), ["Wide Book"]);
    let attribute = format!(
        "<table><tr><td>중1</td><td><input type=button value='Attr Book Title'></td><td>{}</td></tr></table>",
        button(1, 1)
    );
    assert_eq!(labels(&attribute), ["Attr Book Title"]);
    // First-observed order is kept; other students' rows are ignored.
    let order = format!(
        "<table><tr><td>x</td><td>Second Book</td><td>{}</td></tr><tr><td>x</td><td>First Book</td><td>{}</td></tr><tr><td>x</td><td>Foreign Book</td><td><b onclick=\"createBookIncorrectPapers(9,9,77002)\"></b></td></tr></table>",
        button(2, 2),
        button(1, 1)
    );
    assert_eq!(labels(&order), ["Second Book", "First Book"]);
    // Same (course, book) twice: same label is one binding, a different label is refused.
    let same = format!(
        "<table><tr><td>x</td><td>Same Book</td><td>{}{}</td></tr></table>",
        button(1, 1),
        button(1, 1)
    );
    assert_eq!(bind(&same).unwrap().len(), 1);
    let conflict = format!(
        "<table><tr><td>x</td><td>Book A</td><td>{}</td></tr><tr><td>x</td><td>Book B</td><td>{}</td></tr></table>",
        button(1, 1),
        button(1, 1)
    );
    assert_eq!(
        code(bind(&conflict).unwrap_err()),
        "textbook_binding_source_unqualified"
    );
    // No row, no table, a row outside the body sections, no bindings at all.
    for html in [
        button(1, 1),
        format!("<table><tfoot><tr><td>x</td><td>Foot Book</td><td>{}</td></tr></tfoot></table>", button(1, 1)),
        "<table><tr><td>nothing</td></tr></table>".to_string(),
        String::new(),
        "<table><tr><td>x</td><td>Foreign Book</td><td><b onclick=\"createBookIncorrectPapers(9,9,77002)\"></b></td></tr></table>".to_string(),
        format!("<table><tr><td>x</td><td>12 %</td><td>2026-11-03</td><td>{}</td></tr></table>", button(1, 1)),
    ] {
        assert_eq!(code(bind(&html).unwrap_err()), "textbook_binding_source_unqualified", "{html}");
    }
    assert_eq!(
        code(bind("normal_session_error.jsp").unwrap_err()),
        "prestudy_authentication_required"
    );
}

/// `isTextbookBookLabel` under the JavaScript rules.
#[test]
fn book_label_predicate() {
    for good in ["Book One", "가우스 수학", "ab", "개념원리 1-1", "\u{0085}A"] {
        assert!(is_textbook_book_label(good), "{good:?}");
    }
    let two_hundred = "a".repeat(200);
    assert!(is_textbook_book_label(&two_hundred));
    let wide = "가".repeat(200);
    assert!(is_textbook_book_label(&wide));
    for bad in [
        "a",
        "",
        "   ",
        "\u{FEFF}A\u{00A0}",
        "88.8 %",
        "12,345.6",
        "123 456",
        "2026-11-03",
        "2026/1/3 10:00",
        "3/4/2026",
        "11.03.26 x",
        "오답노트 보기",
        "My Incorrect list",
        "WRONG answers",
        "1234",
        "--",
    ] {
        assert!(!is_textbook_book_label(bad), "{bad:?}");
    }
    assert!(!is_textbook_book_label(&"a".repeat(201)));
    assert!(!is_textbook_book_label(&"가".repeat(201)));
    // A date prefix followed by a digit is not a date; a surrogate pair counts
    // 2 units but is not a letter.
    assert!(is_textbook_book_label("2026-11-033 Book"));
    assert!(!is_textbook_book_label("😀"));
    assert!(is_textbook_book_label("😀A"));
}

/// Debug output of every student-related type is length and digest only.
#[test]
fn debug_output_never_prints_student_values() {
    let g = graded(run_grade(Fake::single(), &[PERFECT, WRONG_TITLE]).unwrap());
    let index = run_index(Fake::single()).unwrap();
    let bindings = bind(PROGRESS_SINGLE).unwrap();
    let interview = Interview {
        bindings: vec![InterviewCandidate {
            book_label: "Secret Book Label".into(),
            book_no: "7".into(),
            hint: Some("Secret Hint".into()),
        }],
    };
    let pattern = derive_title_pattern(&titles(&[WRONG_TITLE]), NAME).unwrap();
    let rows = [paper_row(&PAPER_ROWS[1])];
    let shown = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        student(),
        scope("693", "2517"),
        g,
        g.records[0],
        g.records[0].items,
        index,
        index.rows,
        bindings,
        interview,
        pattern,
        rows
    );
    for secret in [
        "Invented",
        "Learner",
        "77001",
        "가우스",
        "Secret",
        "필수예제",
        "Gauss fraction",
        "11-03",
    ] {
        assert!(!shown.contains(secret), "{secret} leaked: {shown}");
    }
    assert!(shown.contains("sha256"));
}

// ------------------------------------------- admitted contracts, wire

/// The three section reads and the paper search each have an admitted
/// contract, and the Academy operation list carries the three new selectors.
#[test]
fn contracts_and_operation_list_carry_the_new_entries() {
    let contracts = read_contracts();
    for (id, selector) in [
        ("textbook.wrongnote", "getWrongAnswerNoteResult"),
        ("textbook.detail", "getStudyResultSingleTestingSingleUser"),
    ] {
        let contract = contracts.iter().find(|c| c.id == id).unwrap();
        assert_eq!(contract.method, "POST");
        assert_eq!(contract.path, TESTPAGE_SELECT_PATH);
        assert_eq!(contract.source_namespace, "academy.lms.student_course");
        assert!(!contract.effect);
        assert!(contract.query.is_empty());
        let value = serde_json::to_value(contract).unwrap();
        assert_eq!(value["body"][0]["name"], "p_process");
        assert_eq!(value["body"][0]["value"]["value"], selector);
        assert_eq!(value["body"].as_array().unwrap().len(), 2);
    }
    for id in [
        "student.books",
        "metadata.paper.initial",
        "metadata.paper.search.7",
    ] {
        assert!(contracts.iter().any(|c| c.id == id), "{id}");
    }
    let operations = http::operations();
    for selector in [
        "getTextBookProgress",
        "getWrongAnswerNoteResult",
        "getStudyResultSingleTestingSingleUser",
    ] {
        assert!(
            operations
                .array()
                .iter()
                .any(|op| op.get("selector").is(selector) && op.get("method").is("POST")),
            "{selector}"
        );
    }
    let progress = operations
        .array()
        .iter()
        .find(|op| op.get("selector").is("getTextBookProgress"))
        .unwrap();
    assert!(
        progress
            .get("path")
            .is("/servlet/controller.coursemanage.TextBookManageServlet")
    );
    assert!(progress.get("selectorPlacement").is("query"));
    let note = operations
        .array()
        .iter()
        .find(|op| op.get("selector").is("getWrongAnswerNoteResult"))
        .unwrap();
    assert!(note.get("path").is(TESTPAGE_SELECT_PATH) && note.get("selectorPlacement").is("body"));
    // The operation list admits the real wire for each selector.
    assert!(
        http::admit_operation(
            http::ORIGIN,
            TESTPAGE_SELECT_PATH,
            "POST",
            &[],
            &[(
                "p_process".into(),
                "getStudyResultSingleTestingSingleUser".into()
            )]
        )
        .is_some()
    );
    assert!(
        http::admit_operation(
            http::ORIGIN,
            TESTPAGE_SELECT_PATH,
            "POST",
            &[],
            &[("p_process".into(), "somethingElse".into())]
        )
        .is_none()
    );
}
