//! The single code-to-Korean verdict table, the lesson renderer and the
//! reusable display lint (DESIGN-P8-v2 part 1).
use crate::envelope::{LessonData, StudentLesson};
use serde::{Serialize, Serializer};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Found,
    NotObserved,
    NotObservedInEitherSource,
    NotInReadRange,
    NotYetVisible,
    Excluded,
    Eligible,
    NotEligible,
    Unknown,
    Unjudged,
    Waiting,
    Completed,
    Persisted,
    NotPersisted,
    Conflict,
    Unresolved,
    Confirmed,
    Deferred,
}

impl Verdict {
    pub fn code(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::NotObserved => "not_observed",
            Self::NotObservedInEitherSource => "not_observed_in_either_source",
            Self::NotInReadRange => "not_in_read_range",
            Self::NotYetVisible => "not_yet_visible",
            Self::Excluded => "excluded",
            Self::Eligible => "eligible",
            Self::NotEligible => "not_eligible",
            Self::Unknown => "unknown",
            Self::Unjudged => "unjudged",
            Self::Waiting => "waiting",
            Self::Completed => "completed",
            Self::Persisted => "persisted",
            Self::NotPersisted => "not_persisted",
            Self::Conflict => "conflict",
            Self::Unresolved => "unresolved",
            Self::Confirmed => "confirmed",
            Self::Deferred => "deferred",
        }
    }
    /// The one code-to-Korean table; codes appear only in `data`.
    pub fn label_ko(self) -> &'static str {
        match self {
            Self::Found => "확인됨",
            Self::NotObserved => "미확인 (읽은 범위에 없음)",
            Self::NotObservedInEitherSource => "미확인 (두 원천 모두 없음)",
            Self::NotInReadRange => "미확인 (읽은 기간에 없음)",
            Self::NotYetVisible => "아직 보이지 않음",
            Self::Excluded => "제외됨",
            Self::Eligible => "클리닉 대상",
            Self::NotEligible => "클리닉 대상 아님",
            Self::Unknown => "알 수 없음",
            Self::Unjudged => "판단 없음",
            Self::Waiting => "대기: 업로드됨, 교사 확인 전",
            Self::Completed => "확인 완료",
            Self::Persisted => "저장 확인됨",
            Self::NotPersisted => "저장되지 않음",
            Self::Conflict => "다른 값으로 저장됨",
            Self::Unresolved => "저장 확인 필요",
            Self::Confirmed => "완료 판정",
            Self::Deferred => "보류 판정",
        }
    }
    pub const ALL: [Verdict; 18] = [
        Self::Found,
        Self::NotObserved,
        Self::NotObservedInEitherSource,
        Self::NotInReadRange,
        Self::NotYetVisible,
        Self::Excluded,
        Self::Eligible,
        Self::NotEligible,
        Self::Unknown,
        Self::Unjudged,
        Self::Waiting,
        Self::Completed,
        Self::Persisted,
        Self::NotPersisted,
        Self::Conflict,
        Self::Unresolved,
        Self::Confirmed,
        Self::Deferred,
    ];
}

impl Serialize for Verdict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

fn how_found_ko(how_found: &str) -> &'static str {
    match how_found {
        "schedule" => "반 요일 일정으로 확인",
        "moved_lesson" => "이동한 수업으로 확인",
        "day_scan" => "날짜를 거슬러 확인",
        _ => "확인",
    }
}

fn strip_dash(line: &str) -> &str {
    line.trim().strip_prefix("- ").unwrap_or(line.trim())
}

fn format_date_ko(iso: &str) -> String {
    // iso is YYYY-MM-DD from the server; render as the design example does.
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3
        && let (Ok(m), Ok(d)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>())
    {
        return format!("{m}월 {d}일");
    }
    iso.to_string()
}

fn student_lines(student: &StudentLesson, prefix: &str) -> Vec<String> {
    let mut lines = Vec::new();
    match &student.previous_lesson {
        Some(prev) => {
            lines.push(format!(
                "{prefix}지난 수업: {} ({})",
                format_date_ko(&prev.date),
                how_found_ko(&prev.how_found)
            ));
            if !prev.progress.is_empty() {
                lines.push(format!("{prefix}진도: {}", strip_dash(&prev.progress)));
            }
            if !prev.homework.is_empty() {
                lines.push(format!("{prefix}숙제: {}", strip_dash(&prev.homework)));
            }
        }
        None => {
            let range = student.searched_range.as_deref().unwrap_or("");
            lines.push(format!(
                "{prefix}지난 수업: 미확인 (읽은 범위에 없음) - 찾은 범위 {range}"
            ));
        }
    }
    for check in &student.checks {
        let label = check.verdict.label_ko();
        let score = match (check.correct, check.total) {
            (Some(c), Some(t)) => format!(" {c}/{t}"),
            _ => String::new(),
        };
        match check.kind.as_str() {
            "prestudy" => lines.push(format!("{prefix}예습영상 {}: {label}{score}", check.scope)),
            "section" => lines.push(format!("{prefix}섹션 {}: {label}{score}", check.scope)),
            _ => lines.push(format!("{prefix}{}: {label}{score}", check.scope)),
        }
    }
    lines
}

/// Render the finished Korean text for `academy_lesson_prepare`.
pub fn lesson_prepare(data: &LessonData) -> String {
    let date_ko = format_date_ko(&data.lesson_date);
    let mut out = Vec::new();
    if let Some(class_name) = &data.class_name {
        out.push(format!("{class_name} {date_ko} 수업 준비"));
        for student in &data.students {
            let prefix = format!("- {} ", student.student);
            for line in student_lines(student, &prefix) {
                out.push(line);
            }
        }
    } else if let Some(student) = data.students.first() {
        out.push(format!("{} {date_ko} 수업 준비", student.student));
        for line in student_lines(student, "- ") {
            out.push(line);
        }
    } else {
        out.push(format!("{date_ko} 수업 준비"));
    }
    out.join("\n")
}

fn book_label_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(가우스|다빈치)( 플러스)? [0-9]+-[0-9]+(-[0-9]+)?").expect("constant")
    })
}
fn unit_code_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[0-9]+-[0-9]+").expect("constant"))
}
fn latin_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[A-Za-z_]{4,}").expect("constant"))
}

/// Lint rule of DESIGN-P8-v2 part 1: no Latin/code word of length >= 4 in
/// `display` except inside a book label or a unit code, and no verdict code.
pub fn display_lint(display: &str) -> Result<(), String> {
    let without_books = book_label_re().replace_all(display, " ");
    let without_units = unit_code_re().replace_all(&without_books, " ");
    if let Some(m) = latin_re().find(&without_units) {
        return Err(format!("display contains non-Korean token: {}", m.as_str()));
    }
    for verdict in Verdict::ALL {
        if display.contains(verdict.code()) {
            return Err(format!("display contains verdict code: {}", verdict.code()));
        }
    }
    Ok(())
}
