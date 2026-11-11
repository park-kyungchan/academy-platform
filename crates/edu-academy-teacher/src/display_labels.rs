//! The one vocabulary and sentence table for lesson preparation.
pub const PARTS: [&str; 4] = ["알림", "학생별 할 일", "미확인 사항", "다음 확인"];
pub const NONE: &str = "없음";
pub const ACTION_HEADER: &str = "학생 | 범위 | 할 일 | 상태 | 근거 | 다음 확인";
pub const REMINDER_HEADER: &str = "학생 | 기한 시각 | 할 일 | 상태 | 근거 | 다음 확인";
pub const CLOSING: &str = "이번 결과는 읽기만 했고 저장한 것은 없습니다.";
pub const TEACHER: &str = "교사 확인";
pub const SERVER: &str = "서버 확인";
pub const PHYSICAL: &str = "미확인(실물 확인 전)";
pub const WAITING: &str = "미확인(업로드됨·교사 확인 전)";
pub const UNJUDGED: &str = "미확인(판단 필요)";
pub const DEFERRED: &str = "미확인(판정 보류)";
pub const MISSING_VIDEO: &str = "미확인(두 목록에 없음)";
pub const MISSING_TITLE: &str = "미확인(읽은 제목 목록에 없음)";
pub const NOT_OBSERVED: &str = "미확인(읽은 범위에 없음)";
pub const NOT_IN_RANGE: &str = "미확인(읽은 기간에 없음)";
pub const NOT_IN_EITHER: &str = "미확인(두 원천 모두에 없음)";
pub const NOT_YET: &str = "미확인(아직 보이지 않음)";
pub const NOT_APPLICABLE: &str = "해당 없음";
pub const UNAVAILABLE: &str = "미확인(원천을 읽지 못함)";
pub const SOLVING: &str = "풀이";
pub const GRADING: &str = "채점";
pub const CORRECTION: &str = "오답수정";
pub const REQUIRED: &str = "필수예제";
pub const PRACTICE: &str = "유형다지기";
pub const ADVANCED: &str = "실력쌓기";
pub const CHAPTER: &str = "대단원총괄";
pub const START: &str = "시작";
pub const TODAY: &str = "오늘";
pub const PREVIOUS_DATE: &str = "지난 수업일";
pub const DUE_PAST: &str = "기한 지남";
pub const DUE_SOON: &str = "임박";
pub const NEW_STUDENT: &str = "전입생(이름 미정)";
pub const FACTS: &str = "교사 사실";
pub const ASSIGNMENT: &str = "배정 기록";
pub const VIDEO: &str = "예습영상";
pub const VIDEO_CHECK: &str = "영상 확인";
pub const VIDEO_NEXT: &str = "영상 내용 확인";
pub const VIDEO_WAIT: &str = "영상 대기 목록";
pub const VIDEO_DONE: &str = "영상 완료 목록";
pub const VIDEO_BOTH: &str = "영상 대기·완료 목록";
pub const PRESTUDY_NOTE: &str = "예습노트";
pub const PRESTUDY_CHECK: &str = "예습노트 검사";
pub const CONCEPT_NEXT: &str = "개념 정리 확인";
pub const SOLUTION_NOTE: &str = "풀이노트";
pub const SOLUTION_CHECK: &str = "풀이노트 검사";
pub const SOLUTION_NEXT: &str = "풀이 과정 확인";
pub const NOTE_NEXT: &str = "풀이노트 확인";
pub const SECTION_CHECK: &str = "섹션 결과 확인";
pub const SECTION_TITLES: &str = "섹션 제목 목록";
pub const SECTION_NEXT: &str = "제목·성적 확인";
pub const CARRY_CHECK: &str = "미지참 판정";
pub const CARRY_NEXT: &str = "판정 기록";
pub const FOLLOW_UP: &str = "수업 후속";
pub const FOLLOW_NEXT: &str = "미처리 내용 확인";
pub const ACADEMY_CORRECTION: &str = "학원 내 수정";
pub const CORRECTION_CHECK: &str = "오답수정 일정 확인";
pub const CORRECTION_NEXT: &str = "실시 여부 확인";
pub const REMINDER_NEXT: &str = "처리 여부 확인";
pub const PENDING: &str = "미처리";
pub const UNITS: [&str; 6] = ["문항 번호", "문항", "시도", "시험지", "숙제", "회차"];
pub const DATE_QUESTION: &str = "날짜를 알려주세요.";
pub const TODAY_MEANING: &str = "오늘 날짜";
pub const SOURCE_FAILURE: &str = "원천을 읽지 못했습니다.\n잠시 후 다시 요청해 주세요.";
pub const CLASS_QUESTION: &str = "어느 반인가요?";
pub const CLASS_MEANING: &str = "반 전체 준비";
pub const BOOK_QUESTION: &str = "어느 교재인가요?";
pub const BOOK_MEANING: &str = "그 학생에게 등록된 교재";
pub const NO_CLASS: &str = "반 조건 없음";
pub const ALL_REMINDERS: &str = "미처리 알림 전체";
pub const SCHEDULE: &str = "반 일정";
pub const ROSTER: &str = "명단";
pub const STUDENT_FACTS: &str = "학생 사실";
pub const REGISTRATION: &str = "학생·교재 등록";
pub const STUDENT_INFO: &str = "학생 정보";
pub const RECORDS: &str = "수업 기록";
pub const TITLES: &str = "섹션 제목";
pub const NO_GRADES: &str = "성적 미조회";
pub const BOTH_LISTS: &str = "대기/완료 목록";
pub const BOTH_LISTS_SINGLE: &str = "대기/완료목록";
pub const EMPTY_ROSTER: &str = "비어 있는 명단";
pub const BELONGS: &str = "소속 반";
pub const FACT: &str = "사실";
pub const EACH_STUDENT: &str = "각 학생";
pub const NO_RECORD: &str = "기록 미관측";
pub const DATE_RESOLUTION: &str = "날짜 해석";
pub const ASSIGNMENT_READ: &str = "배정 확인";

pub fn date(iso: &str) -> String {
    let date = iso.get(..10).unwrap_or(iso);
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() == 3
        && let (Ok(month), Ok(day)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>())
    {
        return format!("{month}월 {day}일");
    }
    date.into()
}
pub fn title(who: &str, day: &str) -> String {
    format!("{who} {} 수업 준비", date(day))
}
pub fn academy(who: &str) -> String {
    format!("{who} 학원 내 처리")
}
pub fn assigned(stage: &str) -> String {
    format!("배정 {stage} 확인")
}
pub fn stage_next(stage: &str) -> String {
    format!("{stage} 상태 확인")
}
pub fn count_labels(n: usize) -> String {
    format!("{} {n}개", UNITS[0])
}
pub fn anonymous_grade(c: usize, t: usize) -> String {
    format!("채점 결과 {c}/{t}{}", UNITS[1])
}
pub fn carry_scope(scope: &str, count: &str, assigned: &str) -> String {
    format!("{scope} {count} ({} 숙제)", date(assigned))
}
pub fn missing_previous(who: &str) -> String {
    format!("{who}의 지난 수업 기록이 없어 지난 수업일을 확인할 수 없습니다.")
}
pub fn missing_homework(who: &str) -> String {
    format!("{who}의 지난 배정 숙제를 확인해야 합니다.")
}
pub fn empty_roster(who: &str) -> String {
    format!("{who}의 명단이 비어 있어 준비할 학생을 확인할 수 없습니다.")
}
pub fn unread_grades(scopes: &str, plural: bool) -> String {
    if plural {
        format!("{scopes}의 섹션 성적은 해당 제목이 보이지 않아 읽지 않았습니다.")
    } else {
        format!("{scopes} 섹션 성적은 해당 제목이 보이지 않아 읽지 않았습니다.")
    }
}
pub fn reminder_next(who: &str, urgency: &str, time: &str, action: &str) -> String {
    format!("- {who}의 {urgency} {time} {action} 알림을 처리하세요.")
}
pub fn video_next(who: &str, unit: &str) -> String {
    format!("- {who}의 {unit} 예습영상 내용을 확인하세요.")
}
pub fn notebook_next(who: &str, unit: &str) -> String {
    format!("- {who}의 {unit} 풀이노트를 확인하세요.")
}
pub fn previous_next(who: &str) -> String {
    format!("- {who}의 지난 수업일과 배정 숙제를 확인하세요.")
}
pub fn roster_next(who: &str) -> String {
    format!("- {who}의 명단을 확인하세요.")
}
pub fn footer(time: &str) -> String {
    format!("{READ_SOURCES} ({time}): ")
}
pub fn unread_record(who: &str) -> String {
    format!("{who}의 지난 수업 기록을 읽지 못했습니다.")
}
pub fn outside(date_: &str, from: &str, to: &str) -> String {
    format!(
        "{}은 조회 허용 기간인 {}부터 {}까지의 범위 밖입니다.\n이 기간 안의 수업일을 알려주세요.",
        date(date_),
        date(from),
        date(to)
    )
}
pub fn roster_mismatch(class: &str, student: &str) -> String {
    format!(
        "{class}의 반 명부와 학생 기록이 {student} 학생의 반에 대해 일치하지 않아 반 전체 수업 준비를 중단했습니다. 반 명부를 수정하거나 학생 이름을 지정해 주세요."
    )
}
pub fn roster_missing(class: &str, student: &str) -> String {
    format!(
        "{class}의 반 명부에 있는 {student} 학생의 소속 반이 학생 기록에 없습니다. 반 명부를 수정하거나 학생 이름을 지정해 주세요."
    )
}
pub fn disagreement(student: &str, actual: &str, requested: &str) -> String {
    format!(
        "{student}의 소속 반은 {actual}인데 요청에는 {requested}가 적혀 있습니다.\n{student}를 어느 반 조건으로 준비할까요?"
    )
}
pub fn class_answer(student: &str, class: &str) -> String {
    format!("반 조건을 {class}로 바로잡고 {student}를 준비합니다.")
}
pub fn no_class_answer(student: &str) -> String {
    format!("반 조건을 빼고 {student}만 준비합니다.")
}
pub fn verdict(code: &str) -> &'static str {
    match code {
        "waiting" => WAITING,
        "completed" | "found" => SERVER,
        "confirmed" => TEACHER,
        "deferred" => DEFERRED,
        "unjudged" => UNJUDGED,
        "not_observed" => NOT_OBSERVED,
        "not_observed_in_either_source" => NOT_IN_EITHER,
        "not_in_read_range" => NOT_IN_RANGE,
        "not_yet_visible" => NOT_YET,
        "not_applicable" => NOT_APPLICABLE,
        "excluded" => EXCLUDED,
        "eligible" => ELIGIBLE,
        "not_eligible" => NOT_ELIGIBLE,
        "persisted" => PERSISTED,
        "not_persisted" => NOT_PERSISTED,
        "conflict" => CONFLICT,
        "unresolved" => UNRESOLVED,
        _ => UNAVAILABLE,
    }
}
pub fn search_dates(dates: &[String]) -> String {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for iso in dates {
        let parts: Vec<_> = iso.split('-').collect();
        if parts.len() != 3 {
            continue;
        }
        let month = parts[1].parse::<u32>().unwrap_or(0).to_string();
        let day = format!("{}일", parts[2].parse::<u32>().unwrap_or(0));
        if let Some((_, days)) = groups.last_mut().filter(|(m, _)| m == &month) {
            days.push(day);
        } else {
            groups.push((month, vec![day]));
        }
    }
    groups
        .into_iter()
        .map(|(m, days)| format!("{m}월 {}", days.join("·")))
        .collect::<Vec<_>>()
        .join(", ")
}
pub const GAUSS: &str = "가우스";
pub const SCHEDULE_SUFFIX: &str = "일정";

pub fn possessive(who: &str, unit: &str) -> String {
    format!("{who}의 {unit}")
}
pub const BOOK_PATTERN: &str = r"(가우스|다빈치)( 플러스)? [0-9]+-[0-9]+(-[0-9]+)?";
pub const DATE_PATTERN: &str = r"^(\d{1,2})월\s*(\d{1,2})일$";

pub fn anonymous_assignment_count(text: &str) -> String {
    let re = regex::Regex::new(r"(\d+)문항").expect("constant");
    re.captures(text)
        .map_or(String::new(), |c| format!("{} {}개", UNITS[1], &c[1]))
}
pub const EXCLUDED: &str = "제외됨";
pub const ELIGIBLE: &str = "클리닉 대상";
pub const NOT_ELIGIBLE: &str = "클리닉 대상 아님";
pub const PERSISTED: &str = "저장 확인됨";
pub const NOT_PERSISTED: &str = "저장되지 않음";
pub const CONFLICT: &str = "다른 값으로 저장됨";
pub const UNRESOLVED: &str = "저장 확인 필요";
pub const GRADES_READ: &str = "성적 조회";

pub const READ_SOURCES: &str = "읽은 원천";
pub const STUDENT_MISSING: &str = "학생을 찾지 못했습니다. 학생 이름과 반을 확인해 주세요.";
pub const STUDENT_AMBIGUOUS: &str =
    "이름, 반, 교재가 같아 두 학생을 구분할 수 없습니다. 반 명부를 수정해 주세요.";
pub const STUDENT_QUESTION: &str = "어느 학생인가요?";
pub const STUDENT_MEANING: &str = "그 반의 학생";
pub const ACTUAL_CLASS_QUESTION: &str = "학생과 반이 일치하지 않습니다. 어느 반인가요?";
pub const ACTUAL_CLASS_MEANING: &str = "그 학생이 속한 반";
