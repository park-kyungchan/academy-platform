//! Textbook section index, merge, title pattern and section grading source
//! APIs (E10a-2). The TypeScript read path is the reference:
//! apps/academy/dc-gang-a/src/lms/academyReadProtocol.ts (`executeAcademyRead`
//! textbook branch, `parseTextbookScopeBinding`), textbookSectionIndex.ts
//! (title pattern and merge), academyHttpClient.ts (`inspectTextbookIndex`,
//! `readTextbookBindings`, the paper-search hook) and prestudyRead.ts
//! (`parseTextbookBindings`, `isTextbookBookLabel`).
//!
//! Admission (the exact configured student, the managed-name set) stays with
//! the E10b adapter: this layer receives an already admitted
//! `TextbookStudent` and binds every read to that identity. Every wire read
//! goes through a `TextbookSectionPort` method that receives its body bound
//! and request policy; this layer owns every qualification, parse and merge
//! decision after the bytes arrive. JavaScript text rules (whitespace, trim,
//! UTF-16 length, bounded decoding) come only from `crate::js_text`.
use crate::{
    data::{Error, Js, Result, array, hash, object, parse, trim},
    dom::{self, Document},
    js_text::{
        DecodeRefusal, JS_WS_CLASS, decode_bounded, is_js_whitespace, js_collapse_whitespace,
        js_trim, utf16_len,
    },
    owner_admission::TransportScope,
    prestudy::{literal_key, literal_name},
    student_course::{ReadPort, Student, check_contract, field},
    student_metadata::{PaperFormPin, metadata_rows},
    textbook::{FetchPolicy, TextbookResponse},
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::OnceLock,
};

/// Keyed-read body bound in BYTES: TS `#readRequest` streams the body and
/// refuses above 2,000,000 bytes with `source_response_too_large`
/// (academyHttpClient.ts, `#readRequest`); the protocol then also checks
/// `text.length` (UTF-16 units), which a body within the byte bound can never
/// exceed. Used by the progress, wrong-note and detail reads of grading and
/// of the index.
pub const MAX_SECTION_BODY_BYTES: usize = 2_000_000;
/// The UTF-16 unit bound (`text.length > 2000000`) that TS applies to the
/// binding progress read (`textbook_binding_source_rejected`) and to the
/// paper-search pages (`student_paper_*`), which are read without the byte
/// stream cap.
pub const MAX_BODY_UNITS: usize = 2_000_000;
/// Byte ceiling handed to the port for a unit-counted read: a UTF-16 unit
/// needs at most three UTF-8 bytes (a surrogate pair needs four for two).
pub const MAX_UNIT_BODY_BYTES: usize = 3 * MAX_BODY_UNITS;
/// `createAcademyReadPlan`: 1 to 12 unique titles (academyReadProtocol.ts).
pub const MAX_SOURCE_TITLES: usize = 12;
/// `inspectTextbookIndex`: at most 1000 wrong-note rows.
pub const MAX_WRONG_NOTE_ROWS: usize = 1000;
/// `isTextbookBookLabel`: 2 to 200 UTF-16 units after trimming.
pub const MIN_BOOK_LABEL_UNITS: usize = 2;
pub const MAX_BOOK_LABEL_UNITS: usize = 200;
/// Backend hint rule (native-dailyrecord-backend.ts): at most 1000 index
/// rows, each title 1 to 2000 units, else no hint.
pub const MAX_HINT_ROWS: usize = 1000;
pub const MAX_HINT_TITLE_UNITS: usize = 2000;
/// Colspan/rowspan clamp (TS has none; see the declared difference).
pub const MAX_SPAN: usize = 1000;
/// Source path shared by the wrong-note and single-test detail reads.
pub const TESTPAGE_SELECT_PATH: &str = "/servlet/controller.common.TestpageSelectExServlet";

/// Academy reads ride the owner session and never follow a redirect.
pub const SECTION_FETCH: FetchPolicy = FetchPolicy {
    send_credentials: true,
    follow_redirects: false,
};

pub const PAPER_SEARCH_COVERAGE: &str = "one_observed_student_search_page_not_complete_history";
pub const INDEX_COVERAGE_WITH_PAPER_SEARCH: &str =
    "wrong_answer_note_index_plus_one_observed_student_paper_search_page_not_complete_history";
pub const INDEX_COVERAGE_WRONG_NOTE_ONLY: &str =
    "wrong_answer_note_index_only_paper_search_not_observed_not_complete_history";
pub const BY_NAME_COVERAGE_WITH_PAPER_SEARCH: &str = "wrong_answer_note_index_plus_one_observed_student_paper_search_page_for_exact_student_course_book_not_section_grading_not_complete_history";
pub const BY_NAME_COVERAGE_WRONG_NOTE_ONLY: &str = "wrong_answer_note_title_index_only_for_exact_student_course_book_not_section_grading_not_complete_history";
pub const GRADING_COVERAGE: &str = "exact_selected_existing_records_not_whole_book";
pub const UNKNOWN_STATE_PAPER_SEARCH_OBSERVED: &str =
    "student_paper_search_one_observed_page_not_complete_history";
pub const UNKNOWN_STATE_PAPER_SEARCH_UNAVAILABLE: &str =
    "student_paper_search_unavailable_wrong_note_index_only";
pub const NOT_IN_EITHER_CODE: &str = "textbook_section_not_in_either_observed_source";
pub const TITLE_CONFLICT_CODE: &str = "textbook_section_title_conflict_for_attempt";
pub const INTERVIEW_REASON: &str = "multiple_book_bindings_select_one";

/// Length and digest only; used by every hand-written `Debug` below.
struct Redacted<'a>(&'a str);
impl fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<utf16_len {} sha256 {}>",
            utf16_len(self.0),
            &hash(self.0.as_bytes())[..12]
        )
    }
}
struct RedactedOpt<'a>(&'a Option<String>);
impl fmt::Debug for RedactedOpt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(v) => Redacted(v).fmt(f),
            None => f.write_str("None"),
        }
    }
}

fn refuse<T>(code: &str) -> Result<T> {
    Err(Error::refusal(code))
}

// ---------------------------------------------------------------- inputs

/// The admitted student whose identity every read of this layer is bound to.
/// Admission itself (exact configured student, managed names) is E10b's.
#[derive(Clone, PartialEq, Eq)]
pub struct TextbookStudent {
    student_key: String,
    student_name: String,
}
impl TextbookStudent {
    /// `readLiteralKey` and `readLiteralName` (academyReadProtocol.ts:42-49).
    pub fn new(student_key: &str, student_name: &str) -> Result<Self> {
        Ok(Self {
            student_key: literal_key(student_key)?,
            student_name: literal_name(student_name)?,
        })
    }
    pub fn student_key(&self) -> &str {
        &self.student_key
    }
    pub fn student_name(&self) -> &str {
        &self.student_name
    }
}
impl fmt::Debug for TextbookStudent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextbookStudent")
            .field("student_key", &Redacted(&self.student_key))
            .field("student_name", &Redacted(&self.student_name))
            .finish()
    }
}

/// The exact (course, book) scope of one binding.
#[derive(Clone, PartialEq, Eq)]
pub struct BookScope {
    course_key: String,
    book_no: String,
}
impl BookScope {
    pub fn new(course_key: &str, book_no: &str) -> Result<Self> {
        Ok(Self {
            course_key: literal_key(course_key)?,
            book_no: literal_key(book_no)?,
        })
    }
    pub fn course_key(&self) -> &str {
        &self.course_key
    }
    pub fn book_no(&self) -> &str {
        &self.book_no
    }
}
impl fmt::Debug for BookScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BookScope")
            .field("course_key", &Redacted(&self.course_key))
            .field("book_no", &Redacted(&self.book_no))
            .finish()
    }
}

// ----------------------------------------------------------------- port

/// Owner port for the textbook section reads. Implementations own
/// authentication, the transport and the admitted operation contracts; each
/// method receives its body bound and policy and must not attach more than the
/// policy allows. The layer refuses a response whose reported applied policy
/// differs from the requested one.
pub trait TextbookSectionPort {
    /// `getTextBookProgress` with the student name only.
    fn progress(
        &mut self,
        student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
    /// `getWrongAnswerNoteResult` for the exact course, book and student.
    fn wrong_notes(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
    /// `getStudyResultSingleTestingSingleUser` for one attempt.
    fn detail(
        &mut self,
        student: &TextbookStudent,
        attempt_key: &str,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
    /// The student's one observed paper-search result page with all three
    /// paper types selected (the port performs the form read, the form pin
    /// check and the owner check and returns the result page).
    fn paper_search(
        &mut self,
        student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse>;
}

// ------------------------------------------------------------- DTO types

#[derive(Clone, PartialEq, Eq)]
pub struct TextbookBinding {
    pub course_key: String,
    pub book_no: String,
    pub student_key: String,
    pub book_label: String,
}
impl fmt::Debug for TextbookBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextbookBinding")
            .field("course_key", &Redacted(&self.course_key))
            .field("book_no", &Redacted(&self.book_no))
            .field("student_key", &Redacted(&self.student_key))
            .field("book_label", &Redacted(&self.book_label))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexSource {
    WrongAnswerNote,
    StudentPaperSearch,
}
impl IndexSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WrongAnswerNote => "wrong_answer_note",
            Self::StudentPaperSearch => "student_paper_search",
        }
    }
}

/// One merged section row (`TextbookSectionRow`, textbookSectionIndex.ts).
#[derive(Clone, PartialEq, Eq)]
pub struct IndexRow {
    pub source_title: String,
    pub paper_key: String,
    pub attempt_key: String,
    pub source_date_literal: Option<String>,
    pub sources: Vec<IndexSource>,
}
impl fmt::Debug for IndexRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexRow")
            .field("source_title", &Redacted(&self.source_title))
            .field("paper_key", &Redacted(&self.paper_key))
            .field("attempt_key", &Redacted(&self.attempt_key))
            .field(
                "source_date_literal",
                &RedactedOpt(&self.source_date_literal),
            )
            .field("sources", &self.sources)
            .finish()
    }
}

/// One wrong-note row (`getWrongAnswerNoteResult`).
#[derive(Clone, PartialEq, Eq)]
pub struct WrongNoteRow {
    pub source_title: String,
    pub paper_key: String,
    pub attempt_key: String,
    pub source_date_literal: Option<String>,
}
impl fmt::Debug for WrongNoteRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WrongNoteRow")
            .field("source_title", &Redacted(&self.source_title))
            .field("paper_key", &Redacted(&self.paper_key))
            .field("attempt_key", &Redacted(&self.attempt_key))
            .field(
                "source_date_literal",
                &RedactedOpt(&self.source_date_literal),
            )
            .finish()
    }
}

/// One paper-search row (`TextbookPaperSearchRow`).
#[derive(Clone, PartialEq, Eq)]
pub struct PaperSearchRow {
    pub testing_name: String,
    pub paper_key: String,
    pub attempt_key: String,
    pub apply_date_literal: Option<String>,
}
impl fmt::Debug for PaperSearchRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaperSearchRow")
            .field("testing_name", &Redacted(&self.testing_name))
            .field("paper_key", &Redacted(&self.paper_key))
            .field("attempt_key", &Redacted(&self.attempt_key))
            .field("apply_date_literal", &RedactedOpt(&self.apply_date_literal))
            .finish()
    }
}

/// The book's section-title template (`TextbookSectionTitlePattern`): only
/// (activity, book) bracket pairs observed together, plus the student suffix.
#[derive(Clone, PartialEq, Eq)]
pub struct TitlePattern {
    pub pairs: Vec<(String, String)>,
    pub student_suffix: Option<String>,
}
impl fmt::Debug for TitlePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TitlePattern")
            .field("pair_count", &self.pairs.len())
            .field("student_suffix", &RedactedOpt(&self.student_suffix))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexBasis {
    WrongAnswerNotePlusStudentPaperSearch,
    WrongAnswerNoteOnly,
}
impl IndexBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WrongAnswerNotePlusStudentPaperSearch => {
                "wrong_answer_note_plus_student_paper_search"
            }
            Self::WrongAnswerNoteOnly => "wrong_answer_note_only",
        }
    }
    pub fn coverage(self) -> &'static str {
        match self {
            Self::WrongAnswerNotePlusStudentPaperSearch => INDEX_COVERAGE_WITH_PAPER_SEARCH,
            Self::WrongAnswerNoteOnly => INDEX_COVERAGE_WRONG_NOTE_ONLY,
        }
    }
    pub fn by_name_coverage(self) -> &'static str {
        match self {
            Self::WrongAnswerNotePlusStudentPaperSearch => BY_NAME_COVERAGE_WITH_PAPER_SEARCH,
            Self::WrongAnswerNoteOnly => BY_NAME_COVERAGE_WRONG_NOTE_ONLY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperSearchStatus {
    Observed { matched_row_count: usize },
    Unavailable,
    NoDerivableSectionTitlePattern,
}

/// `inspectTextbookIndex` result.
#[derive(Clone, PartialEq, Eq)]
pub struct TextbookIndex {
    pub scope: BookScope,
    pub rows: Vec<IndexRow>,
    pub index_basis: IndexBasis,
    pub coverage: &'static str,
    pub paper_search: PaperSearchStatus,
}
impl fmt::Debug for TextbookIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextbookIndex")
            .field("scope", &self.scope)
            .field("row_count", &self.rows.len())
            .field("index_basis", &self.index_basis)
            .field("coverage", &self.coverage)
            .field("paper_search", &self.paper_search)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scoring {
    Correct,
    Wrong,
}
impl Scoring {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Correct => "O",
            Self::Wrong => "X",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SectionItem {
    pub ordinal: String,
    pub lecture_key: Option<String>,
    pub outer_lecture_key: Option<String>,
    pub scoring: Scoring,
}
impl fmt::Debug for SectionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionItem")
            .field("ordinal", &Redacted(&self.ordinal))
            .field("lecture_key", &RedactedOpt(&self.lecture_key))
            .field("outer_lecture_key", &RedactedOpt(&self.outer_lecture_key))
            .field("scoring", &self.scoring)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourceDate {
    pub literal: Option<String>,
    /// `month_day` for a `dd-dd` literal, else `unknown`.
    pub precision: &'static str,
    pub meaning: &'static str,
}
impl fmt::Debug for SourceDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceDate")
            .field("literal", &RedactedOpt(&self.literal))
            .field("precision", &self.precision)
            .finish()
    }
}

/// One graded section (`StoredSourceRecord`, textbook branch).
#[derive(Clone, PartialEq)]
pub struct SectionRecord {
    pub source_title: String,
    pub student_key: String,
    pub student_name: String,
    pub course_key: String,
    pub book_no: String,
    pub paper_key: String,
    pub attempt_key: String,
    pub score: f64,
    pub items: Vec<SectionItem>,
    pub source_date: SourceDate,
}
impl SectionRecord {
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn correct_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.scoring == Scoring::Correct)
            .count()
    }
}
impl fmt::Debug for SectionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionRecord")
            .field("source_title", &Redacted(&self.source_title))
            .field("student_key", &Redacted(&self.student_key))
            .field("student_name", &Redacted(&self.student_name))
            .field("course_key", &Redacted(&self.course_key))
            .field("book_no", &Redacted(&self.book_no))
            .field("paper_key", &Redacted(&self.paper_key))
            .field("attempt_key", &Redacted(&self.attempt_key))
            .field("item_count", &self.items.len())
            .field("source_date", &self.source_date)
            .finish()
    }
}

/// Graded records sorted by title (JavaScript default string order).
#[derive(Clone, PartialEq)]
pub struct GradedSections {
    pub records: Vec<SectionRecord>,
    pub coverage: &'static str,
    pub paper_search_observed: bool,
    /// The extra `unknownStates` literal of the paper-search basis.
    pub unknown_state: &'static str,
}
impl fmt::Debug for GradedSections {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GradedSections")
            .field("record_count", &self.records.len())
            .field("coverage", &self.coverage)
            .field("paper_search_observed", &self.paper_search_observed)
            .finish()
    }
}

/// Step 5: the typed absent-title outcome. It exists only when the paper
/// search was observed and a title pattern was derivable; coverage and the
/// public status object are E10b's (the literals are exported above).
#[derive(Clone, PartialEq, Eq)]
pub struct NotObservedInEither {
    pub source_title: String,
}
impl fmt::Debug for NotObservedInEither {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotObservedInEither")
            .field("source_title", &Redacted(&self.source_title))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GradeOutcome {
    Graded(GradedSections),
    NotObservedInEither(NotObservedInEither),
}

#[derive(Clone, PartialEq, Eq)]
pub struct InterviewCandidate {
    pub book_label: String,
    pub book_no: String,
    /// The first observed section title of that binding; absent when its
    /// index read failed or violated the hint rule.
    pub hint: Option<String>,
}
impl fmt::Debug for InterviewCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InterviewCandidate")
            .field("book_label", &Redacted(&self.book_label))
            .field("book_no", &Redacted(&self.book_no))
            .field("hint", &RedactedOpt(&self.hint))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interview {
    pub bindings: Vec<InterviewCandidate>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct IndexByName {
    pub scope: BookScope,
    pub book_label: String,
    pub index: TextbookIndex,
}
impl fmt::Debug for IndexByName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexByName")
            .field("scope", &self.scope)
            .field("book_label", &Redacted(&self.book_label))
            .field("index", &self.index)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct GradedByName {
    pub scope: BookScope,
    pub book_label: String,
    pub outcome: GradeOutcome,
}
impl fmt::Debug for GradedByName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GradedByName")
            .field("scope", &self.scope)
            .field("book_label", &Redacted(&self.book_label))
            .field("outcome", &self.outcome)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SectionsByName {
    Interview(Interview),
    Index(IndexByName),
    Graded(GradedByName),
}

// ------------------------------------------------------ JS value helpers

fn read_input_record(v: &Js) -> Result<&Js> {
    if object(v) {
        Ok(v)
    } else {
        refuse("invalid_read_request")
    }
}

fn parse_json(text: &str) -> Result<Js> {
    // TS surfaces the raw SyntaxError; this layer uses one catalogued code.
    parse(text).map_err(|_| Error::refusal("source_json_rejected"))
}

const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

fn safe_positive_integer(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0 && n > 0.0 && n <= MAX_SAFE
}

/// `sourceResponseKey` (academyReadProtocol.ts:158): a safe positive integer
/// number, else `readLiteralKey`.
fn source_response_key(v: &Js) -> Result<String> {
    if let Js::Num(n) = v
        && safe_positive_integer(*n)
    {
        return Ok(format!("{n:.0}"));
    }
    if v.string() {
        let s = String::from_utf16(v.units()).map_err(|_| Error::refusal("invalid_read_key"))?;
        return literal_key(&s);
    }
    refuse("invalid_read_key")
}

/// `sourceKey` (assessmentReadProtocol.ts:248): the lenient score-view key.
fn source_key(v: &Js) -> Result<String> {
    if let Js::Num(n) = v
        && safe_positive_integer(*n)
    {
        return Ok(format!("{n:.0}"));
    }
    if v.string() {
        let units = v.units();
        if !units.is_empty()
            && units.iter().all(|u| (0x30..=0x39).contains(u))
            && !units.iter().all(|u| *u == 0x30)
        {
            return Ok(String::from_utf16_lossy(units));
        }
    }
    refuse("invalid_source_key")
}

fn name_ok(v: &Js) -> bool {
    v.string() && !trim(v.units()).is_empty() && v.units().len() <= 200
}

/// `readLiteralName` over a JSON value (invalid_read_name).
fn read_literal_name_value(v: &Js) -> Result<String> {
    if !v.string() {
        return refuse("invalid_read_name");
    }
    let s = String::from_utf16(v.units()).map_err(|_| Error::refusal("invalid_read_name"))?;
    literal_name(&s)
}

fn optional_string(v: &Js) -> Option<String> {
    if v.string() {
        String::from_utf16(v.units()).ok()
    } else {
        None
    }
}

// ------------------------------------------------- body text and refusals

fn session_page_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // TS `#readRequest`: webUserPass, the expiry text, a password input
        // or a loginForm, ASCII case-insensitive, ASCII `\b`, JS `\s`.
        regex::Regex::new(&format!(
            r#"(?i-u:webUserPass)|세션이 만료|(?i-u:<input\b)[^>]*(?i-u:\btype){ws}*={ws}*["']?(?i-u:password\b)|(?i-u:<form\b)[^>]*(?i-u:\bname){ws}*={ws}*["']?(?i-u:loginForm\b)"#,
            ws = JS_WS_CLASS
        ))
        .expect("constant")
    })
}

fn expired_binding_page_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // prestudyRead.ts:20 `rejectExpired`.
        regex::Regex::new(&format!(
            r#"(?i-u:normal_session_error\.jsp|popup_session_error\.jsp)|세션이 만료|(?i-u:<input\b)[^>]*(?i-u:type){ws}*={ws}*["']?(?i-u:password)"#,
            ws = JS_WS_CLASS
        ))
        .expect("constant")
    })
}

fn applied_policy_ok(response: &TextbookResponse, policy: FetchPolicy) -> bool {
    response.credentials_sent == policy.send_credentials
        && response.redirect_followed == policy.follow_redirects
}

/// TS `post` and `#readRequest` for a keyed read: the response must be an
/// OK (2xx), unredirected one (`source_transport_failed`); the body is
/// bounded in bytes (`source_response_too_large`) and then in units; a
/// session page is `authentication_required`.
fn keyed_text(response: TextbookResponse) -> Result<String> {
    if !(200..=299).contains(&response.status) || response.redirected {
        return refuse("source_transport_failed");
    }
    let text = decode_bounded(&response.bytes, MAX_SECTION_BODY_BYTES, MAX_BODY_UNITS).map_err(
        |refusal| {
            Error::refusal(match refusal {
                DecodeRefusal::TooLarge => "source_response_too_large",
                DecodeRefusal::NotUtf8 => "source_response_encoding_rejected",
            })
        },
    )?;
    if session_page_re().is_match(&text) {
        return refuse("authentication_required");
    }
    Ok(text)
}

/// `readTextbookBindings`: the progress body is bounded in UTF-16 units
/// (`textbook_binding_source_rejected`) and has no byte stream cap in TS.
fn binding_text(response: TextbookResponse) -> Result<String> {
    if !(200..=299).contains(&response.status) || response.redirected {
        return refuse("source_transport_failed");
    }
    decode_bounded(&response.bytes, MAX_UNIT_BODY_BYTES, MAX_BODY_UNITS).map_err(|refusal| {
        Error::refusal(match refusal {
            DecodeRefusal::TooLarge => "textbook_binding_source_rejected",
            DecodeRefusal::NotUtf8 => "source_response_encoding_rejected",
        })
    })
}

/// The paper-search page body (`student_paper_response_too_large`, units).
fn paper_text(response: TextbookResponse) -> Result<String> {
    if !(200..=299).contains(&response.status) || response.redirected {
        return refuse("source_transport_failed");
    }
    decode_bounded(&response.bytes, MAX_UNIT_BODY_BYTES, MAX_BODY_UNITS).map_err(|refusal| {
        Error::refusal(match refusal {
            DecodeRefusal::TooLarge => "student_paper_response_too_large",
            DecodeRefusal::NotUtf8 => "source_response_encoding_rejected",
        })
    })
}

// ------------------------------------------------------- scope binding

fn call_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(&format!(
            r#"createBookIncorrectPapers\({ws}*['"]?([0-9]+)['"]?{ws}*,{ws}*['"]?([0-9]+)['"]?{ws}*,{ws}*['"]?([0-9]+)['"]?{ws}*\)"#,
            ws = JS_WS_CLASS
        ))
        .expect("constant")
    })
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// JS `/\bonclick\s*=\s*(["'])([\s\S]*?)\1/gi` over the raw text; returns each
/// attribute value in order. A failed attempt moves one byte on; a match
/// resumes after its end, as `matchAll` does.
fn raw_onclick_values(html: &str) -> Vec<&str> {
    let bytes = html.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i + 7 <= bytes.len() {
        let boundary = i == 0 || !is_word_byte(bytes[i - 1]);
        if boundary && bytes[i..i + 7].eq_ignore_ascii_case(b"onclick") {
            let skip_ws = |mut p: usize| {
                while let Some(c) = html.get(p..).and_then(|s| s.chars().next()) {
                    if is_js_whitespace(c) {
                        p += c.len_utf8();
                    } else {
                        break;
                    }
                }
                p
            };
            let p = skip_ws(i + 7);
            if bytes.get(p) == Some(&b'=') {
                let p = skip_ws(p + 1);
                if let Some(&quote) = bytes.get(p)
                    && (quote == b'"' || quote == b'\'')
                    && let Some(end) = bytes[p + 1..].iter().position(|b| *b == quote)
                {
                    out.push(&html[p + 1..p + 1 + end]);
                    i = p + 1 + end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// `parseTextbookScopeBinding`: the set of distinct exact
/// (course, book, student) bindings on the progress page must have exactly
/// one member. TS keeps them in a `Set`, so an identical button repeated is
/// still one binding.
fn require_scope_binding(html: &str, scope: &BookScope, student: &TextbookStudent) -> Result<()> {
    let mut found = HashSet::new();
    for attribute in raw_onclick_values(html) {
        let source = attribute
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&quot;", "\"");
        if let Some(call) = call_re().captures(&source)
            && call[1] == scope.course_key
            && call[2] == scope.book_no
            && call[3] == student.student_key
        {
            found.insert((
                call[1].to_string(),
                call[2].to_string(),
                call[3].to_string(),
            ));
        }
    }
    if found.len() == 1 {
        Ok(())
    } else {
        refuse("exact_textbook_scope_not_observed")
    }
}

// ---------------------------------------------------- binding discovery

/// `isTextbookBookLabel` (prestudyRead.ts:204): a title, never a progress,
/// date, number or button cell.
pub fn is_textbook_book_label(value: &str) -> bool {
    static NUMERIC: OnceLock<regex::Regex> = OnceLock::new();
    static DATES: OnceLock<[regex::Regex; 2]> = OnceLock::new();
    static WRONG_NOTE: OnceLock<regex::Regex> = OnceLock::new();
    let v = js_trim(value);
    let units = utf16_len(v);
    if !(MIN_BOOK_LABEL_UNITS..=MAX_BOOK_LABEL_UNITS).contains(&units) || v.contains('%') {
        return false;
    }
    let numeric = NUMERIC.get_or_init(|| {
        regex::Regex::new(&format!(r"^(?:[0-9.,]|{JS_WS_CLASS})+$")).expect("constant")
    });
    if numeric.is_match(v) {
        return false;
    }
    let dates = DATES.get_or_init(|| {
        [
            regex::Regex::new(r"^[0-9]{4}[-./][0-9]{1,2}[-./][0-9]{1,2}(?:[^0-9]|$)")
                .expect("constant"),
            regex::Regex::new(r"^[0-9]{1,2}[-./][0-9]{1,2}[-./][0-9]{2,4}(?:[^0-9]|$)")
                .expect("constant"),
        ]
    });
    if dates.iter().any(|re| re.is_match(v)) {
        return false;
    }
    let wrong_note = WRONG_NOTE
        .get_or_init(|| regex::Regex::new("오답노트|(?i-u:incorrect|wrong)").expect("constant"));
    if wrong_note.is_match(v) {
        return false;
    }
    v.chars()
        .any(|c| matches!(c, '가'..='힣' | 'A'..='Z' | 'a'..='z'))
}

/// JavaScript `Number(text)` for the span attributes (decimal, signed
/// decimal, `Infinity`, `0x`/`0o`/`0b`, empty is 0, anything else NaN).
fn js_number(value: &str) -> f64 {
    static DECIMAL: OnceLock<regex::Regex> = OnceLock::new();
    let t = js_trim(value);
    if t.is_empty() {
        return 0.0;
    }
    let decimal = DECIMAL.get_or_init(|| {
        regex::Regex::new(r"^[+-]?(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
            .expect("constant")
    });
    if decimal.is_match(t) {
        return t.parse::<f64>().unwrap_or(f64::NAN);
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let (radix, digits) = match t.get(..2) {
        Some("0x" | "0X") => (16, &t[2..]),
        Some("0o" | "0O") => (8, &t[2..]),
        Some("0b" | "0B") => (2, &t[2..]),
        _ => return f64::NAN,
    };
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut total = 0.0;
    for c in digits.chars() {
        match c.to_digit(radix) {
            Some(d) => total = total * f64::from(radix) + f64::from(d),
            None => return f64::NAN,
        }
    }
    total
}

/// `Math.max(1, Number(attr ?? 1) || 1)`, clamped to `MAX_SPAN`.
fn span_of(attribute: Option<&str>) -> f64 {
    let n = attribute.map_or(1.0, js_number);
    let n = if n.is_nan() || n == 0.0 { 1.0 } else { n };
    n.clamp(1.0, MAX_SPAN as f64)
}

#[derive(Clone)]
struct GridCell {
    text: String,
    attrs: Vec<String>,
}

struct Grid<'a> {
    rows: Vec<scraper::ElementRef<'a>>,
    cells: Vec<Vec<GridCell>>,
    header_row: Option<usize>,
}

/// `tableGrid` (prestudyRead.ts:219): rowspan/colspan are expanded so a
/// shifted physical row keeps its logical columns.
fn table_grid(table: scraper::ElementRef<'_>) -> Grid<'_> {
    let rows = dom::select(
        table,
        ":scope > tbody > tr, :scope > thead > tr, :scope > tr",
    );
    let mut cells: Vec<Vec<GridCell>> = vec![];
    let mut carry: HashMap<usize, (GridCell, f64)> = HashMap::new();
    let mut header_row = None;
    for (ri, tr) in rows.iter().enumerate() {
        let mut out: Vec<GridCell> = vec![];
        let place_carried =
            |out: &mut Vec<GridCell>, carry: &mut HashMap<usize, (GridCell, f64)>| {
                while let Some((cell, left)) = carry.get_mut(&out.len()) {
                    let column = out.len();
                    out.push(cell.clone());
                    *left -= 1.0;
                    if *left <= 0.0 {
                        carry.remove(&column);
                    }
                }
            };
        for node in dom::select(*tr, ":scope > th, :scope > td") {
            place_carried(&mut out, &mut carry);
            let attrs = dom::select(node, "input, img")
                .iter()
                .flat_map(|n| {
                    ["value", "title", "alt"]
                        .iter()
                        .map(|a| js_collapse_whitespace(n.value().attr(a).unwrap_or("")))
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty())
                .collect();
            let cell = GridCell {
                text: js_collapse_whitespace(&dom::content(node)),
                attrs,
            };
            let width = span_of(node.value().attr("colspan")).ceil() as usize;
            let down = span_of(node.value().attr("rowspan"));
            for _ in 0..width {
                let column = out.len();
                out.push(cell.clone());
                if down > 1.0 {
                    carry.insert(column, (cell.clone(), down - 1.0));
                }
            }
            if node.value().name() == "th" && header_row.is_none() {
                header_row = Some(ri);
            }
        }
        place_carried(&mut out, &mut carry);
        cells.push(out);
    }
    Grid {
        rows,
        cells,
        header_row,
    }
}

/// `parseTextbookBindings` (prestudyRead.ts:243): the registered bindings of
/// one student from the `getTextBookProgress` rows, in first-observed order.
/// The label is chosen from the logical table grid: the header-named book
/// column, else a branded label, else the first non-UI title cell.
pub fn parse_textbook_bindings(html: &str, student_key: &str) -> Result<Vec<TextbookBinding>> {
    if expired_binding_page_re().is_match(html) {
        return refuse("prestudy_authentication_required");
    }
    if utf16_len(html) > MAX_BODY_UNITS {
        return refuse("prestudy_source_rejected");
    }
    static HEADER: OnceLock<regex::Regex> = OnceLock::new();
    static BRANDED: OnceLock<regex::Regex> = OnceLock::new();
    let header = HEADER.get_or_init(|| regex::Regex::new("교재|도서").expect("constant"));
    let branded =
        BRANDED.get_or_init(|| regex::Regex::new("가우스|다빈치|개념원리").expect("constant"));
    const UI_WORDS: [&str; 12] = [
        "생성", "보기", "확인", "닫기", "수정", "삭제", "출력", "인쇄", "검색", "등록", "저장",
        "취소",
    ];
    let doc = Document::parse(html);
    let mut order: Vec<(String, String)> = vec![];
    let mut found: HashMap<(String, String), TextbookBinding> = HashMap::new();
    let mut grids: HashMap<_, Grid<'_>> = HashMap::new();
    for node in doc.select("[onclick]") {
        let Some(call) = call_re().captures(node.value().attr("onclick").unwrap_or("")) else {
            continue;
        };
        if &call[3] != student_key {
            continue;
        }
        let row = if node.value().name() == "tr" {
            Some(node)
        } else {
            dom::closest(node, "tr")
        };
        let table = row.and_then(|r| dom::closest(r, "table"));
        let (Some(row), Some(table)) = (row, table) else {
            return refuse("textbook_binding_source_unqualified");
        };
        let grid = grids.entry(table.id()).or_insert_with(|| table_grid(table));
        let grid_row: &[GridCell] = grid
            .rows
            .iter()
            .position(|r| r.id() == row.id())
            .and_then(|i| grid.cells.get(i))
            .map_or(&[], |v| v.as_slice());
        let header_cells: &[GridCell] = grid
            .header_row
            .and_then(|i| grid.cells.get(i))
            .map_or(&[], |v| v.as_slice());
        let book_col = header_cells.iter().position(|c| header.is_match(&c.text));
        let at = |c: Option<&GridCell>| -> Option<String> {
            let c = c?;
            if is_textbook_book_label(&c.text) {
                return Some(c.text.clone());
            }
            c.attrs.iter().find(|a| is_textbook_book_label(a)).cloned()
        };
        let from_header = book_col.and_then(|i| at(grid_row.get(i)));
        let qualified: Vec<String> = grid_row
            .iter()
            .skip(1)
            .filter_map(|c| at(Some(c)))
            .collect();
        let book_label = from_header
            .or_else(|| qualified.iter().find(|c| branded.is_match(c)).cloned())
            .or_else(|| {
                qualified
                    .iter()
                    .find(|c| !UI_WORDS.contains(&c.as_str()))
                    .cloned()
            })
            .or_else(|| qualified.first().cloned());
        let Some(book_label) = book_label else {
            return refuse("textbook_binding_source_unqualified");
        };
        let binding = TextbookBinding {
            course_key: call[1].to_string(),
            book_no: call[2].to_string(),
            student_key: student_key.to_string(),
            book_label,
        };
        let key = (binding.course_key.clone(), binding.book_no.clone());
        if let Some(prior) = found.get(&key) {
            if prior.book_label != binding.book_label {
                return refuse("textbook_binding_source_unqualified");
            }
        } else {
            order.push(key.clone());
        }
        found.insert(key, binding);
    }
    if order.is_empty() {
        return refuse("textbook_binding_source_unqualified");
    }
    Ok(order
        .into_iter()
        .map(|key| found.remove(&key).expect("recorded"))
        .collect())
}

/// `#assertTextbookBindings` (native-dailyrecord-backend.ts:246): the backend
/// half of the binding contract.
fn assert_bindings(bindings: &[TextbookBinding], student_key: &str) -> Result<()> {
    if bindings.is_empty() {
        return refuse("textbook_binding_source_unqualified");
    }
    let mut identities = HashSet::new();
    for binding in bindings {
        if binding.student_key != student_key {
            return refuse("textbook_binding_scope_conflict");
        }
        if !is_textbook_book_label(&binding.book_label) {
            return refuse("textbook_binding_source_unqualified");
        }
        if !identities.insert((binding.course_key.as_str(), binding.book_no.as_str())) {
            return refuse("textbook_binding_source_unqualified");
        }
    }
    Ok(())
}

// --------------------------------------------------- title pattern, merge

/// Leading `[token]` groups (each followed by JS whitespace) and the rest.
fn bracket_tokens(title: &str) -> (Vec<&str>, &str) {
    let mut tokens = vec![];
    let mut rest = title;
    while let Some(inner) = rest.strip_prefix('[') {
        let Some(end) = inner.find(']') else { break };
        if end == 0 {
            break;
        }
        tokens.push(&inner[..end]);
        rest = inner[end + 1..].trim_start_matches(is_js_whitespace);
    }
    (tokens, rest)
}

/// `deriveTextbookSectionTitlePattern`: `None` when the template cannot
/// anchor paper rows to this book (no titles, or a title without an
/// activity and book bracket pair).
pub fn derive_title_pattern(
    wrong_note_titles: &[String],
    student_name: &str,
) -> Option<TitlePattern> {
    if wrong_note_titles.is_empty() {
        return None;
    }
    let token_lists: Vec<Vec<&str>> = wrong_note_titles
        .iter()
        .map(|t| bracket_tokens(t).0)
        .collect();
    if token_lists.iter().any(|tokens| tokens.len() < 2) {
        return None;
    }
    let mut seen = HashSet::new();
    let mut pairs = vec![];
    for tokens in &token_lists {
        if seen.insert((tokens[0], tokens[1])) {
            pairs.push((tokens[0].to_string(), tokens[1].to_string()));
        }
    }
    let suffix = format!(" - {student_name}");
    let all_suffixed = wrong_note_titles.iter().all(|t| t.ends_with(&suffix));
    Some(TitlePattern {
        pairs,
        student_suffix: all_suffixed.then_some(suffix),
    })
}

/// `isTextbookSectionPaperTitle`.
pub fn is_section_paper_title(title: &str, pattern: &TitlePattern) -> bool {
    let (tokens, rest) = bracket_tokens(title);
    if tokens.len() < 2 {
        return false;
    }
    if !pattern
        .pairs
        .iter()
        .any(|(activity, book)| activity == tokens[0] && book == tokens[1])
    {
        return false;
    }
    if let Some(suffix) = &pattern.student_suffix
        && !title.ends_with(suffix.as_str())
    {
        return false;
    }
    // A bare template echo with no section-specific remainder is not a section.
    let remainder = js_trim(rest);
    let echo = js_trim(pattern.student_suffix.as_deref().unwrap_or(""));
    !remainder.is_empty() && remainder != echo
}

/// `mergeTextbookSectionRows`: wrong-note rows always stay; a shared attempt
/// key must carry the same title in both sources (else
/// `textbook_section_title_conflict_for_attempt`); a paper row joins only when
/// its title matches the pattern.
pub fn merge_section_rows(
    wrong_rows: &[WrongNoteRow],
    paper_rows: &[PaperSearchRow],
    pattern: Option<&TitlePattern>,
) -> Result<Vec<IndexRow>> {
    let mut merged: Vec<IndexRow> = wrong_rows
        .iter()
        .map(|r| IndexRow {
            source_title: r.source_title.clone(),
            paper_key: r.paper_key.clone(),
            attempt_key: r.attempt_key.clone(),
            source_date_literal: r.source_date_literal.clone(),
            sources: vec![IndexSource::WrongAnswerNote],
        })
        .collect();
    // A `Map` built from the rows: a later duplicate attempt key wins.
    let mut by_attempt: HashMap<String, usize> = HashMap::new();
    for (index, row) in merged.iter().enumerate() {
        by_attempt.insert(row.attempt_key.clone(), index);
    }
    for paper in paper_rows {
        if let Some(&index) = by_attempt.get(&paper.attempt_key) {
            if merged[index].source_title != paper.testing_name {
                return refuse(TITLE_CONFLICT_CODE);
            }
            if !merged[index]
                .sources
                .contains(&IndexSource::StudentPaperSearch)
            {
                merged[index].sources = vec![
                    IndexSource::WrongAnswerNote,
                    IndexSource::StudentPaperSearch,
                ];
            }
            continue;
        }
        let Some(pattern) = pattern else { continue };
        if !is_section_paper_title(&paper.testing_name, pattern) {
            continue;
        }
        by_attempt.insert(paper.attempt_key.clone(), merged.len());
        merged.push(IndexRow {
            source_title: paper.testing_name.clone(),
            paper_key: paper.paper_key.clone(),
            attempt_key: paper.attempt_key.clone(),
            source_date_literal: paper.apply_date_literal.clone(),
            sources: vec![IndexSource::StudentPaperSearch],
        });
    }
    Ok(merged)
}

// ------------------------------------------------------ wrong-note rows

/// The contract part of the wrong-note read: `result == "ok"` and a
/// `noteResultList` array (`wrong_note_contract_rejected`), with the
/// `inspectTextbookIndex` row cap when `strict`.
fn wrong_note_list(text: &str, strict: bool) -> Result<Vec<Js>> {
    let data = parse_json(text)?;
    read_input_record(&data)?;
    let list = data.get("noteResultList");
    if !data.get("result").is("ok")
        || !array(list)
        || (strict && list.array().len() > MAX_WRONG_NOTE_ROWS)
    {
        return refuse("wrong_note_contract_rejected");
    }
    Ok(list.array().to_vec())
}

fn check_subject(row: &Js, student: &TextbookStudent) -> Result<()> {
    read_input_record(row)?;
    if source_response_key(row.get("pri_no"))? != student.student_key
        || !row.get("name").is(&student.student_name)
    {
        return refuse("source_subject_conflict");
    }
    Ok(())
}

/// Convert the rows; `strict` is the `inspectTextbookIndex` path, which also
/// refuses a repeated (paper, attempt) identity (`ambiguous_textbook_index`).
fn wrong_rows(notes: &[Js], student: &TextbookStudent, strict: bool) -> Result<Vec<WrongNoteRow>> {
    let mut identities = HashSet::new();
    let mut rows = vec![];
    for row in notes {
        check_subject(row, student)?;
        let paper_key = source_response_key(row.get("p_no"))?;
        let attempt_key = source_response_key(row.get("testing_no"))?;
        if strict && !identities.insert((paper_key.clone(), attempt_key.clone())) {
            return refuse("ambiguous_textbook_index");
        }
        rows.push(WrongNoteRow {
            source_title: read_literal_name_value(row.get("testing_name"))?,
            paper_key,
            attempt_key,
            source_date_literal: optional_string(row.get("apply_date")),
        });
    }
    Ok(rows)
}

fn plain_rows(rows: &[WrongNoteRow]) -> Vec<IndexRow> {
    rows.iter()
        .map(|r| IndexRow {
            source_title: r.source_title.clone(),
            paper_key: r.paper_key.clone(),
            attempt_key: r.attempt_key.clone(),
            source_date_literal: r.source_date_literal.clone(),
            sources: vec![IndexSource::WrongAnswerNote],
        })
        .collect()
}

// -------------------------------------------------------------- detail

fn parse_detail(
    text: &str,
    student: &TextbookStudent,
    title: &str,
) -> Result<(f64, Vec<SectionItem>)> {
    let data = parse_json(text)?;
    read_input_record(&data)?;
    // parseScoreView (assessmentReadProtocol.ts:266)
    let print_list = data.get("print_list");
    if !data.get("result").is("OK") || !array(print_list) || print_list.array().len() != 1 {
        return refuse("ambiguous_captured_result");
    }
    let paper = &print_list.array()[0];
    if !object(paper) {
        return refuse("invalid_capture_shape");
    }
    if source_key(paper.get("pri_no"))? != student.student_key {
        return refuse("captured_subject_mismatch");
    }
    if !name_ok(paper.get("student_name")) {
        return refuse("invalid_student_name");
    }
    if !paper.get("student_name").is(&student.student_name) {
        return refuse("captured_subject_mismatch");
    }
    let score = match paper.get("score") {
        Js::Num(n) if n.is_finite() && (0.0..=100.0).contains(n) => *n,
        _ => return refuse("invalid_captured_score"),
    };
    let exam_list = paper.get("exam_list");
    if !array(exam_list) || exam_list.array().is_empty() {
        return refuse("captured_items_missing");
    }
    let mut seen = HashSet::new();
    for item in exam_list.array() {
        if !object(item) {
            return refuse("invalid_capture_shape");
        }
        if !seen.insert(source_key(item.get("exam_no"))?) {
            return refuse("duplicate_captured_item");
        }
        if !item.get("scoring_result").is("O") && !item.get("scoring_result").is("X") {
            return refuse("unknown_captured_scoring");
        }
    }
    // executeAcademyRead: the sub-title must equal the requested title.
    if !paper.get("sub_title").is(title) {
        return refuse("source_title_conflict");
    }
    let mut items = vec![];
    for item in exam_list.array() {
        let optional_key = |v: &Js| -> Result<Option<String>> {
            if v.absent() {
                Ok(None)
            } else {
                source_response_key(v).map(Some)
            }
        };
        items.push(SectionItem {
            ordinal: source_response_key(item.get("exam_no"))?,
            lecture_key: optional_key(item.get("lecture_key"))?,
            outer_lecture_key: optional_key(item.get("outer_lecture_key"))?,
            scoring: if item.get("scoring_result").is("O") {
                Scoring::Correct
            } else {
                Scoring::Wrong
            },
        });
    }
    items.sort_by(|a, b| {
        let number = |s: &str| s.parse::<f64>().unwrap_or(f64::NAN);
        number(&a.ordinal)
            .partial_cmp(&number(&b.ordinal))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok((score, items))
}

fn source_date(literal: &Option<String>) -> SourceDate {
    let month_day = literal.as_ref().is_some_and(|l| {
        let b = l.as_bytes();
        b.len() == 5
            && b[2] == b'-'
            && b[..2].iter().all(u8::is_ascii_digit)
            && b[3..].iter().all(u8::is_ascii_digit)
    });
    SourceDate {
        literal: literal.clone(),
        precision: if month_day { "month_day" } else { "unknown" },
        meaning: "source_apply_date_not_exact_grade_time",
    }
}

// --------------------------------------------------------------- source

/// `createAcademyReadPlan` for the textbook family: 1 to 12 unique titles,
/// each a literal name.
fn validate_titles(titles: &[String]) -> Result<()> {
    let unique: HashSet<&String> = titles.iter().collect();
    if titles.is_empty() || titles.len() > MAX_SOURCE_TITLES || unique.len() != titles.len() {
        return refuse("invalid_source_selection");
    }
    for title in titles {
        literal_name(title)?;
    }
    Ok(())
}

enum PaperObservation {
    Observed(Vec<PaperSearchRow>),
    Unavailable,
}

pub struct TextbookIndexSource<P: TextbookSectionPort> {
    port: P,
    principal_label: String,
    requests: usize,
}
impl<P: TextbookSectionPort> TextbookIndexSource<P> {
    pub fn new(port: P, principal_label: impl Into<String>) -> Self {
        Self {
            port,
            principal_label: principal_label.into(),
            requests: 0,
        }
    }
    pub fn requests(&self) -> usize {
        self.requests
    }
    fn require_principal(&self, teacher_label: &str, code: &str) -> Result<()> {
        if teacher_label == self.principal_label {
            Ok(())
        } else {
            refuse(code)
        }
    }
    fn keyed_progress(&mut self, student: &TextbookStudent) -> Result<String> {
        let response = self
            .port
            .progress(student, MAX_SECTION_BODY_BYTES, SECTION_FETCH)?;
        self.requests += 1;
        if !applied_policy_ok(&response, SECTION_FETCH) {
            return refuse("source_transport_failed");
        }
        keyed_text(response)
    }
    fn keyed_wrong_notes(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
    ) -> Result<String> {
        let response =
            self.port
                .wrong_notes(student, scope, MAX_SECTION_BODY_BYTES, SECTION_FETCH)?;
        self.requests += 1;
        if !applied_policy_ok(&response, SECTION_FETCH) {
            return refuse("source_transport_failed");
        }
        keyed_text(response)
    }
    fn keyed_detail(&mut self, student: &TextbookStudent, attempt_key: &str) -> Result<String> {
        let response =
            self.port
                .detail(student, attempt_key, MAX_SECTION_BODY_BYTES, SECTION_FETCH)?;
        self.requests += 1;
        if !applied_policy_ok(&response, SECTION_FETCH) {
            return refuse("source_transport_failed");
        }
        keyed_text(response)
    }
    fn paper_rows(&mut self, student: &TextbookStudent) -> Result<Vec<PaperSearchRow>> {
        let response = self
            .port
            .paper_search(student, MAX_UNIT_BODY_BYTES, SECTION_FETCH)?;
        self.requests += 1;
        if !applied_policy_ok(&response, SECTION_FETCH) {
            return refuse("source_transport_failed");
        }
        let html = paper_text(response)?;
        let rows = metadata_rows(
            &html,
            &Student {
                student_key: student.student_key.clone(),
                student_name: student.student_name.clone(),
            },
        )?;
        // A row whose title or keys are not strings is dropped, as TS drops a
        // row whose `titles.testingName` is not a string.
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(PaperSearchRow {
                    testing_name: optional_string(r.get("titles").get("testingName"))?,
                    paper_key: optional_string(r.get("paperKey"))?,
                    attempt_key: optional_string(r.get("attemptKey"))?,
                    apply_date_literal: optional_string(r.get("titles").get("applyDate")),
                })
            })
            .collect())
    }
    /// The paper-search hook: any failure is `Unavailable`, never an error.
    fn paper_search(&mut self, student: &TextbookStudent) -> PaperObservation {
        match self.paper_rows(student) {
            Ok(rows) => PaperObservation::Observed(rows),
            Err(_) => PaperObservation::Unavailable,
        }
    }

    /// `readTextbookBindings` plus the backend assertion: the student's
    /// registered bindings from one progress read, each label qualified.
    pub fn bindings(
        &mut self,
        teacher_label: &str,
        student: &TextbookStudent,
    ) -> Result<Vec<TextbookBinding>> {
        self.require_principal(teacher_label, "textbook_binding_principal_rejected")?;
        let response = self
            .port
            .progress(student, MAX_UNIT_BODY_BYTES, SECTION_FETCH)?;
        self.requests += 1;
        if !applied_policy_ok(&response, SECTION_FETCH) {
            return refuse("source_transport_failed");
        }
        let html = binding_text(response)?;
        let bindings = parse_textbook_bindings(&html, student.student_key())?;
        assert_bindings(&bindings, student.student_key())?;
        Ok(bindings)
    }

    /// `inspectTextbookIndex`: the merged wrong-note and paper-search index
    /// of one exact (course, book, student). A failed paper search degrades to
    /// the wrong-note-only basis; a title conflict for an attempt fails.
    pub fn index(
        &mut self,
        teacher_label: &str,
        student: &TextbookStudent,
        scope: &BookScope,
    ) -> Result<TextbookIndex> {
        self.require_principal(teacher_label, "textbook_index_scope_required")?;
        let progress = self.keyed_progress(student)?;
        require_scope_binding(&progress, scope, student)?;
        let notes_text = self.keyed_wrong_notes(student, scope)?;
        let notes = wrong_note_list(&notes_text, true)?;
        let wrong = wrong_rows(&notes, student, true)?;
        let titles: Vec<String> = wrong.iter().map(|r| r.source_title.clone()).collect();
        let pattern = derive_title_pattern(&titles, student.student_name());
        let (rows, paper_search) = match &pattern {
            None => (
                plain_rows(&wrong),
                PaperSearchStatus::NoDerivableSectionTitlePattern,
            ),
            Some(pattern) => match self.paper_rows(student) {
                Ok(paper) => {
                    let merged = merge_section_rows(&wrong, &paper, Some(pattern))?;
                    let matched = merged
                        .iter()
                        .filter(|r| r.sources.contains(&IndexSource::StudentPaperSearch))
                        .count();
                    (
                        merged,
                        PaperSearchStatus::Observed {
                            matched_row_count: matched,
                        },
                    )
                }
                Err(_) => (plain_rows(&wrong), PaperSearchStatus::Unavailable),
            },
        };
        let index_basis = if matches!(paper_search, PaperSearchStatus::Observed { .. }) {
            IndexBasis::WrongAnswerNotePlusStudentPaperSearch
        } else {
            IndexBasis::WrongAnswerNoteOnly
        };
        Ok(TextbookIndex {
            scope: scope.clone(),
            rows,
            index_basis,
            coverage: index_basis.coverage(),
            paper_search,
        })
    }

    /// `executeAcademyRead`, textbook branch: grade the requested titles for
    /// the exact (course, book, student). Each title must select exactly one
    /// merged attempt; the detail's sub-title must equal the title.
    pub fn grade(
        &mut self,
        teacher_label: &str,
        student: &TextbookStudent,
        scope: &BookScope,
        titles: &[String],
    ) -> Result<GradeOutcome> {
        validate_titles(titles)?;
        self.require_principal(teacher_label, "teacher_scope_conflict")?;
        let progress = self.keyed_progress(student)?;
        require_scope_binding(&progress, scope, student)?;
        let notes_text = self.keyed_wrong_notes(student, scope)?;
        let notes = wrong_note_list(&notes_text, false)?;
        for row in &notes {
            check_subject(row, student)?;
        }
        // The paper search is always asked for here (the TS client provides
        // the hook); it never raises, an unavailable page degrades.
        let paper = self.paper_search(student);
        let wrong = wrong_rows(&notes, student, false)?;
        let (attempts, pattern, observed) = match &paper {
            PaperObservation::Observed(rows) => {
                let wrong_titles: Vec<String> =
                    wrong.iter().map(|r| r.source_title.clone()).collect();
                let pattern = derive_title_pattern(&wrong_titles, student.student_name());
                (
                    merge_section_rows(&wrong, rows, pattern.as_ref())?,
                    pattern,
                    true,
                )
            }
            PaperObservation::Unavailable => (plain_rows(&wrong), None, false),
        };
        let mut records = vec![];
        for title in titles {
            let matches: Vec<&IndexRow> = attempts
                .iter()
                .filter(|r| &r.source_title == title)
                .collect();
            if matches.len() != 1 {
                // Absence is claimable only when the paper search was observed
                // and a pattern anchored paper rows to this book.
                if matches.is_empty() && observed && pattern.is_some() {
                    return Ok(GradeOutcome::NotObservedInEither(NotObservedInEither {
                        source_title: title.clone(),
                    }));
                }
                return refuse("source_selection_missing_or_ambiguous");
            }
            let row = matches[0];
            let attempt_key = literal_key(&row.attempt_key)?;
            let detail_text = self.keyed_detail(student, &attempt_key)?;
            let (score, items) = parse_detail(&detail_text, student, title)?;
            records.push(SectionRecord {
                source_title: title.clone(),
                student_key: student.student_key.clone(),
                student_name: student.student_name.clone(),
                course_key: scope.course_key.clone(),
                book_no: scope.book_no.clone(),
                paper_key: row.paper_key.clone(),
                attempt_key: row.attempt_key.clone(),
                score,
                items,
                source_date: source_date(&row.source_date_literal),
            });
        }
        // JavaScript default order: by UTF-16 code unit.
        records.sort_by(|a, b| {
            a.source_title
                .encode_utf16()
                .cmp(b.source_title.encode_utf16())
        });
        Ok(GradeOutcome::Graded(GradedSections {
            records,
            coverage: GRADING_COVERAGE,
            paper_search_observed: observed,
            unknown_state: if observed {
                UNKNOWN_STATE_PAPER_SEARCH_OBSERVED
            } else {
                UNKNOWN_STATE_PAPER_SEARCH_UNAVAILABLE
            },
        }))
    }

    /// `readTextbookSectionsByName` (native-dailyrecord-backend.ts:268): the
    /// owner discovers the binding; several bindings and no label return the
    /// interview (no grading read); an exact label selects exactly one.
    pub fn sections_by_name(
        &mut self,
        teacher_label: &str,
        student: &TextbookStudent,
        book_label: Option<&str>,
        titles: Option<&[String]>,
    ) -> Result<SectionsByName> {
        let bindings = self.bindings(teacher_label, student)?;
        let label = book_label.filter(|l| !l.is_empty());
        let selected: Vec<&TextbookBinding> = match label {
            Some(l) => bindings.iter().filter(|b| b.book_label == l).collect(),
            None => bindings.iter().collect(),
        };
        if label.is_some() && selected.len() != 1 {
            return refuse("textbook_binding_missing_or_ambiguous");
        }
        if label.is_none() && bindings.len() > 1 {
            let mut candidates = vec![];
            for binding in &bindings {
                let hint = BookScope::new(&binding.course_key, &binding.book_no)
                    .and_then(|scope| self.index(teacher_label, student, &scope))
                    .ok()
                    .and_then(|index| {
                        let valid = index.rows.len() <= MAX_HINT_ROWS
                            && index.rows.iter().all(|r| {
                                (1..=MAX_HINT_TITLE_UNITS).contains(&utf16_len(&r.source_title))
                            });
                        valid.then(|| index.rows.first().map(|r| r.source_title.clone()))?
                    });
                candidates.push(InterviewCandidate {
                    book_label: binding.book_label.clone(),
                    book_no: binding.book_no.clone(),
                    hint,
                });
            }
            return Ok(SectionsByName::Interview(Interview {
                bindings: candidates,
            }));
        }
        let binding = selected[0].clone();
        if binding.student_key != student.student_key {
            return refuse("textbook_binding_scope_conflict");
        }
        let scope = BookScope::new(&binding.course_key, &binding.book_no)?;
        match titles {
            Some(titles) => {
                let outcome = self.grade(teacher_label, student, &scope, titles)?;
                Ok(SectionsByName::Graded(GradedByName {
                    scope,
                    book_label: binding.book_label,
                    outcome,
                }))
            }
            None => {
                let index = self.index(teacher_label, student, &scope)?;
                Ok(SectionsByName::Index(IndexByName {
                    scope,
                    book_label: binding.book_label,
                    index,
                }))
            }
        }
    }
}

// ------------------------------------------------------- session port

/// Section reads over an admitted `ReadPort` session. The course scope drives
/// the progress, wrong-note and detail reads (namespace
/// `academy.lms.student_course`); the metadata scope drives the paper search
/// (namespace `academy.lms.metadata`). Both scopes are bound to the one
/// admitted student: a read for another student refuses.
pub struct SessionTextbookSectionPort<'a, P: ReadPort> {
    port: &'a mut P,
    course_scope: TransportScope,
    metadata_scope: TransportScope,
    paper_form: PaperFormPin,
}
impl<'a, P: ReadPort> SessionTextbookSectionPort<'a, P> {
    pub fn new(
        port: &'a mut P,
        course_scope: TransportScope,
        metadata_scope: TransportScope,
        paper_form: PaperFormPin,
    ) -> Self {
        Self {
            port,
            course_scope,
            metadata_scope,
            paper_form,
        }
    }
    fn gate(&self, student: &TextbookStudent, policy: FetchPolicy) -> Result<()> {
        if policy != SECTION_FETCH {
            return refuse("source_transport_failed");
        }
        if student.student_key != self.course_scope.student_id
            || student.student_key != self.metadata_scope.student_id
        {
            return refuse("textbook_read_scope_conflict");
        }
        Ok(())
    }
    fn course_read(
        &mut self,
        operation: &str,
        arguments: &[(String, String)],
        max_bytes: usize,
        over_code: &str,
    ) -> Result<TextbookResponse> {
        check_contract(self.port.transport_admission(), operation)?;
        let response = self.port.read(&self.course_scope, operation, arguments)?;
        Self::wrap(response, max_bytes, over_code)
    }
    fn wrap(
        response: crate::http::Response,
        max_bytes: usize,
        over_code: &str,
    ) -> Result<TextbookResponse> {
        if response.stalled {
            return refuse("source_transport_failed");
        }
        if response.bytes.len() > max_bytes {
            return refuse(over_code);
        }
        Ok(TextbookResponse {
            status: response.status,
            headers: response.headers,
            bytes: response.bytes,
            final_url: (!response.url.is_empty()).then_some(response.url),
            redirected: false,
            credentials_sent: true,
            redirect_followed: false,
        })
    }
}
impl<P: ReadPort> TextbookSectionPort for SessionTextbookSectionPort<'_, P> {
    fn progress(
        &mut self,
        student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.gate(student, policy)?;
        // The binding read is unit-bounded in TS (`textbook_binding_source_rejected`);
        // the keyed progress read is byte-bounded (`source_response_too_large`).
        let over = if max_bytes == MAX_UNIT_BODY_BYTES {
            "textbook_binding_source_rejected"
        } else {
            "source_response_too_large"
        };
        self.course_read(
            "student.books",
            &[("student_name".into(), student.student_name.clone())],
            max_bytes,
            over,
        )
    }
    fn wrong_notes(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.gate(student, policy)?;
        // JSON.stringify({courseKeyList:[..],bookNoList:[..],userKeyList:[..]})
        let argument = format!(
            r#"{{"courseKeyList":["{}"],"bookNoList":["{}"],"userKeyList":["{}"]}}"#,
            scope.course_key, scope.book_no, student.student_key
        );
        self.course_read(
            "textbook.wrongnote",
            &[("answerNoteResult".into(), argument)],
            max_bytes,
            "source_response_too_large",
        )
    }
    fn detail(
        &mut self,
        student: &TextbookStudent,
        attempt_key: &str,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.gate(student, policy)?;
        let attempt = literal_key(attempt_key)?;
        // Number(readLiteralKey(attemptKey)) and Number(studentKey), in the
        // TS key order.
        let condition = format!(
            r#"{{"testing_no":{attempt},"pri_no":{},"student_name":{},"score":{{"create":1}},"incorrect":{{"create":0}},"similar":{{"create":0}},"advance":{{"create":0}},"report":{{"create":0}}}}"#,
            student.student_key,
            serde_json::to_string(&student.student_name)
                .map_err(|_| Error::refusal("invalid_read_name"))?
        );
        self.course_read(
            "textbook.detail",
            &[("condition".into(), condition)],
            max_bytes,
            "source_response_too_large",
        )
    }
    fn paper_search(
        &mut self,
        student: &TextbookStudent,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        self.gate(student, policy)?;
        check_contract(self.port.transport_admission(), "metadata.paper.initial")?;
        check_contract(self.port.transport_admission(), "metadata.paper.search.7")?;
        let college_id = self.port.transport_admission().college_id.clone();
        let teacher_id = self.metadata_scope.teacher_id.clone();
        let initial = self
            .port
            .read(&self.metadata_scope, "metadata.paper.initial", &[])?;
        let wrapped = Self::wrap(initial, max_bytes, "student_paper_source_rejected")?;
        let html = decode_bounded(&wrapped.bytes, MAX_UNIT_BODY_BYTES, MAX_BODY_UNITS)
            .map_err(|_| Error::refusal("student_paper_source_rejected"))?;
        let d = Document::parse(&html);
        self.paper_form.verify(&d)?;
        if field(&d, "form[name=userForm] [name=pri_no]")? != teacher_id {
            return refuse("student_paper_owner_drift");
        }
        let forms = d.select("form[name=search]");
        if forms.len() != 1
            || !forms[0]
                .value()
                .attr("method")
                .is_some_and(|m| m.eq_ignore_ascii_case("post"))
        {
            return refuse("student_paper_search_method_drift");
        }
        // All three paper types are selected explicitly: the form checks only
        // one by default, which would drop attempts of the other types.
        let mut arguments: Vec<(String, String)> = vec![];
        let mut class = String::new();
        for k in [
            "clg_no",
            "cls_no",
            "check_da_test",
            "check_fa_test",
            "check_na_test",
            "chioce_cls",
            "sort_date1",
            "sort_date2",
        ] {
            let nodes = dom::select(forms[0], &format!("[name={k}]"));
            if nodes.len() != 1 {
                return refuse("student_paper_filter_drift");
            }
            let node = nodes[0];
            let value = dom::value(node);
            let limit = if k.starts_with("sort_date") { 10 } else { 64 };
            if value.len() > limit
                || (k.starts_with("check_") && node.value().attr("type") != Some("checkbox"))
            {
                return refuse("student_paper_filter_drift");
            }
            if k == "chioce_cls" {
                class = value.clone();
            }
            arguments.push((k.to_string(), value));
        }
        if !arguments
            .iter()
            .any(|(k, v)| k == "clg_no" && v == &college_id)
            || utf16_len(&student.student_name) < 2
        {
            return refuse("student_paper_college_scope_conflict");
        }
        if let Some(entry) = arguments.iter_mut().find(|(k, _)| k == "cls_no") {
            entry.1 = class;
        }
        arguments.push(("stu_name".into(), student.student_name.clone()));
        let result = self
            .port
            .read(&self.metadata_scope, "metadata.paper.search.7", &arguments)?;
        let wrapped = Self::wrap(result, max_bytes, "student_paper_response_too_large")?;
        let html = decode_bounded(&wrapped.bytes, MAX_UNIT_BODY_BYTES, MAX_BODY_UNITS)
            .map_err(|_| Error::refusal("student_paper_response_too_large"))?;
        let d = Document::parse(&html);
        if field(&d, "form[name=userForm] [name=pri_no]")? != teacher_id {
            return refuse("student_paper_owner_drift");
        }
        Ok(wrapped)
    }
}

#[cfg(test)]
mod tests {
    //! The session port against the real admitted contracts: every read is
    //! rendered through `TransportAdmission::request`, so an argument name or
    //! bound that the contract does not admit fails here.
    use super::*;
    use crate::{
        http::Response,
        owner_admission::TransportAdmission,
        student_course::{COURSE_NS, METADATA_NS, PAPER_PATH, read_contracts},
    };

    type Rendered = (String, Vec<(String, String)>, Vec<(String, String)>);
    struct Rendering {
        admission: TransportAdmission,
        log: Vec<Rendered>,
        replies: HashMap<String, Vec<u8>>,
        stalled: bool,
    }
    impl ReadPort for Rendering {
        fn transport_admission(&self) -> &TransportAdmission {
            &self.admission
        }
        fn read(
            &mut self,
            scope: &TransportScope,
            id: &str,
            args: &[(String, String)],
        ) -> Result<Response> {
            let request = self.admission.request(scope, id, args, false)?;
            self.log.push((id.into(), request.query, request.body));
            Ok(Response {
                status: 200,
                headers: vec![],
                bytes: self.replies.get(id).cloned().unwrap_or_default(),
                url: "https://dc.gang-a.kr/x".into(),
                stalled: self.stalled,
            })
        }
    }
    fn admission(operations: Vec<crate::owner_admission::OperationContract>) -> TransportAdmission {
        TransportAdmission {
            schema_version: "academy.transport-admission/1".into(),
            owner_id: "fictional-owner".into(),
            teacher_id: "99001".into(),
            teacher_label: "Fictional Teacher".into(),
            college_id: "88001".into(),
            allowed_from: "2040-01-01".into(),
            allowed_through: "2040-01-31".into(),
            student_ids: vec!["77001".into()],
            source_namespaces: vec![COURSE_NS.into(), METADATA_NS.into()],
            operations,
        }
    }
    fn scope(namespace: &str) -> TransportScope {
        TransportScope {
            owner_id: "fictional-owner".into(),
            source_namespace: namespace.into(),
            teacher_id: "99001".into(),
            student_id: "77001".into(),
            date: "2040-01-04".into(),
        }
    }
    const SEARCH_FORM: &str = r#"<form name="search" action="/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&amp;ass_no=1001" method="post"><input name="clg_no" type="hidden" value="88001"><input name="cls_no" type="hidden" value="0"><input name="p_pageno" type="hidden" value="4"><input name="check_da_test" type="checkbox" value="D" checked><input name="check_fa_test" type="checkbox" value="F"><input name="check_na_test" type="checkbox" value="N"><input name="stu_name" type="text"><select name="chioce_cls"><option value="0">All fictional classes</option></select><input name="sort_date1" type="text" value="2000-01-01"><input name="sort_date2" type="text" value="2000-01-02"></form>"#;
    const OWNER: &str = "<form name=userForm><input name=pri_no value=99001></form>";
    fn pin() -> PaperFormPin {
        let fields: Vec<serde_json::Value> = [
            ("clg_no", "input", Some("hidden"), Some("88001")),
            ("cls_no", "input", Some("hidden"), None),
            ("p_pageno", "input", Some("hidden"), None),
            ("check_da_test", "input", Some("checkbox"), Some("D")),
            ("check_fa_test", "input", Some("checkbox"), Some("F")),
            ("check_na_test", "input", Some("checkbox"), Some("N")),
            ("stu_name", "input", Some("text"), None),
            ("chioce_cls", "select", None, Some("0")),
            ("sort_date1", "input", Some("text"), None),
            ("sort_date2", "input", Some("text"), None),
        ]
        .iter()
        .map(|(name, tag, ty, value)| {
            serde_json::json!({
                "name": name, "tag": tag, "type": ty, "value": value,
                "checked": if *ty == Some("checkbox") { serde_json::json!(*name == "check_da_test") } else { serde_json::Value::Null },
                "options": if *tag == "select" { serde_json::json!([{"value": "0", "label": "All fictional classes"}]) } else { serde_json::json!([]) },
            })
        })
        .collect();
        let value = serde_json::json!({
            "schemaVersion": "academy.paper-search-form/1",
            "action": format!("{PAPER_PATH}?p_process=UserBySearchTestResult&ass_no=1001"),
            "method": "post",
            "fields": fields,
        });
        PaperFormPin::parse(&parse(&value.to_string()).unwrap()).unwrap()
    }
    fn rendering() -> Rendering {
        let mut replies = HashMap::new();
        replies.insert("student.books".to_string(), b"progress".to_vec());
        replies.insert("textbook.wrongnote".to_string(), b"notes".to_vec());
        replies.insert("textbook.detail".to_string(), b"detail".to_vec());
        replies.insert(
            "metadata.paper.initial".to_string(),
            format!("{OWNER}{SEARCH_FORM}").into_bytes(),
        );
        replies.insert(
            "metadata.paper.search.7".to_string(),
            format!("{OWNER}<p>result</p>").into_bytes(),
        );
        Rendering {
            admission: admission(read_contracts()),
            log: vec![],
            replies,
            stalled: false,
        }
    }
    fn student() -> TextbookStudent {
        TextbookStudent::new("77001", "Invented Learner").unwrap()
    }
    fn code(error: Error) -> String {
        error.message
    }
    fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn session_port_renders_every_read_through_the_admitted_contracts() {
        let mut read = rendering();
        let mut port =
            SessionTextbookSectionPort::new(&mut read, scope(COURSE_NS), scope(METADATA_NS), pin());
        let s = student();
        let book = BookScope::new("693", "2517").unwrap();
        for max in [MAX_SECTION_BODY_BYTES, MAX_UNIT_BODY_BYTES] {
            let r = port.progress(&s, max, SECTION_FETCH).unwrap();
            assert_eq!(r.bytes, b"progress");
            assert!(r.credentials_sent && !r.redirect_followed && !r.redirected);
        }
        port.wrong_notes(&s, &book, MAX_SECTION_BODY_BYTES, SECTION_FETCH)
            .unwrap();
        port.detail(&s, "82004", MAX_SECTION_BODY_BYTES, SECTION_FETCH)
            .unwrap();
        let paper = port
            .paper_search(&s, MAX_UNIT_BODY_BYTES, SECTION_FETCH)
            .unwrap();
        assert!(String::from_utf8(paper.bytes).unwrap().contains("result"));
        let log = &read.log;
        assert_eq!(log[0].0, "student.books");
        assert_eq!(log[0].1, pairs(&[("reqCmd", "getTextBookProgress")]));
        assert_eq!(log[0].2, pairs(&[("student_name", "Invented Learner")]));
        assert_eq!(log[2].0, "textbook.wrongnote");
        assert!(log[2].1.is_empty());
        assert_eq!(
            log[2].2,
            pairs(&[
                ("p_process", "getWrongAnswerNoteResult"),
                (
                    "answerNoteResult",
                    r#"{"courseKeyList":["693"],"bookNoList":["2517"],"userKeyList":["77001"]}"#
                ),
            ])
        );
        assert_eq!(log[3].0, "textbook.detail");
        assert_eq!(
            log[3].2,
            pairs(&[
                ("p_process", "getStudyResultSingleTestingSingleUser"),
                (
                    "condition",
                    r#"{"testing_no":82004,"pri_no":77001,"student_name":"Invented Learner","score":{"create":1},"incorrect":{"create":0},"similar":{"create":0},"advance":{"create":0},"report":{"create":0}}"#
                ),
            ])
        );
        // The paper search reads the form, then posts with all three types.
        assert_eq!(log[4].0, "metadata.paper.initial");
        assert_eq!(log[5].0, "metadata.paper.search.7");
        assert_eq!(
            log[5].1,
            pairs(&[("p_process", "UserBySearchTestResult"), ("ass_no", "1001")])
        );
        for expected in [
            ("clg_no", "88001"),
            ("cls_no", "0"),
            ("p_pageno", "1"),
            ("check_da_test", "D"),
            ("check_fa_test", "F"),
            ("check_na_test", "N"),
            ("stu_name", "Invented Learner"),
            ("chioce_cls", "0"),
            ("sort_date1", "2000-01-01"),
            ("sort_date2", "2000-01-02"),
        ] {
            assert!(
                log[5]
                    .2
                    .contains(&(expected.0.to_string(), expected.1.to_string())),
                "{expected:?}"
            );
        }
        assert_eq!(log[5].2.len(), 10);
    }

    #[test]
    fn session_port_binds_the_student_policy_and_contracts() {
        let s = student();
        let other = TextbookStudent::new("77002", "Other Learner").unwrap();
        let book = BookScope::new("693", "2517").unwrap();
        let mut read = rendering();
        let mut port =
            SessionTextbookSectionPort::new(&mut read, scope(COURSE_NS), scope(METADATA_NS), pin());
        assert_eq!(
            code(
                port.progress(&other, MAX_SECTION_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "textbook_read_scope_conflict"
        );
        assert_eq!(
            code(
                port.wrong_notes(&other, &book, MAX_SECTION_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "textbook_read_scope_conflict"
        );
        for policy in [
            FetchPolicy {
                send_credentials: false,
                follow_redirects: false,
            },
            FetchPolicy {
                send_credentials: true,
                follow_redirects: true,
            },
        ] {
            assert_eq!(
                code(
                    port.detail(&s, "1", MAX_SECTION_BODY_BYTES, policy)
                        .unwrap_err()
                ),
                "source_transport_failed"
            );
        }
        assert_eq!(
            code(
                port.detail(&s, "0", MAX_SECTION_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "invalid_read_key"
        );
        assert!(
            read.log.is_empty(),
            "no read happened for a refused request"
        );
        // An unadmitted contract is refused before any read.
        let mut none = Rendering {
            admission: admission(vec![]),
            ..rendering()
        };
        let mut port =
            SessionTextbookSectionPort::new(&mut none, scope(COURSE_NS), scope(METADATA_NS), pin());
        assert_eq!(
            code(
                port.progress(&s, MAX_SECTION_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "student_read_contract_not_admitted"
        );
        assert_eq!(
            code(
                port.paper_search(&s, MAX_UNIT_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "student_read_contract_not_admitted"
        );
        // A stalled or over-bound response refuses with the read's own code.
        let mut stalled = Rendering {
            stalled: true,
            ..rendering()
        };
        let mut port = SessionTextbookSectionPort::new(
            &mut stalled,
            scope(COURSE_NS),
            scope(METADATA_NS),
            pin(),
        );
        assert_eq!(
            code(
                port.progress(&s, MAX_SECTION_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "source_transport_failed"
        );
        for (max, size, expected) in [
            (
                MAX_SECTION_BODY_BYTES,
                MAX_SECTION_BODY_BYTES + 1,
                "source_response_too_large",
            ),
            (
                MAX_UNIT_BODY_BYTES,
                MAX_UNIT_BODY_BYTES + 1,
                "textbook_binding_source_rejected",
            ),
        ] {
            let mut big = rendering();
            big.replies.insert("student.books".into(), vec![b'a'; size]);
            let mut port = SessionTextbookSectionPort::new(
                &mut big,
                scope(COURSE_NS),
                scope(METADATA_NS),
                pin(),
            );
            assert_eq!(
                code(port.progress(&s, max, SECTION_FETCH).unwrap_err()),
                expected
            );
        }
    }

    #[test]
    fn session_port_paper_search_checks_the_form_and_the_owner() {
        let s = student();
        // Form drift: a changed pin field refuses; the search is never posted.
        let mut drift = rendering();
        drift.replies.insert(
            "metadata.paper.initial".into(),
            format!(
                "{OWNER}{}",
                SEARCH_FORM.replace("value=\"88001\"", "value=\"88002\"")
            )
            .into_bytes(),
        );
        let mut port = SessionTextbookSectionPort::new(
            &mut drift,
            scope(COURSE_NS),
            scope(METADATA_NS),
            pin(),
        );
        assert_eq!(
            code(
                port.paper_search(&s, MAX_UNIT_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "contract_drift"
        );
        assert_eq!(drift.log.len(), 1);
        // Owner drift on the form page and on the result page.
        for (id, page, expected) in [
            (
                "metadata.paper.initial",
                SEARCH_FORM.to_string(),
                "contract_drift",
            ),
            (
                "metadata.paper.initial",
                format!("{}{SEARCH_FORM}", OWNER.replace("99001", "99002")),
                "student_paper_owner_drift",
            ),
            (
                "metadata.paper.search.7",
                OWNER.replace("99001", "99002"),
                "student_paper_owner_drift",
            ),
        ] {
            let mut read = rendering();
            read.replies.insert(id.into(), page.into_bytes());
            let mut port = SessionTextbookSectionPort::new(
                &mut read,
                scope(COURSE_NS),
                scope(METADATA_NS),
                pin(),
            );
            let error = port
                .paper_search(&s, MAX_UNIT_BODY_BYTES, SECTION_FETCH)
                .unwrap_err();
            assert!(
                code(error.clone()) == expected
                    || id == "metadata.paper.initial" && expected == "contract_drift",
                "{id} {error:?}"
            );
        }
        // A student name under two units cannot be searched.
        let short = TextbookStudent::new("77001", "A").unwrap();
        let mut read = rendering();
        let mut port =
            SessionTextbookSectionPort::new(&mut read, scope(COURSE_NS), scope(METADATA_NS), pin());
        assert_eq!(
            code(
                port.paper_search(&short, MAX_UNIT_BODY_BYTES, SECTION_FETCH)
                    .unwrap_err()
            ),
            "student_paper_college_scope_conflict"
        );
    }
}
