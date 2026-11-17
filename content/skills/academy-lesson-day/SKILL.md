---
name: academy-lesson-day
description: "Use when the user starts or runs Academy lesson work (학원실무 시작, 수업 준비, 수업 1부/2부 체크리스트, 학생 현황, 리마인더): the order of a lesson day in edu-lessons, which academy_operations tools answer each step, and how to report to the teacher in Korean."
---

# Academy lesson day

The user is the teacher. Lesson work happens in `/home/palantirkc/work/edu-lessons/`. This skill gives the order of a lesson day and the shape of the reports. The tool descriptions of `academy_operations` give each tool's arguments and call order: read them there, not here. Related skills: `academy-dailyrecord-authoring` for a DailyRecord, `academy-prestudy-review` for a pre-study video.

## 1. Find the day

1. Read `STATE.md` of edu-lessons. It names the active lesson directory `YYYYMMDD/`.
2. Read that directory's `STATE.md` before any tool call. It holds the day's roster, the classes (`1부`, `2부`), the teacher's statements, the open reminders, the holds and the next action.
3. If the teacher starts a new lesson day and no directory exists for today's date (Asia/Seoul), create `YYYYMMDD/STATE.md` and point the hub `STATE.md` to it.
4. Other `edu-*` folders are engineering work, not lesson instructions.

## 2. Gather in three stages

Stop as soon as the request is answered.

1. **Local files, no tool call.** The day's `STATE.md`, any checklist already written today, earlier days' `STATE.md` and saved DailyRecord drafts, and `consultations/<학생>.md`. List what is missing or may have changed: the gap list.
2. **Decide whether a live read is needed.** Read live only for a gap, or when one of these holds:
   - nothing in the local files was observed today;
   - the student may have uploaded homework or a pre-study video since the last read;
   - the local files disagree with each other, or the teacher corrected them;
   - the teacher asks for a fresh check.
3. **Live reads, one call at a time.** Never call two `academy_operations` tools in parallel: they share one LMS account. Run lesson reads from one session at a time. Per scheduled student, start with at most one `academy_dailyrecord_get` and one `academy_prestudy_get`, then read only the gaps.

| Step of the day | Tool that answers it |
| --- | --- |
| Today's or a past DailyRecord (`수업일지`) | `academy_dailyrecord_get` |
| Pre-study uploads and AI scores | `academy_prestudy_get`, then `academy_prestudy_report_get` |
| Section results (O/X, score) of a book | `academy_textbook_section_results_get` |
| Wrong items and clinic candidates | `academy_clinic_paper_status_get` |
| Which section an item number belongs to | `academy_item_section_trace` |
| An item's official answer | `academy_item_answer_lookup` |
| DailyTest history with paper titles | `academy_dailytest_history_with_headers` |
| A printed paper (`출력코드`) | `academy_printed_paper_lookup`, `academy_paper_definition_lookup`, `academy_paper_headers` |
| A book's units or catalog entry | `academy_textbook_units_get`, `academy_textbook_catalog` |
| Weekly or monthly CISM | `academy_cism_weekly_get`, `academy_cism_month` |

Skip a detailed read of a book or unit that the local files already record as completed by the teacher, unless the teacher asks for it.

## 3. Reading the results

- Label every fact with its source: `SERVER_OBSERVED` with its `observedAt` time, `LOCAL_STATE` with the file, `TEACHER_CONFIRMED` or `TEACHER_REPORTED` with the teacher's words, `PLANNED`, `UNCONFIRMED` or `UNOBSERVED`, `NOT_APPLICABLE`.
- A server score or an `OK` status is not the teacher's completion judgment. Only the teacher confirms that a unit, a clinic or a pre-study check is complete.
- `student_name_not_found` on a date, or partial coverage, does not prove an absence or a service failure. A missing catalog entry does not prove that no work was done.
- An older observation from the local files is a dated snapshot. A fresh live read replaces it only for the same fact of the same occurrence; it does not reopen a later completion the teacher confirmed.
- Never fill a missing value by guessing. Say which tool or field lacks it.
- Without a working tool result, use only what the teacher gave or a saved file, and name its source, date and coverage.

## 4. Failures and gaps

- A tool answers `unavailable`, an error or a `failure`: read its reason code and origin, do not repeat the same call unchanged, and tell the teacher the code and the short cause before continuing work that does not depend on it.
- A step that no tool supports, or a field that no tool returns: say so; do not invent the data.
- Record either one, without student data, in `mcp-error-logs/` or `mcp-backlog/` of edu-lessons, as their `README.md` files say.

## 5. Effects

Reads are the default. A DailyRecord or CISM save, a print, or a message to a parent happens only when the teacher asks for it in this session and the day's `STATE.md` allows it. After a save, read the record back. After an uncertain write, check the target before any retry.

## 6. Report to the teacher

Write in Korean. Keep tool names, field names, codes and status labels in their original form. Short questions get short answers.

A lesson checklist (`체크리스트`) always uses the detailed form of [references/checklist-format.md](references/checklist-format.md), even when the teacher does not ask for history or a tree. If the teacher names a date but not a class, cover every class of that date.

Order of a checklist:
1. Open reminders first, including those of students not on today's roster. Closed reminders stay closed.
2. Per student, the flow of the day: arrival, homework check, carried-over work, planned activities, pre-study status.
3. Per book, the tree `교재 -> 대단원 -> (중단원) -> 소단원 -> 섹션` with all observed history: section results, wrong items, DailyTest attempts, clinic status, and the teacher's judgment kept apart from the server's records.
4. When a student starts a new Gauss minor unit, the pre-study block of `academy-prestudy-review`.
5. What is still to confirm today.

A status report (`학생 현황`) uses the structure in the same reference: a dated title, students numbered, and per book the units in order. An update of one unit keeps the title and numbering and lists only the change of the day. A full class report repeats every student's whole structure. Never add students together into one count without stating what is counted.
