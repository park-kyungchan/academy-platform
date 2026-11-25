---
name: academy-lesson-day
description: "Use when the user starts or runs Academy lesson work (학원실무 시작, 수업 준비, 수업 1부/2부 체크리스트, 학생 현황, 리마인더): the order of a lesson day in edu-lessons, how each student's start reads the memory (edu-academy-memory brief and due) before any live read, which academy_operations tools answer each step, how the readings are written back and the day closed, and how to report to the teacher in Korean."
---

# Academy lesson day

The user is the teacher. Lesson work happens in `/home/palantirkc/work/edu-lessons/`. This skill gives the order of a lesson day and the shape of the reports. The tool descriptions of `academy_operations` give each tool's arguments and call order: read them there, not here. The command `edu-academy-memory` reads and writes each student's memory in the store; its subcommands, output and refusal codes are in the README of the crate `edu-academy-memory`. Related skills: `academy-dailyrecord-authoring` for a DailyRecord, `academy-prestudy-review` for a pre-study video, `academy-counseling` for a counseling.

## 1. Find the day

1. Read `STATE.md` of edu-lessons. It names the active lesson directory `YYYYMMDD/`.
2. Read that directory's `STATE.md` before any tool call. It holds the day's roster, the classes (`1부`, `2부`), the teacher's statements, the open reminders, the holds and the next action.
3. If the teacher starts a new lesson day and no directory exists for today's date (Asia/Seoul), create `YYYYMMDD/STATE.md` and point the hub `STATE.md` to it.
4. Other `edu-*` folders are engineering work, not lesson instructions.

The lesson date of every command below is that directory's date, `YYYY-MM-DD`. A student's slug comes from `edu-academy-memory roster`; where the day's roster names a student whose slug is not plain, the `## student` section of the brief shows the name to compare. A student the store does not know (`student_missing`) is told to the teacher; registering a student is a write the teacher asks for.

## 2. A student's start: memory first, then only the live reads `due` names

Stop as soon as the request is answered.

1. **Memory, no `academy_operations` call.** For each scheduled student, before any live read:
   - `edu-academy-memory brief <slug>`: the student's card, active bindings, newest observations, notes, open reminders and counselings, each with its date.
   - `edu-academy-memory due <slug> <lesson_date>`: `memory_as_of`, the kinds already read for that date (`have`), the kinds not yet read (`due`), and the active bindings with their exact keys (`book_no`, `course_key`, `book_label`).

   Do not read the per-student memory files under `students/<slug>/` (`profile.md`, `textbook-ledger.md`, `progress-log.md`, `CONTEXT.md`): the brief holds what they held. Do read the day's `STATE.md`, any checklist already written today, saved DailyRecord drafts and `consultations/<학생>.md`, which the memory does not take. Earlier days' `STATE.md` are read only for a fact the brief lacks. List what the request needs and neither source holds: the gap list.
2. **Decide which live reads to make.** Make an `academy_operations` read only for a kind that `due` lists, or that the teacher asks to be read fresh, and at most one read per kind and student; a continuation page that an answer asks for (`nextHeaderOffset`) belongs to the same read. A tool of the table below that answers no kind (an item's section or answer, a printed paper, a book's units, CISM) runs only for a question the teacher asked. A kind that `due` does not list was read for this date: use the brief's observation, not a new read. Skip a detailed read of a book or unit that the memory or the local files record as completed by the teacher, unless the teacher asks for it.
3. **Pass the binding's exact keys.** Where a tool takes `bookNo` and `courseKey` (`academy_textbook_section_results`), pass the `book_no` and `course_key` of the binding that `due` printed, never a shortened label. Where a tool takes only a label (`bookLabel` of `academy_textbook_section_results_get` or `academy_clinic_paper_status_get`), pass the binding's `book_label` word for word. A kind read per book (`section_results`, `clinic`) is read for the binding the lesson works in; another book is read only when the teacher asks.
4. **Live reads, one call at a time.** Never call two `academy_operations` tools in parallel: they share one LMS account. Run lesson reads from one session at a time. Read the LMS only through these tools: no browser, direct HTTP or other client.
5. **Write each reading back, directly after it.** After each live read, run `edu-academy-memory observed <slug> <kind> <lesson_date> --at <observedAt> --summary <text> --tool <tool>`, with `--record <id>` where the answer names the record. `--at` is the tool's own `observedAt` in UTC to the second (drop the fraction of a second), never the clock's time. The summary holds what the lesson needs from the answer (scores, wrong item numbers, statuses, titles, dates), at most 1,500 bytes, and no credential, answer-key literal or parent contact. A binding that a read shows (a book's `bookNo`, `courseKey` and bound label, as step 2 of `academy_textbook_section_results_get` gives them) is written with `edu-academy-memory bind <slug> <book_no> --label <bound label word for word> --course-key <courseKey> --at <observedAt> --provenance server_observed`; the command applies nothing when the store already holds it.
6. **The teacher's words.** The teacher's words that should outlast the day go to `edu-academy-memory note <slug> <lesson_date> --provenance teacher_confirmed --text <text or @file>`. A student has one note per date, and a later `note` of that date replaces its text: write the day's whole note each time, the earlier text and the new words. A reminder goes to `edu-academy-memory remind open <id> --text <text> --student <slug> [--due <date>] --provenance teacher_confirmed` (an id `R-YYYYMMDD-<KEY>`, at most 32 bytes; no `--student` for a reminder of the whole class), and a reminder the teacher closes to `edu-academy-memory remind close <id>`.
7. **Close the day.** At the end of the day run `edu-academy-memory day-close <lesson_date> <slug>...` once for the students of the day. It sets each student's `memory_as_of` to the lesson date. It prints one line per student and refuses with `nothing_read` a student of whom the store holds neither an observation nor a note of that date: report every refused student to the teacher, with the code.

The memory writes of this section are the lesson day's record in the store: they are not an effect on the LMS and need no request of their own. They lift no hold of section 6.

| Kind of `due` | Step of the day | Tool that answers it |
| --- | --- | --- |
| `dailyrecord` | Today's or a past DailyRecord (`수업일지`) | `academy_dailyrecord_get` |
| `prestudy` | Pre-study uploads and AI scores | `academy_prestudy_get`, then `academy_prestudy_report_get` (`prestudy_report`) |
| `section_results` | Section results (O/X, score) of a book | `academy_textbook_section_results` with the binding's keys, else `academy_textbook_section_results_get` |
| `clinic` | Wrong items and clinic candidates | `academy_clinic_paper_status_get` |
| `dailytest` | DailyTest history with paper titles | `academy_dailytest_history_with_headers` |
| none | Which section an item number belongs to | `academy_item_section_trace` |
| none | An item's official answer | `academy_item_answer_lookup` |
| none | A printed paper (`출력코드`) | `academy_printed_paper_lookup`, `academy_paper_definition_lookup`, `academy_paper_headers` |
| none | A book's units or catalog entry | `academy_textbook_toc_get`, `academy_textbook_units_get`, `academy_textbook_catalog` |
| none | Weekly or monthly CISM | `academy_cism_weekly_get`, `academy_cism_month` |

## 3. Reading the results

- Label every fact with its source: `SERVER_OBSERVED` with its `observedAt` time, `LOCAL_STATE` with the file, `TEACHER_CONFIRMED` or `TEACHER_REPORTED` with the teacher's words, `PLANNED`, `UNCONFIRMED` or `UNOBSERVED`, `NOT_APPLICABLE`. A line of the brief keeps its own: an observation is `SERVER_OBSERVED` with its `observed_at`, a note or a reminder the label of its provenance (`teacher_confirmed` is `TEACHER_CONFIRMED`, `local_state` is `LOCAL_STATE`, and so on), with its date.
- A server score or an `OK` status is not the teacher's completion judgment. Only the teacher confirms that a unit, a clinic or a pre-study check is complete.
- `student_name_not_found` on a date, or partial coverage, does not prove an absence or a service failure. A missing catalog entry does not prove that no work was done.
- An older observation, from the memory or the local files, is a dated snapshot. A fresh live read replaces it only for the same fact of the same occurrence; it does not reopen a later completion the teacher confirmed.
- `academy_dailytest_history_with_headers` needs `lessonDate` and takes only the periods `"11"` to `"16"`; a selector outside them is refused, so an empty or refused answer for one period is not data. A DailyTest attempt carries no date: never give it one.
- `academy_clinic_paper_status_get` with the view `compact` returns no wrong items; `academy_textbook_section_results` does.
- A result that is a JSON string is parsed before its fields are read.
- `sourceDate.literal` is the month and day a result was applied, not the lesson's day.
- Never fill a missing value by guessing. Before a record is reported missing, run the batch read with the exact `sourceTitles`, and name the call and the field that lack it.
- Without a working tool result, use only what the teacher gave, the memory or a saved file, and name its source, date and coverage.

## 4. Failures and gaps

- A tool answers `unavailable`, an error or a `failure`: read its reason code and origin, do not repeat the same call unchanged, and tell the teacher the code and the short cause before continuing work that does not depend on it.
- `edu-academy-memory` refuses (`{"error":{"code","field","detail"}}`, exit 2) or cannot run its engine (`engine_missing`, exit 3): tell the teacher the code and the field, do not repeat the same call unchanged, and continue the day from the local files and the live reads the gap list needs; a reading that could not be written back is named in the report.
- A step that no tool supports, or a field that no tool returns: say so; do not invent the data.
- Record either one, without student data, in `mcp-error-logs/` or `mcp-backlog/` of edu-lessons, as their `README.md` files say.

## 5. Questions to the teacher

A question to the teacher that is a choice or a field's value is asked with the CLI's question tool where the CLI has one, with the options named; other questions are asked in the reply.

## 6. Effects

Reads are the default. A DailyRecord or CISM save, a print, or a message to a parent happens only when the teacher asks for it in this session and no hold in the day's `STATE.md` forbids it. After a save, read the record back. After an uncertain write, check the target before any retry. Keep credentials, answer-key literals and parent contact details out of every file you write and every text you give to `edu-academy-memory`.

## 7. Report to the teacher

Write in Korean. Keep tool names, field names, codes and status labels in their original form. Short questions get short answers.

A lesson checklist (`체크리스트`) always uses the detailed form of [references/checklist-format.md](references/checklist-format.md), even when the teacher does not ask for history or a tree. If the teacher names a date but not a class, cover every class of that date.

Order of a checklist:
1. Open reminders first, including those of students not on today's roster. Closed reminders stay closed.
2. Per student, the flow of the day: arrival, homework check, carried-over work, planned activities, pre-study status.
3. Per book, the tree `교재 -> 대단원 -> (중단원) -> 소단원 -> 섹션` with all observed history: section results, wrong items, DailyTest attempts, clinic status, and the teacher's judgment kept apart from the server's records.
4. When a student starts a new Gauss minor unit, the pre-study block of `academy-prestudy-review`.
5. What is still to confirm today.

A status report (`학생 현황`) uses the structure in the same reference: a dated title, students numbered, and per book the units in order. An update of one unit keeps the title and numbering and lists only the change of the day. A full class report repeats every student's whole structure. Never add students together into one count without stating what is counted.

At the end of the day, the report names every student that `day-close` refused, with its code.
