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
// Contract vocabulary whose required chapter/label evidence is absent from v2.
pub const CHAPTER_SCOPE: &str = "대단원";
pub const DESCRIPTIVE_WORK: &str = "서술형 정복";
pub const CHAPTER_CHECK: &str = "총괄평가 전 검사";
pub const CHAPTER_NEXT: &str = "대단원 검사 여부 확인";
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
pub fn identified_grade(correct: usize, total: usize) -> String {
    format!(
        "정답 {} {correct}개 / 전체 {} {total}개",
        UNITS[0], UNITS[0]
    )
}
pub fn unknown(reason: &str) -> String {
    format!("미확인({reason})")
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

pub const DISPLAY_FAILURE: &str =
    "표시문을 올바른 형식으로 만들지 못했습니다. 범위를 줄여 다시 요청해 주세요.";
pub const TOOL_UNAVAILABLE: &str = "이 도구는 아직 제공되지 않습니다.";
pub fn unknown_tool(names: &str) -> String {
    format!("그런 이름의 도구는 없습니다. 쓸 수 있는 도구: {names}")
}
pub fn no_homework(student: &str) -> String {
    format!("{student}의 지난 수업에는 숙제가 배정되지 않았습니다.")
}
pub fn unread_homework(student: &str) -> String {
    format!("{student}의 지난 배정 숙제를 읽을 수 없습니다.")
}
pub fn unread_scope(scope: &str) -> String {
    format!("{scope}: 원천을 읽지 못해 읽은 원천에 포함하지 않았습니다.")
}
pub fn omitted(student: &str) -> String {
    format!(
        "{student}부터 생략했습니다: 표시 한도(60줄·6,000바이트)를 넘어 학생 블록 경계에서 중단했습니다."
    )
}

pub fn omitted_reminders(student: &str) -> String {
    format!(
        "{student}의 알림부터 생략했습니다: 표시 한도(60줄·6,000바이트)를 넘어 알림 행 경계에서 중단했습니다."
    )
}

pub const VALUE_BAR: &str = "｜";
pub const VALUE_BREAK: &str = " / ";
pub const VALUE_COMMENT_OPEN: &str = "＜!--";
pub const VALUE_COMMENT_CLOSE: &str = "--＞";

// Unit status words and complete sentence templates (slice 2).
pub const UNIT_TOPICS: [&str; 5] = ["전체", "DailyTest", "클리닉", "섹션", "예습"];
pub const UNIT_ACTIVITIES: [&str; 4] = ["필수예제", "유형다지기", "실력쌓기", "대단원총괄"];
pub const UNIT_LEVELS: [&str; 2] = ["기본", "발전"];
pub const UNIT_TOPIC_ASK: &str = "조회할 항목을 정하지 못했습니다.\n어느 항목을 볼까요?";
pub const UNIT_TOPIC_MEANING: &str = "이 항목의 현황을 봅니다.";
pub const UNIT_ASK: &str = "조회할 소단원 범위를 정하지 못했습니다.\n어느 소단원을 볼까요?";
pub const UNIT_MEANING: &str = "이 소단원의 현황을 봅니다.";
pub const UNIT_TREE_STOP: &str = "교재 판본 전체 목차를 확인하지 못했습니다. 교사에게 해당 판본의 전체 목차 등록을 요청해 주세요.";
pub const UNIT_SCOPE_STOP: &str =
    "조회할 소단원 후보가 없습니다. 교사에게 해당 판본의 소단원 등록을 요청해 주세요.";
pub const UNIT_BOOK_STOP: &str =
    "요청한 교재가 학생에게 등록되어 있지 않습니다. 등록된 교재의 판본 이름을 확인해 주세요.";
pub const UNIT_BOOK_MEANING: &str = "이 교재의 클리닉 현황을 봅니다.";
pub const UNIT_BOOK_OTHER_MEANING: &str = "이 교재의 현황을 봅니다.";
pub const UNIT_DATE_UNKNOWN: &str = "미확인(날짜 미확인)";
pub const UNIT_LEVEL_UNKNOWN: &str = "미확인(수준 표기 없음)";
pub const UNIT_RECORDED_UNKNOWN: &str = "기록 날짜 미확인";
pub const UNIT_CONFIRMED: &str = "완료 판정";
pub const UNIT_TEACHER_ONLY: &str = "미확인(교사만 확인 가능)";
pub const UNIT_NOT_EXPECTED: &str = "미확인(아직 확인할 단계 아님)";
pub const UNIT_DASH: &str = "—";
pub const UNIT_DASH_LEGEND: &str = "—: 미확인(읽은 섹션 결과에 없음)";
pub const UNIT_CLINIC_TITLE: &str = "오답클리닉 대상 섹션";
pub const UNIT_DAILY_TITLE: &str = "일일시험 현황";
pub const UNIT_DAILY_GRID: &str = "소단원 | 일일시험 원천";
pub const UNIT_CLINIC_GRID: &str = "소단원 | 필수예제 | 유형다지기 | 실력쌓기";
pub const UNIT_SECTION_TITLE: &str = "섹션 결과";
pub const UNIT_TREE_FACT: &str = "판본 전체 목차";
pub const UNIT_SECTION_SCOPE: &str = "오답노트와 시험지 검색 각 첫 관측 페이지·관측 섹션 성적";
pub const UNIT_TRUNCATION: &str = "표시 한도로 생략한 첫 소단원";
pub const UNIT_SOURCE_UNKNOWN: &str = "읽지 못한 원천 또는 범위";
pub const UNIT_SAME_ATTEMPT: &str = "같은 시험 시도인지: 미확인";
pub const UNIT_STAGE_NAMES: [&str; 6] = [
    "오답클리닉 풀이",
    "오답클리닉 채점",
    "유사클리닉",
    "발전클리닉",
    "CISM 지원",
    "인쇄·배부",
];
pub fn unit_book_ask(who: &str, count: usize) -> String {
    let count = if count == 2 {
        "두 권".to_string()
    } else {
        format!("{count}권")
    };
    format!("{who}에게 등록된 교재가 {count}입니다.\n어느 교재의 클리닉을 볼까요?")
}
pub fn unit_book_other_ask(who: &str) -> String {
    format!("{who}의 조회 교재를 정하지 못했습니다.\n어느 교재를 볼까요?")
}
pub fn unit_source(score: Option<f64>) -> String {
    score.map_or_else(
        || UNAVAILABLE.to_string(),
        |s| format!("{SERVER}: 원천 점수 {s}"),
    )
}
pub fn unit_judgment(j: &crate::unit_data::TeacherJudgment) -> String {
    use crate::unit_data::UnitVerdict;
    let at = j
        .recorded_at
        .as_deref()
        .map(unit_date_literal)
        .unwrap_or_else(|| UNIT_RECORDED_UNKNOWN.into());
    match j.verdict {
        UnitVerdict::Confirmed => format!("{UNIT_CONFIRMED}({at})"),
        UnitVerdict::Deferred => format!("미확인(판정 보류·{at})"),
        _ => UNJUDGED.into(),
    }
}
pub fn unit_daily_range(from: &str, to: &str, selector: &str, page: &str) -> String {
    format!(
        "일일시험({}부터 {}까지의 {selector} {page}·전체 이력 아님)",
        date(from),
        date(to)
    )
}
pub fn unit_fact_judgment(
    unit: &str,
    level: Option<&str>,
    j: &crate::unit_data::TeacherJudgment,
) -> String {
    let scope = format!(
        "{unit}{}",
        level.map(|v| format!(" {v}")).unwrap_or_default()
    );
    match j.verdict {
        crate::unit_data::UnitVerdict::Confirmed => format!(
            "{scope} 완료 판정: {}",
            j.recorded_at
                .as_deref()
                .map(|at| crate::render::cell(&unit_date_literal(at)))
                .unwrap_or_else(|| UNIT_RECORDED_UNKNOWN.into())
        ),
        crate::unit_data::UnitVerdict::Deferred => format!("{scope} 보류 판정"),
        _ => format!("{scope} 판정 미기록"),
    }
}
pub fn unit_expected(unit: &str, level: &str) -> String {
    format!("{unit} {level} 당일 확인 대상")
}
pub fn unit_exclusion(unit: &str, activity: &str) -> String {
    format!("{unit} {activity} 제외")
}
pub fn unit_source_line(scope: &str, value: &str) -> String {
    format!("{scope} 원천: {value}")
}
pub fn unit_date_line(scope: &str, at: Option<&str>) -> String {
    format!(
        "{scope} 시험 날짜: {}",
        at.map(unit_date_literal)
            .unwrap_or_else(|| UNIT_DATE_UNKNOWN.into())
    )
}
pub fn unit_level_line(scope: &str, level: Option<&str>) -> String {
    format!("{scope} 시험 수준: {}", level.unwrap_or(UNIT_LEVEL_UNKNOWN))
}
pub fn unit_judgment_line(scope: &str, value: &str) -> String {
    format!("{scope} 교사 판정: {value}")
}
pub fn unit_section_identity(chapter: &str, middle: Option<&str>, minor: &str) -> String {
    format!(
        "대단원 {chapter}{} 소단원 {minor}",
        middle.map(|v| format!(" 중단원 {v}")).unwrap_or_default()
    )
}
pub fn unit_section_details(
    title: &str,
    correct: usize,
    total: usize,
    score: f64,
    source_date: &str,
) -> Vec<String> {
    vec![
        format!("원문 섹션 제목: {title}"),
        format!("정답 수/전체 문항 수: {correct}/{total}문항"),
        format!("원천 점수: {score}"),
        format!("원천 날짜: {source_date}"),
    ]
}
pub fn unit_item(ordinal: &str, scoring: &str, label: &str) -> String {
    format!("원문 문항 순번 {ordinal}: {scoring} · 문항 번호 {label}")
}
pub fn unit_source_date(literal: Option<&str>, precision: &str) -> String {
    match literal {
        Some(v) if precision == "month_day" => format!("{v}(월·일 표기·채점 시각 미확인)"),
        Some(v) => format!("{v}(채점 시각 미확인)"),
        None => UNIT_DATE_UNKNOWN.into(),
    }
}
pub fn unit_absent(code: &str, value: &str) -> String {
    format!("{code} 섹션: {value}")
}
pub fn unit_clinic_basis(code: &str, activity: &str, labels: &str) -> String {
    format!("{code} {activity} 대상 근거: 원문 오답 문항 번호 {labels}")
}
pub fn unit_clinic_excluded(code: &str, activity: &str) -> String {
    format!("{code} {activity}: 교사가 제외한 섹션")
}
pub fn unit_clinic_judgment(code: &str, value: &str) -> String {
    format!("{code} 오답클리닉 교사 판정: {value}")
}
pub fn unit_next_daily_range(from: &str, to: &str) -> String {
    format!("{from}부터 {to}까지의 시험 기록 확인.")
}
pub fn unit_next_daily(scope: &str, found: bool) -> String {
    if found {
        format!("{scope}의 새 원천 점수에 대한 교사 판정 확인.")
    } else {
        format!("{scope} 시험 기록의 다음 조회 결과 확인.")
    }
}
pub fn unit_next_clinic(scopes: &str) -> String {
    format!("{scopes}의 오답클리닉 교사 판정 확인.")
}
pub fn unit_next_clinic_judgments(scopes: &str) -> String {
    format!("{scopes} 오답클리닉의 교사 판정 확인.")
}
pub fn unit_next_section(code: &str, activity: Option<&str>, absent: bool) -> String {
    if absent {
        format!("{code} 섹션 제목과 결과의 교사 확인.")
    } else {
        format!(
            "{code}{} 원문 오답 문항의 교사 확인.",
            activity.map(|v| format!(" {v}")).unwrap_or_default()
        )
    }
}
pub fn unit_next_generic() -> String {
    "조회 범위의 원천 기록과 교사 판정 확인.".into()
}
pub fn unit_next_line(next: &str) -> String {
    format!("다음 확인: {next}")
}
pub fn unit_footer(at: &str, source: &str, who: &str, facts: &str) -> String {
    format!("읽은 원천 ({at}): {source},\n학생·교재 등록({who}), 교사 사실({facts}).")
}
pub fn unit_section_source(who: &str, book: &str, scope: &str, titles: bool) -> String {
    format!(
        "{}({who}·{book}·{scope})",
        if titles { "섹션 제목" } else { "섹션" }
    )
}
pub fn unit_section_absent_scope(code: &str) -> String {
    format!("오답노트와 시험지 검색 각 첫 관측 페이지·{code} 제목 미관측")
}
pub fn unit_section_small_scope(code: &str, activity: Option<&str>) -> String {
    match activity {
        Some(a) => format!("{code} {a} 관측 성적·원문 순번과 문항 번호"),
        None => format!("{code}의 관측 섹션 성적·원문 순번과 문항 번호"),
    }
}
pub fn unit_cached_tree(at: &str) -> String {
    format!("판본 전체 목차: 원래 읽은 시각 {at}")
}
pub fn unit_clinic_missing_facts(codes: &str) -> String {
    format!("{codes} 판정 미기록")
}
pub fn unit_unknown_line(reason: &str) -> String {
    format!("{UNIT_SOURCE_UNKNOWN}: {reason}.")
}
pub fn unit_truncation(code: &str) -> String {
    format!("{UNIT_TRUNCATION}: {code}.")
}
pub fn unit_daily_heading(who: &str, book: &str, leaf: Option<&str>) -> String {
    format!(
        "{who} {book} {}{UNIT_DAILY_TITLE}",
        leaf.map(|s| format!("{s} "))
            .unwrap_or_else(|| "소단원별 ".into())
    )
}
pub fn unit_unknown_reason(code: &str) -> &'static str {
    match code {
        "daily_header_unavailable" | "daily_mapping_unavailable" => {
            "일일시험 제목·판본 연결 미확인"
        }
        "section_ambiguous_attempts" => "동일 제목의 시험 시도를 구분하지 못함",
        "section_mapping_unavailable" => "단원·활동 연결 미확인",
        "prestudy_name_ambiguous" => "같은 이름의 학생을 예습 원천에서 구분하지 못함",
        "prestudy_unavailable" => "예습 목록을 읽지 못함",
        "daily_unavailable" => "일일시험을 읽지 못함",
        "daily_partial_page" => "일일시험의 일부 관측 페이지를 읽지 못함",
        "section_grade_unavailable" => "섹션 성적을 읽지 못함",
        "section_unavailable" => "섹션을 읽지 못함",
        _ => "시험지 검색을 읽지 못했거나 제목 연결을 확인하지 못함",
    }
}

pub const UNIT_SOURCE_NOT_OBSERVED: &str = "미확인(읽은 원천에 없음)";

pub fn unit_stage_name(stage: &str) -> &str {
    match stage {
        "dailytest" => "일일시험",
        "chapter_print" => "서버 대단원 인쇄",
        "prestudy" => "예습",
        "original_clinic_solving" => "오답클리닉 풀이",
        "original_clinic_grading" => "오답클리닉 채점",
        "similar_clinic" => "유사클리닉",
        "advanced_clinic" => "발전클리닉",
        "cism_support" => "CISM 지원",
        "printed" => "인쇄",
        "handout" => "배부",
        _ => stage,
    }
}

pub fn unit_source_name(name: &str) -> &str {
    match name {
        "dailytest" => "일일시험",
        "dailytest_relative_page" => "일일시험 상대기간 관측 페이지",
        "dailytest_header" => "일일시험 제목",
        "section_index" => "섹션 제목",
        "section_grades" => "섹션 성적",
        "prestudy" => "예습 목록",
        _ => name,
    }
}
pub fn unit_source_observation(name: &str, at: &str, scope: &str) -> String {
    format!("{name}({at}·{scope})")
}

/// Only recognized ISO dates are formatted; other native literals remain whole.
pub fn unit_date_literal(value: &str) -> String {
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(value) {
        return date(&at.date_naive().format("%Y-%m-%d").to_string());
    }
    if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
        date(value)
    } else {
        value.to_string()
    }
}

pub const UNIT_RELATIVE_PAGE: &str = "상대기간 첫 관측 페이지·기간 범위 미확인";

pub const UNIT_WRONG_NOTE_PAGE: &str = "오답노트 첫 관측 페이지";

pub const UNIT_NEXT_SUPPLEMENTARY: &str = "교재 자체 평가와 교사 판정 확인.";

// Source-qualified slice-2 repair vocabulary.
pub const UNIT_MINOR: &str = "소단원";
pub const UNIT_GRADE_UNREAD: &str = "미확인(성적을 읽지 못함)";
pub const UNIT_ATTEMPTS_AMBIGUOUS: &str = "미확인(동일 제목의 시험 시도 구분 불가)";
pub const UNIT_EXCLUSION_SCOPE_UNKNOWN: &str = "미확인(섹션 제외의 시험 시도 범위 미확인)";
pub const UNIT_STUDENT_REQUIRED: &str =
    "이 도구는 학생 한 명의 현황을 조회합니다. 반 이름 대신 학생 이름을 알려 주세요.";
pub const UNIT_FROM_ASK: &str = "시작 소단원을 정하지 못했습니다.\n어느 소단원부터 볼까요?";
pub const UNIT_TO_ASK: &str = "끝 소단원이 없거나 시작보다 앞섭니다.\n어느 소단원까지 볼까요?";
pub fn unit_unmapped_title(title: &str) -> String {
    format!("단원·활동 연결 미확인: {title}")
}
pub fn unit_timed_scope(scope: &str, at: &str) -> String {
    format!("{scope}·읽은 시각 {at}")
}
pub fn unit_snapshot_scope(attempt: Option<(usize, Option<String>)>) -> String {
    match attempt {
        Some((i, date)) => format!(
            "관측 시도 {i}({})",
            date.unwrap_or_else(|| UNIT_DATE_UNKNOWN.into())
        ),
        None => "이전 관측 시도(현재 범위에서 연결 미확인)".into(),
    }
}
pub fn unit_verdict(v: crate::unit_data::UnitVerdict) -> &'static str {
    use crate::unit_data::UnitVerdict;
    match v {
        UnitVerdict::Confirmed => UNIT_CONFIRMED,
        UnitVerdict::Completed => TEACHER,
        UnitVerdict::NotYetExpected => UNIT_NOT_EXPECTED,
        UnitVerdict::TeacherReportedNoServer => UNIT_TEACHER_ONLY,
        _ => verdict(v.code()),
    }
}
pub fn observation_time(at: &str) -> String {
    let local = crate::render::local_time(at);
    if local
        .get(..10)
        .is_some_and(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
        && local.get(11..16).is_some()
    {
        format!("{} {}", date(&local), &local[11..16])
    } else {
        crate::render::cell(at)
    }
}

pub const UNIT_TITLES_OBSERVED: &str =
    "오답노트와 시험지 검색 각 첫 관측 페이지·관측 제목·성적 미확인";
pub const UNIT_WRONG_NOTE_TITLES: &str = "오답노트 첫 관측 페이지·관측 제목·성적 미확인";

pub const UNIT_INDEX_SCOPE: &str = "오답노트와 시험지 검색 각 첫 관측 페이지·관측 제목";
pub fn unit_unmapped_summary(count: usize) -> String {
    format!("단원·활동 연결 미확인: 제목 {count}개(표시 한도로 원문 제목 생략·결과 자료에 보존).")
}
