//! One tool table (R8): the only registry, routing list and docs source.
use crate::envelope::Envelope;
use serde_json::Value;

/// Model-facing carrier; the following text block alone is the teacher display.
pub const DISPLAY_DIRECTIVE: &str =
    "다음 블록의 모든 줄을 교사에게 그대로 보여 주세요. 앞뒤에 아무것도 덧붙이지 마세요.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    LessonPrepare,
    UnitStatus,
    NotInSlice1,
}
impl Handler {
    /// Slice-1 dispatch for the 13 unimplemented tools; the lesson handler
    /// needs a backend and facts, so it returns None here.
    pub fn unavailable_envelope(self) -> Option<Envelope> {
        match self {
            Self::NotInSlice1 => Some(Envelope::unavailable_tool()),
            Self::LessonPrepare | Self::UnitStatus => None,
        }
    }
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static str,
    pub handler: Handler,
}

pub static TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "academy_lesson_prepare",
        description: "오늘 수업 준비·체크리스트: 지난 숙제와 예습·섹션·시험 점검, 못 가져온 숙제를 보여줍니다. 현황만이면 academy_unit_status입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"class_name":{"type":"string","description":"반"},"date":{"type":"string","description":"날짜"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"}},"additionalProperties":false}"##,
        handler: Handler::LessonPrepare,
    },
    ToolSpec {
        name: "academy_unit_status",
        description: "소단원별 현황·DailyTest·섹션 결과와 오답·예습 올렸어?: 소단원마다 시험·섹션·클리닉·예습을 봅니다. 체크리스트면 academy_lesson_prepare입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"},"unit":{"type":"string","description":"소단원. 교사가 말한 활동명(필수예제·유형다지기·실력쌓기·대단원총괄)은 소단원 번호와 함께 unit에 넣습니다."},"unit_from":{"type":"string","description":"시작"},"unit_to":{"type":"string","description":"끝"},"topic":{"type":"string","description":"항목","enum":["전체","DailyTest","클리닉","섹션","예습"]}},"additionalProperties":false,"required":["student"]}"##,
        handler: Handler::UnitStatus,
    },
    ToolSpec {
        name: "academy_record_read",
        description: "수업일지 보여줘·저장 됐어?: 그날 수업일지와 지난 저장 확인을 보여줍니다. 새 저장이면 academy_save입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"date":{"type":"string","description":"날짜"}},"additionalProperties":false,"required":["student"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_record_write",
        description: "수업일지 써줘·고쳐줘: 진도·숙제·메모로 미리보기를 만들고 빠진 평가를 한 번에 묻습니다. 조회면 academy_record_read입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"date":{"type":"string","description":"날짜"},"progress":{"type":"array","items":{"type":"string"},"description":"진도 한 줄에 하나씩"},"homework":{"type":"array","items":{"type":"string"},"description":"숙제 한 줄에 하나씩"},"memo":{"type":"string","description":"메모. 없으면 없음"},"attendance":{"type":"string","description":"출석","enum":["출석","결석","지각","미입력"]},"daily_test":{"type":"string","description":"시험 평가","enum":["미확인","미실시","10점","20점","30점","40점","50점","60점","70점","80점","90점","100점"]},"prev_homework":{"type":"string","description":"이전 숙제","enum":["미확인","미숙제","C","B","B+","A","A+"]},"replace":{"type":"boolean","description":"교체 여부"},"restart":{"type":"boolean","description":"새로 시작"}},"additionalProperties":false}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_weekly_read",
        description: "주간 보고·CISM 빈 칸 남았어?: 그 주나 달의 원문과 빈 칸을 봅니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"week":{"type":"string","description":"주"},"month":{"type":"string","description":"달"}},"additionalProperties":false,"required":["student"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_weekly_write",
        description: "주간 보고 써줘: 빈 칸에 교사 문장을 넣은 미리보기를 만듭니다. 조회면 academy_weekly_read입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"week":{"type":"string","description":"주"},"additions":{"type":"string","description":"넣을 문장"},"restart":{"type":"boolean","description":"새로 시작"}},"additionalProperties":false}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_save",
        description: "저장해줘·확인하고 저장해줘: 미리보기를 저장하고 다시 읽어 확인합니다. 상태만 물으면 academy_record_read입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"date":{"type":"string","description":"날짜"},"teacher_words":{"type":"string","description":"교사의 저장 지시 문장 그대로"}},"additionalProperties":false,"required":["student","teacher_words"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_prestudy_report",
        description: "예습영상 확인해줘·예습 올렸어?: 예습 상태와 교사 전용 리포트를 봅니다. 전체 현황이면 academy_unit_status입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"unit":{"type":"string","description":"소단원"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"}},"additionalProperties":false,"required":["student"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_item",
        description: "이 문항 뭐야·정답이랑 어느 섹션: 문항 정답과 원래 섹션을 알려줍니다. 섹션 점수면 academy_unit_status입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"labels":{"type":"array","items":{"type":"string"},"description":"문항 번호 목록"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"}},"additionalProperties":false,"required":["labels"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_textbook",
        description: "목차 줘·교재 찾기: 판본과 전체 단원 목록을 봅니다. 학생 현황이면 academy_unit_status입니다.",
        input_schema: r##"{"type":"object","properties":{"book":{"type":"string","description":"교재. 예 가우스 5-1-2"},"unit":{"type":"string","description":"소단원"}},"additionalProperties":false,"required":["book"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_fact",
        description: "완료 처리·안 가져옴·제외·반 요일: 교사 사실을 기록해 체크리스트에 반영합니다.",
        input_schema: r##"{"type":"object","properties":{"kind":{"type":"string","description":"종류","enum":["반 요일","반 명단","학생 교재","용어 뜻","완료 판정","보류 판정","숙제 안 가져옴","섹션 제외","섹션 제외 해제","수업 이동","유형 학습 완료"]},"student":{"type":"string","description":"학생"},"class_name":{"type":"string","description":"반"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"},"unit":{"type":"string","description":"소단원"},"topic":{"type":"string","description":"항목","enum":["전체","DailyTest","클리닉","섹션","예습"]},"date":{"type":"string","description":"날짜"},"date_to":{"type":"string","description":"옮길 날짜"},"weekdays":{"type":"string","description":"요일"},"term":{"type":"string","description":"용어"},"meaning":{"type":"string","description":"용어 뜻"},"note":{"type":"string","description":"문장"}},"additionalProperties":false,"required":["kind"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_history",
        description: "지난 기록 종합·시험 메타데이터·아카이브 됐어?: 저장 기록을 출처·기준일과 봅니다. 수업일지면 academy_record_read입니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"scope":{"type":"string","description":"범위","enum":["종합","시험 메타데이터","아카이브"]},"date":{"type":"string","description":"날짜"}},"additionalProperties":false,"required":["student"]}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_paper_plan",
        description: "유형 후보·학습지 계획: 공부한 유형 후보와 쪽수 계획을 만듭니다. 인쇄 안 합니다.",
        input_schema: r##"{"type":"object","properties":{"student":{"type":"string","description":"학생"},"book":{"type":"string","description":"교재. 예 가우스 5-1-2"},"unit":{"type":"string","description":"소단원"},"count":{"type":"integer","description":"문항 수"},"restart":{"type":"boolean","description":"새로 시작"}},"additionalProperties":false}"##,
        handler: Handler::NotInSlice1,
    },
    ToolSpec {
        name: "academy_help",
        description: "유형 10이 무슨 뜻이야·서버 되나·출석 메모 정리해줘: 용어 뜻, 서버 상태, 도구 없는 요청을 알려줍니다.",
        input_schema: r##"{"type":"object","properties":{"topic":{"type":"string","description":"주제"}},"additionalProperties":false}"##,
        handler: Handler::NotInSlice1,
    },
];

pub fn tools_json() -> Value {
    serde_json::from_str(&tools_list_minified()).expect("generated tools list is valid JSON")
}

/// Minified `tools/list` value in fixture key order, built only from TOOLS.
pub fn tools_list_minified() -> String {
    let mut out = String::from("{\"tools\":[");
    for (i, tool) in TOOLS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        out.push_str(&serde_json::to_string(tool.name).expect("name"));
        out.push_str(",\"description\":");
        out.push_str(&serde_json::to_string(tool.description).expect("description"));
        out.push_str(",\"inputSchema\":");
        out.push_str(tool.input_schema);
        out.push('}');
    }
    out.push_str("]}");
    out
}

pub fn dispatch(name: &str) -> Option<Handler> {
    TOOLS.iter().find(|t| t.name == name).map(|t| t.handler)
}

pub fn docs_table() -> String {
    let mut out = String::from("| tool | description | bytes |\n| --- | --- | ---: |\n");
    for tool in TOOLS {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            tool.name,
            tool.description,
            tool.description.len()
        ));
    }
    out
}
