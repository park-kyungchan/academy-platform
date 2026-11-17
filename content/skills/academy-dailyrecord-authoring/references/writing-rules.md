# DailyRecord writing rules

The rules come from the newest records the teacher accepted, which win over older practice. Rule IDs match the alignment rule sheet of the parity task. Each example shows the old form, an arrow, and the form to write.

## Tone (T)

- **T1. Polite parent register.** `합니다체` throughout. No casual speech, no note to the teacher, no system wording.
- **T2. A progress or homework line is one item, not prose.**
  - A progress line ends with its status (`완료`, `진행 중`, `진행`) or a score.
  - A homework line ends with the task (`… 해오기`, `… 업로드`, `… 풀이`, `… N문항`).
  - A progress field may instead end every line with `~했습니다.`. One field never mixes the two forms.
  - Example: `- 가우스 중1-1-2, 6-2 정비례 관계 y=ax DailyTest 기본·발전 완료`.
- **T3. lessonMemo is whole polite sentences.** One notice per line; every sentence ends in `~니다.`
  - Example: `- 수요일(YYYY-MM-DD)에 학원서버 프린트 대단원총괄평가(60분 시간 제한)를 진행할 예정입니다.`
- **T4. No internal words.** Banned: `채점`, `미실시`, `미완료`, `교사 확인`, `교사 검토`, `완료판정`, `시간 부족`, `확인 전`, `시리즈`/`버전`/`v` with a number, edition codes such as `1P6A`, and `22개정`. Rewrite each with the same meaning.
  - `집에서는 채점하지 않고 등원 후 채점합니다.` → memo `- 집에서는 답을 맞춰 보지 않고 등원 후 학원에서 확인합니다.`
  - `(지난시간 미완료 숙제)` → `(지난 시간에 끝내지 못한 숙제)`
- **T5. Concise.** No repeated word or phrase, no heading or label line (`해당범위`, `대단원 총괄평가:`), no quotation marks around the record's own text.
  - `4단원 대단원총괄평가 대단원 TEST 대단원 총괄평가` is wrong.
- **T6. Say nothing new and drop nothing.** No praise, advice or fact that the source lacks.
- **T7. A memo carries notices only.** No internal comment about scope or planning.

## Form (F)

- **F1. Line grammar.** Every nonempty line starts with `- `. One LF between lines. No blank line, `'''` fence, indented sub-line, numbered list or table.
- **F2. One content unit per line, with the book on that line.** Never a book alone on a header line.
  - `- 가우스 중1-1-2` + `- 4-1 순서쌍과 좌표평면 (기본) 개념백지테스트 완료` → `- 가우스 중1-1-2, 4-1 순서쌍과 좌표평면 (기본) 개념백지테스트 완료`
  - A line whose activity has no book stays bookless: `- 오답클리닉을 완료했습니다.`, `- 하브루타 완료`.
- **F3. Line shape.** `- <book>, <unit code> <unit title> <activity> <status>`, e.g. `- 가우스 6-2-1, 2-4-1 비례식의 성질 DailyTest 기본 완료`.
- **F4. The deadline line.** The first homework line is `- YYYY-MM-DD까지`, without a weekday. A missing deadline stays missing.
  - `- YYYY-MM-DD까지 다빈치 5-1-1 3-1 대응 관계 (1) 실력다지기·실력올리기 풀이` → `- YYYY-MM-DD까지` + `- 다빈치 5-1-1, 3-1 대응 관계 (1) 실력다지기·실력올리기 풀이`
- **F5. Several deadlines.** Undated entries first. Then each deadline line once, followed by its entries.
- **F6. Joiners.** `·` joins the sections, units or levels of one item; `~` marks a range: `필수예제·유형다지기`, `4-1~4-4`, `DailyTest 기본·발전` (not `(기본/발전)`).
- **F7. Dates in sentences.** `요일(YYYY-MM-DD)`, e.g. `금요일(YYYY-MM-DD)에`; `12월 3일(수)에` is the old form. Clock times stay as written.
- **F8. Order.** Keep the old order of items in a field. The deadline lines of F4 and F5 are the only reordering.
- **F9. Length.** At most 400 characters per text field (a 460-character homework was rejected once). If the text needs more, do not save; list the record.

## Mapping (M)

- **M1. Field ownership.** Done in the lesson → `progress`. Assigned for next time → `homework`. Plans, carry-overs, reasons, requests, how-to sentences and context for a score → `lessonMemo`.
  - `…발전 25점, 발전 오답 수정은 12월 4일(금) 수업에서 이어서 진행 예정` → progress `…발전 25점` + memo `- (분수)×(자연수)의 계산 DailyTest 발전 오답 수정은 금요일(YYYY-MM-DD) 수업에서 이어서 진행할 예정입니다.`
- **M2. A carry-over is a plan.**
  - `- 소인수분해 오답/유사 클리닉·Daily Test 미실시로 다음 수업으로 이월` → memo `- 소인수분해 오답클리닉·유사클리닉과 DailyTest는 다음 수업에서 진행합니다.`
- **M3. Book names.** Brand first, then `중` for middle school or nothing for elementary, then `grade-term-volume`. Write the volume only if the source states it (`N권` or a third number).
  - `초5-2 가우스 2권` → `가우스 5-2-2`; `가우스5-2-1` → `가우스 5-2-1`; `가우스 중1-1 1권` → `가우스 중1-1-1`; `22개정 가우스 중1-1-2` → `가우스 중1-1-2`; `중1-1 가우스 플러스` → `가우스플러스 중1-1`.
- **M4. Davinci keeps its literal.** Reorder and space a Davinci name, but never add or drop a number in it, and never infer a middle-school Davinci series.
- **M5. Unit codes and titles are kept literally.**
  - Never convert between a two-part and a three-part code. A DailyTest line keeps its DailyTest's own code: `2-4-1 비례식의 성질`.
  - Add no code or title that the source lacks.
  - A chapter assessment is `N단원 대단원총괄평가`.
  - A bare list number loses its period: `7. 일차방정식의 풀이(응용)` → `7 일차방정식의 풀이(응용)`.
  - `*` becomes `×`.
- **M6. Vocabulary.** Write the left form, never the forms on the right:
  - `DailyTest` ← `Daily Test`, `[DailyTest]`
  - `개념백지테스트` ← `개념 백지테스트`, `백지테스트`
  - `오답클리닉`, `유사클리닉` ← `Clinic`, `오답/유사 클리닉`
  - `대단원총괄평가` ← `대단원 총괄평가`, `대단원 총괄`
  - `Daily Restudy` ← `Daily restudy`, `DailyRestudy`
  - `오답풀이` ← `오답 풀이`
  - Section names keep the book's spelling: `개념정복`, `개념예제`, `기본문제`, `필수예제`, `유형다지기`, `실력쌓기`, `실력다지기`, `실력올리기` are distinct.
- **M7. DailyTest lines.** `<book>, <code> <title> DailyTest <기본|발전> [score] <status>`, levels joined by `·`: `DailyTest 기본 75점·발전 100점`.
  - `- [DailyTest] [기본] [가우스 중1-1 1권] 1-2 소인수분해 50점(4문항 중2문항 정답)` → `- 가우스 중1-1-1, 1-2 소인수분해 DailyTest 기본 50점(4문항 중 2문항 정답)`
  - DailyTest exists for Gauss books only. Add a book to a DailyTest line only when the source names exactly one full Gauss book.
- **M8. A clinic is a print, not a book.** A clinic line names the books whose units it covers, joined by ` · `: `- 가우스 6-2-1 · 다빈치 6-1-1, 숙제 오답클리닉 완료`. Add no book the source does not name.
- **M9. Assessments stay distinct.** Never merge or rename the chapter test in the book, the printed chapter test from the LMS (`학원서버 프린트 대단원총괄평가`), `학력인증평가`, `월말평가` and DailyTest.
- **M10. Units of measure.** `N문항` (never `N문제`), `N문항 중 K문항 정답`, `N점`, `p.N~p.M`, `N회차`, `N번`.
  - `(1·2회, 8문제 중 7정답)` → `(1·2회, 8문항 중 7문항 정답)`
- **M11. Codes.** Keep printed-paper codes (`(#<code>)`), item numbers, item IDs and phone numbers exactly. Remove edition labels.
