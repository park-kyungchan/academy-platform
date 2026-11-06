# Academy term map (Korean labels, Hindsight wording, Academy routes)

Purpose: agents route between the teacher's Korean words, the mixed English and Korean wording
in Hindsight, and the Academy MCP tools. The map is N:M. One Korean term can name several
objects (for example `학원서버 프린트`), and one object has several spellings in Hindsight.

Hindsight counts come from a scan of 18,359 shared-bank memory units on 2026-11-06. Use them as
search aliases, not as definitions. Academy calls always use the exact Korean label that the
tool returned (for example the `bookLabel` candidates of `interview_required`). Hindsight
searches combine an English alias, the Korean label and the student's real name.

User decisions dated 2026-11-06 are marked **U**.

## Books and structure

| Korean | Object | Hindsight aliases | Academy route | Notes |
| --- | --- | --- | --- | --- |
| 가우스 / 다빈치 | Textbook series | Gauss, Davinci, 가우스, 다빈치 | `academy_textbook_section_results_get` with the exact `bookLabel` | `가우스 5-2-1` = grade 5, term 2, volume 1. Edition text such as `(22개정)` stays in the label. |
| 대단원 / 소단원 / 단원 | Chapter / subunit | chapter, major unit, unit, subunit | section titles such as `3-1 ...` | Unit numbers are book-local. Never join two books by number alone. |
| 기본 / 발전 / 심화 | (a) level words inside a book label; (b) DailyTest levels | basic, advanced | (a) `bookLabel`; (b) DailyTest rows | Same words, different objects. |
| PART A / 유형 N | A part or type range inside a section | PART A, type 10 | none (teacher scope) | Source titles do not carry PART boundaries. |

## Sections and assessments

| Korean | Object | Hindsight aliases (count) | Academy route | Notes |
| --- | --- | --- | --- | --- |
| 필수예제 | Required-examples section | 필수예제 (163), required example (30), required-example (18) | section index | |
| 유형다지기 | Type-practice section (Gauss) | 유형다지기 (151), type-practice (14), type practice (9), type drill (4) | section index | |
| 실력쌓기 | Skill-building section (Gauss) | 실력쌓기 (75), skill-building / skill building (4) | section index | |
| 실력다지기 | Skill-strengthening section (Davinci) | 실력다지기 (60), skill-strengthening (2) | section index | |
| 실력올리기 | Skill-raising section (Davinci) | 실력올리기 (53), skill raising (2) | section index | |
| 대단원총괄평가 (교재 내) | In-book chapter assessment section | chapter assessment (155), comprehensive assessment (35), 대단원총괄평가 (32), major-unit (16) | section index (`wrong_answer_note` source, per-item `lectureKey`) | **U**: the server `단원평가` row with the same score and date is this same exam entered on the server. Link the two records; do not count them as two exams. |
| 단원평가 (서버 행) | Server paper attempt of the in-book chapter assessment | unit assessment (4), unit-assessment (7), 단원평가 (77) | student paper search (`paperKey`, `attemptKey`) | Same exam as the row above (**U**). |
| 학원서버 프린트 | **Four kinds (U):** 대단원총괄평가 프린트, 오답·유사클리닉 프린트, DailyTest 프린트, 월말평가 프린트 | print, printed paper | `academy_printed_paper_lookup` by print code (for example `#839RF`) | Ask which kind when the teacher does not say. |
| 대단원총괄평가 프린트 | Printed chapter assessment (for example 16 questions) | printed chapter assessment | print code | Distinct from the 20-question in-book section. It may have no attempt row; the teacher's report is the evidence. |
| 오답클리닉 | Original-error clinic | error clinic (71), original-error clinic (60), 오답클리닉 (50), wrong-answer clinic (11) | `academy_clinic_paper_status_get` | The server has no clinic score input; completion comes from the teacher. |
| 유사클리닉 | Similar-problem clinic | similar clinic (72), 유사클리닉 (36), similar-problem clinic (10) | `academy_clinic_paper_status_get` | Whether a similar clinic was generated is not visible on the server. |
| 클리닉 프린트 | Printed 오답·유사 clinic papers | clinic print | as above | |
| DailyTest (DT) | Per-subunit daily test (기본/발전) | DailyTest, DT | `academy_daily_test_inventory`, `academy_dailytest_history_with_headers`, `academy_student_metadata_get` daily history | Titles can be empty. `paper_scope_unresolved` is an intentional boundary. Rows entered by another student are excluded only on the teacher's word. |
| 월말평가 | Monthly assessment | monthly assessment (44), 월말평가 (12) | paper search / print code | |
| 개념백지테스트 | Concept blank-sheet test | blank test (106), 개념백지 (13), blank-sheet (2) | none (teacher report) | Completing the test and its score are separate facts. |
| 오답노트 (원천) | The wrong-answer-note source of the section index | wrong_answer_note | section index `indexBasis` | Merged with `student_paper_search`; the index is bounded, not a full history. |

## Prestudy, lesson and DailyRecord

| Korean | Object | Hindsight aliases (count) | Academy route | Notes |
| --- | --- | --- | --- | --- |
| 예습 / 예습영상 | Prestudy video upload | prestudy (108), preview (184, also ordinary English) | `academy_prestudy_get`, `academy_prestudy_report_get` (AI report score) | `teacherJudgment` is always null; moving to the completed list is not a teacher review. |
| 풀이노트 검사 | Solution-notebook inspection | solution note (30), solution notebook (28), 풀이노트 (6) | none (teacher) | |
| 하브루타 | Havruta (paired explanation) | 하브루타 (72), havruta (52) | none (teacher) | |
| 등원 / 출석 | Arrival / attendance class | arrival, attendance | DailyRecord attendance field | Observed values: `Y`, `L` (late). |
| 수업1부 / 2부 | Lesson session 1 / 2 | session1, session2 | business date + class | |
| 진도 / 숙제 / 메모 | DailyRecord progress / homework / memo | progress, homework, memo | `academy_dailyrecord_get` → intake → review → save → readback | Save stays a separate, instructed effect. |
| 이전 숙제 평가 | Previous-homework assessment option | previous-homework assessment | DailyRecord option | Observed codes: `5` = A+ (2026-11-02), `3` = B+ (October 30 evidence). |
| DailyTest 평가 | DailyRecord DailyTest option | DailyTest assessment | DailyRecord option | Observed codes: `0` = 미실시, `9` = 90점대 (2026-11-02). |
| 교재 카탈로그 | Textbook catalog and sample PDF | textbook catalog | `academy_textbook_catalog`, `academy_textbook_pdf` | A catalog row is not an assignment. |
| CISM 주간 보고 | CISM weekly report | CISM weekly | `academy_cism_weekly_get` / `_review` / `_save` | |

## Ambiguous or easily mistaken references

| Teacher wording | Candidates | Action |
| --- | --- | --- |
| `학원서버 프린트` | the four print kinds above | Ask which kind. |
| `총괄`, `대단원총괄` | in-book section (= server 단원평가 row), 대단원총괄평가 프린트 | Ask in-book or print. |
| `클리닉` | 오답클리닉, 유사클리닉, 클리닉 프린트 | Ask, unless the teacher says both. |
| A book with volume or term (`5-1-2`, `5-2-1`) | the student's registered `bookLabel` candidates | If it is not among them, ask; the teacher has typed the wrong volume before. |
| `기본` / `발전` | book level word or DailyTest level | Resolve from context, otherwise ask. |
| `예습` vs `preview` in Hindsight | prestudy upload, or the ordinary English word | Search with `prestudy` and `예습`. |

## Not mapped (U)

The completion phrases `풀이 완료`, `채점 완료`, `오답까지 완료`, `오답 수정 완료`, `검토 완료`,
`완료판정`, `교사 최종완료` and `앱 입력 완료` are not mapped onto each other. Record the
teacher's exact words. When a decision needs the difference, ask the teacher.
