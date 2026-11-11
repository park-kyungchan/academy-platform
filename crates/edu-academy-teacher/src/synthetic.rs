//! Synthetic backend: the three existing source ports served from one
//! parsed scenario file, so the real parsers of `edu-academy-sources` run.
use edu_academy_core::js::Js;
use edu_academy_sources::{
    data::{Error, Result, obj},
    http::Response,
    native::SourcePort,
    prestudy::{ListKind, PrestudyPort, PrestudySearchForm},
    textbook::{FetchPolicy, TextbookResponse},
    textbook_index::{BookScope, TextbookSectionPort, TextbookStudent},
};
use serde::Deserialize;

pub const TEACHER_LABEL: &str = "가상교사";
pub const TEACHER_KEY: &str = "99001";
pub const TEACHER_ID: &str = "9001";
pub const NOW: &str = "2026-11-11T09:00:00+09:00";

#[derive(Clone, Debug, Deserialize)]
pub struct Scenario {
    pub today: String,
    pub students: Vec<ScenarioStudent>,
    pub classes: Vec<ScenarioClass>,
    pub records: Vec<ScenarioRecord>,
    pub prestudy: PrestudyScenario,
    pub sections: SectionsScenario,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioStudent {
    pub name: String,
    pub key: String,
    pub class_name: String,
    pub bindings: Vec<ScenarioBinding>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioBinding {
    pub label: String,
    pub course_key: String,
    pub book_no: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioClass {
    pub name: String,
    pub weekdays: Vec<String>,
    pub roster: Vec<String>,
    #[serde(default)]
    pub moved: Vec<MovedSpec>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MovedSpec {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioRecord {
    pub student: String,
    pub date: String,
    pub progress: String,
    pub homework: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PrestudyScenario {
    #[serde(default)]
    pub waiting: Vec<PrestudyRow>,
    #[serde(default)]
    pub completed: Vec<PrestudyRow>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PrestudyRow {
    pub student: String,
    pub unit_label: String,
    pub date: String,
    pub key: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SectionsScenario {
    #[serde(default)]
    pub index_titles: Vec<IndexTitle>,
    #[serde(default)]
    pub grades: Vec<GradeSpec>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IndexTitle {
    pub student: String,
    pub course_key: String,
    pub book_no: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GradeSpec {
    pub title: String,
    pub correct: usize,
    pub total: usize,
    pub wrong_labels: Vec<String>,
}

/// Load and validate a scenario. Checks: at most one record per student
/// and date, and every student's earliest record can be given a synthetic
/// predecessor (its date is after 0001-01-01), which the day port serves.
pub fn load_scenario(text: &str) -> std::result::Result<Scenario, String> {
    let scenario: Scenario =
        serde_json::from_str(text).map_err(|e| format!("scenario_parse_failed: {e}"))?;
    validate_scenario(&scenario)?;
    Ok(scenario)
}

pub fn validate_scenario(scenario: &Scenario) -> std::result::Result<(), String> {
    chrono::NaiveDate::parse_from_str(&scenario.today, "%Y-%m-%d")
        .map_err(|_| "scenario_today_invalid".to_string())?;
    let mut seen = std::collections::HashSet::new();
    for record in &scenario.records {
        if !seen.insert((record.student.clone(), record.date.clone())) {
            return Err(format!(
                "scenario_duplicate_record: {} {}",
                record.student, record.date
            ));
        }
        chrono::NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
            .map_err(|_| "scenario_record_date_invalid".to_string())?;
        if !scenario.students.iter().any(|s| s.name == record.student) {
            return Err(format!(
                "scenario_record_unknown_student: {}",
                record.student
            ));
        }
    }
    // Earliest-record predecessor check: a predecessor date must exist.
    for student in &scenario.students {
        let earliest = scenario
            .records
            .iter()
            .filter(|r| r.student == student.name)
            .map(|r| r.date.as_str())
            .min();
        if let Some(earliest) = earliest {
            let date = chrono::NaiveDate::parse_from_str(earliest, "%Y-%m-%d")
                .map_err(|_| "scenario_record_date_invalid".to_string())?;
            if date.pred_opt().is_none() {
                return Err(format!(
                    "scenario_earliest_record_without_predecessor: {}",
                    student.name
                ));
            }
        }
    }
    Ok(())
}

impl Scenario {
    pub fn student_by_name(&self, name: &str) -> Option<&ScenarioStudent> {
        self.students.iter().find(|s| s.name == name)
    }
    pub fn student_by_key(&self, key: &str) -> Option<&ScenarioStudent> {
        self.students.iter().find(|s| s.key == key)
    }
    pub fn record(&self, student: &str, date: &str) -> Option<&ScenarioRecord> {
        self.records
            .iter()
            .find(|r| r.student == student && r.date == date)
    }
    fn record_key(&self, student: &str, date: &str) -> String {
        let mut sorted: Vec<&ScenarioRecord> = self.records.iter().collect();
        sorted.sort_by(|a, b| (&a.student, &a.date).cmp(&(&b.student, &b.date)));
        sorted
            .iter()
            .position(|r| r.student == student && r.date == date)
            .map_or_else(|| "7999".to_string(), |i| format!("{}", 8100 + i))
    }
    fn row(&self, record: &ScenarioRecord) -> Js {
        let student = self.student_by_name(&record.student);
        let (key, course) = student.map_or(("0", "101"), |s| {
            (
                s.key.as_str(),
                s.bindings.first().map_or("101", |b| b.course_key.as_str()),
            )
        });
        obj(&[
            ("studentName", record.student.clone().into()),
            ("date", record.date.clone().into()),
            ("groupId", "1".into()),
            ("teacherKey", TEACHER_ID.into()),
            ("studentKey", key.into()),
            ("courseKey", course.into()),
            (
                "recordKey",
                self.record_key(&record.student, &record.date).into(),
            ),
            ("membershipKey", "601".into()),
        ])
    }
}

// ---------------------------------------------------------------- day port

pub struct SyntheticDayPort {
    scenario: Scenario,
    expecting_previous: Option<Js>,
}

impl SyntheticDayPort {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            expecting_previous: None,
        }
    }
}

fn js_string(v: &Js) -> String {
    String::from_utf16_lossy(v.units())
}

impl SourcePort for SyntheticDayPort {
    fn authenticate(&mut self) -> Result<Js> {
        Ok(obj(&[("status", "authenticated".into())]))
    }
    fn discover(&mut self, _input: &Js) -> Result<Js> {
        Ok(obj(&[(
            "groups",
            Js::Arr(vec![obj(&[("groupId", "1".into())])]),
        )]))
    }
    fn execute(&mut self, input: &Js) -> Result<Js> {
        if let Some(previous) = self.expecting_previous.take() {
            return Ok(obj(&[("dayRecords", Js::Arr(vec![previous]))]));
        }
        let date = js_string(input.get("date"));
        let names = input.get("studentNames").array();
        let name = names.first().map_or(String::new(), js_string);
        let rows: Vec<Js> = self
            .scenario
            .records
            .iter()
            .filter(|r| r.student == name && r.date == date)
            .map(|r| self.scenario.row(r))
            .collect();
        Ok(obj(&[("dayRecords", Js::Arr(rows))]))
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        let student_key = js_string(target.get("studentKey"));
        let date = js_string(target.get("date"));
        let student = self
            .scenario
            .student_by_key(&student_key)
            .ok_or_else(|| Error::error("synthetic_student_missing"))?;
        let record = self
            .scenario
            .record(&student.name, &date)
            .ok_or_else(|| Error::error("synthetic_record_missing"))?
            .clone();
        let previous = self
            .scenario
            .records
            .iter()
            .filter(|r| r.student == student.name && r.date < date)
            .max_by(|a, b| a.date.cmp(&b.date))
            .cloned();
        let (previous_date, previous_key, previous_row) = match previous {
            Some(prev) => {
                let key = self.scenario.record_key(&prev.student, &prev.date);
                let row = self.scenario.row(&prev);
                (prev.date.clone(), key, row)
            }
            None => {
                let current = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .map_err(|_| Error::error("synthetic_date_invalid"))?;
                let prev_date = current
                    .pred_opt()
                    .ok_or_else(|| Error::error("synthetic_predecessor_missing"))?;
                let prev_record = ScenarioRecord {
                    student: student.name.clone(),
                    date: prev_date.format("%Y-%m-%d").to_string(),
                    progress: String::new(),
                    homework: String::new(),
                };
                let mut row = self.scenario.row(&prev_record);
                row.set("recordKey", "7999".into());
                (prev_record.date.clone(), "7999".to_string(), row)
            }
        };
        self.expecting_previous = Some(previous_row);
        Ok(obj(&[
            ("target", target.clone()),
            ("previousDate", previous_date.into()),
            ("previousRecordKey", previous_key.into()),
            (
                "fields",
                obj(&[
                    ("progress", record.progress.clone().into()),
                    ("homework", record.homework.clone().into()),
                ]),
            ),
            (
                "wireDigest",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            ),
        ]))
    }
}

// ----------------------------------------------------------- prestudy port

pub struct SyntheticPrestudyPort {
    scenario: Scenario,
}

impl SyntheticPrestudyPort {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
    fn form_html(&self, kind: ListKind) -> String {
        format!(
            "<html><form name=\"form1\"><input type=\"hidden\" name=\"reqCmd\" value=\"{}\"><input type=\"hidden\" name=\"g_clg_no\" value=\"88001\"><input type=\"hidden\" name=\"g_mem_type\" value=\"1\"><input type=\"hidden\" name=\"g_pri_no\" value=\"{TEACHER_KEY}\"><input type=\"text\" name=\"student_name\" value=\"\"></form></html>",
            kind.cmd()
        )
    }
    fn list_html(&self, kind: ListKind) -> String {
        let rows = match kind {
            ListKind::Waiting => &self.scenario.prestudy.waiting,
            ListKind::Completed => &self.scenario.prestudy.completed,
        };
        let mut html = self.form_html(kind);
        html.push_str("<table><tr><th>학생이름</th><th>단원명</th><th>날짜</th><th>영상 상태</th><th></th></tr>");
        for row in rows {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>OK</td><td><button onclick=\"prestudy_main({});\">예습 검사</button></td></tr>",
                row.student, row.unit_label, row.date, row.key
            ));
        }
        html.push_str("</table>");
        html
    }
}

fn response(html: String) -> Response {
    Response {
        status: 200,
        headers: vec![],
        bytes: html.into_bytes(),
        url: String::new(),
        stalled: false,
    }
}

impl PrestudyPort for SyntheticPrestudyPort {
    fn shell(&mut self, kind: ListKind, _bound: usize) -> Result<Response> {
        Ok(response(self.form_html(kind)))
    }
    fn search(
        &mut self,
        kind: ListKind,
        _form: &PrestudySearchForm,
        _student_name: &str,
        _bound: usize,
    ) -> Result<Response> {
        Ok(response(self.list_html(kind)))
    }
    fn summary(&mut self, _bound: usize) -> Result<Response> {
        Ok(response(
            "<table><tr><th>학생</th><th>단원명</th><th>날짜</th><th>Ai총점</th><th>별점</th><th>action</th></tr></table>".into(),
        ))
    }
    fn detail(&mut self, _key: &str, _bound: usize) -> Result<Response> {
        Err(Error::refusal("prestudy_source_port_unavailable"))
    }
    fn report(&mut self, _key: &str, _bound: usize) -> Result<Response> {
        Err(Error::refusal("prestudy_source_port_unavailable"))
    }
}

// ----------------------------------------------------------- textbook port

pub struct SyntheticTextbookPort {
    scenario: Scenario,
}

impl SyntheticTextbookPort {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
    fn titles_for(&self, student_name: &str) -> Vec<&IndexTitle> {
        self.scenario
            .sections
            .index_titles
            .iter()
            .filter(|t| t.student == student_name)
            .collect()
    }
    fn attempt_key(&self, title: &str) -> String {
        self.scenario
            .sections
            .index_titles
            .iter()
            .position(|t| t.title == title)
            .map_or_else(|| "8299".to_string(), |i| format!("{}", 8200 + i))
    }
    fn paper_key(&self, title: &str) -> String {
        self.scenario
            .sections
            .index_titles
            .iter()
            .position(|t| t.title == title)
            .map_or_else(|| "8199".to_string(), |i| format!("{}", 8100 + i))
    }
}

fn textbook_response(body: String) -> TextbookResponse {
    TextbookResponse {
        status: 200,
        headers: vec![],
        bytes: body.into_bytes(),
        final_url: None,
        redirected: false,
        credentials_sent: true,
        redirect_followed: false,
    }
}

impl TextbookSectionPort for SyntheticTextbookPort {
    fn progress(
        &mut self,
        student: &TextbookStudent,
        _max: usize,
        _policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let scenario_student = self.scenario.student_by_key(student.student_key());
        let mut html = String::from("<table>");
        if let Some(found) = scenario_student {
            for binding in &found.bindings {
                html.push_str(&format!(
                    "<tr><td>가상</td><td>{}</td><td><button onclick=\"createBookIncorrectPapers({},{},{})\">생성</button></td></tr>",
                    binding.label, binding.course_key, binding.book_no, found.key
                ));
            }
        }
        html.push_str("</table>");
        Ok(textbook_response(html))
    }
    fn wrong_notes(
        &mut self,
        student: &TextbookStudent,
        scope: &BookScope,
        _max: usize,
        _policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let key: u64 = student.student_key().parse().unwrap_or(0);
        let list: Vec<serde_json::Value> = self
            .titles_for(student.student_name())
            .into_iter()
            .filter(|t| t.course_key == scope.course_key() && t.book_no == scope.book_no())
            .map(|t| {
                serde_json::json!({"pri_no": key, "name": student.student_name(),
                    "p_no": self.paper_key(&t.title).parse::<u64>().unwrap_or(0),
                    "testing_no": self.attempt_key(&t.title).parse::<u64>().unwrap_or(0),
                    "testing_name": t.title, "apply_date": "11-09"})
            })
            .collect();
        Ok(textbook_response(
            serde_json::json!({"result": "ok", "noteResultList": list}).to_string(),
        ))
    }
    fn detail(
        &mut self,
        student: &TextbookStudent,
        attempt_key: &str,
        _max: usize,
        _policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let title = self
            .scenario
            .sections
            .index_titles
            .iter()
            .find(|t| self.attempt_key(&t.title) == attempt_key)
            .map(|t| t.title.clone())
            .ok_or_else(|| Error::refusal("synthetic_detail_missing"))?;
        let grade = self
            .scenario
            .sections
            .grades
            .iter()
            .find(|g| g.title == title)
            .ok_or_else(|| Error::refusal("synthetic_grade_missing"))?;
        let key: u64 = student.student_key().parse().unwrap_or(0);
        let mut items = Vec::new();
        let mut exam_no = 1u64;
        for label in &grade.wrong_labels {
            items.push(serde_json::json!({"exam_no": exam_no, "lecture_key": label.parse::<u64>().unwrap_or(0), "scoring_result": "X"}));
            exam_no += 1;
        }
        let correct = grade.total.saturating_sub(grade.wrong_labels.len());
        for i in 0..correct {
            items.push(serde_json::json!({"exam_no": exam_no, "lecture_key": 9000 + i as u64, "scoring_result": "O"}));
            exam_no += 1;
        }
        let score = (grade.correct * 100).checked_div(grade.total).unwrap_or(0);
        Ok(textbook_response(
            serde_json::json!({"result": "OK", "print_list": [{"pri_no": key, "student_name": student.student_name(), "sub_title": title, "score": score, "exam_list": items}]})
            .to_string(),
        ))
    }
    fn paper_search(
        &mut self,
        student: &TextbookStudent,
        _max: usize,
        _policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        let list: Vec<serde_json::Value> = self
            .titles_for(student.student_name())
            .into_iter()
            .map(|t| {
                serde_json::json!({"priNo": student.student_key(), "userName": student.student_name(),
                    "pNo": self.paper_key(&t.title), "testingNo": self.attempt_key(&t.title),
                    "score": 100, "_id": format!("r{}", self.attempt_key(&t.title)),
                    "testingName": t.title, "applyDate": "2026-11-09 10:00:00"})
            })
            .collect();
        let raw = serde_json::Value::Array(list)
            .to_string()
            .replace('\\', "\\\\")
            .replace('\'', "\\'");
        Ok(textbook_response(format!(
            "<form name=userForm><input name=pri_no value={TEACHER_KEY}></form><script>const results={{testResultList:JSON.parse('{raw}')}};</script>"
        )))
    }
}
