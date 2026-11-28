---
name: academy-dailyrecord-authoring
description: "Use when writing, aligning or saving an Academy DailyRecord (수업일지 작성, 일지 정리, 일지 저장): the writing rules of each field, and the order intake, review, save and readback that the save tools require."
---

# Academy DailyRecord authoring

A DailyRecord (`수업일지`, `일지`) is the record of one student's lesson that the parent reads. This skill says what to write in each field and what the author does between the tool calls. The tool descriptions of `academy_dailyrecord_get` (its views `'record'` and `'intake'`), `academy_dailyrecord_review` and `academy_dailyrecord_save` give the arguments and the call order: follow them, do not copy them here.

This skill keeps calling `academy_dailyrecord_get` directly, never `edu lesson`, which only the skill `academy-lesson-day` uses for a lesson's DailyRecord read: its read before `academy_dailyrecord_review` (view `'record'` or `'intake'`, for the `observationHandle`), its readback after a save and the views `'status'` and `'reconcile'` stay on `academy_dailyrecord_get`.

Two kinds of work use the same rules:
- **Writing** a new record from the teacher's statements for one lesson.
- **Aligning** an old record: a rewrite that changes wording, form, order and field mapping only. The facts stay.

## 1. The fields

| Field | What it holds | Who changes it |
| --- | --- | --- |
| `progress` (`진도`) | What the student did in this lesson: textbook work, tests, clinics, checks | Author |
| `homework` (`숙제`) | What is due, deadline line first | Author |
| `lessonMemo` (`메모`, `코멘트`) | Notices to the parent: plans, schedules, reasons, requests, explanations; `""` for none | Author |
| `attendance` | `Y` present, `L` late, `N` absent, blank unset | Only on the teacher's explicit request for this field |
| `dailyTestAssessment` | `-1` unknown, `0` not given, `1`-`10` = 10-100 points | Only on the teacher's explicit request for this field |
| `previousHomeworkAssessment` | The grade of the previous lesson's homework, stored on the previous record: `-1` unknown, `0` homework not done (`미숙제`; the LMS page labels it `미실시`), `1` C, `2` B, `3` B+, `4` A, `5` A+ | Only on the teacher's explicit request for this field |

An alignment rewrite selects only `progress`, `homework` and `lessonMemo`. A code field changes only when the teacher explicitly asks to change that field. If a code field looks wrong (for example `-1` for DailyTest on a day without one, which the teacher sets to `0`), list it for the teacher instead of changing it.

## 2. The writing rules

The full rules, with examples, are in [references/writing-rules.md](references/writing-rules.md). Read it before writing. In short:

- **Tone.** Polite parent register (`합니다체`). No note to the teacher, no system wording, no internal words (the banned list is in the reference).
- **Lines.** Every line starts with `- `; lines are separated by one line-feed character (U+000A); no blank line, fence, indent, number or table. A text field never holds the two characters backslash and `n` in place of a line feed (ERR-20261116-002): in a JSON value the line feed is written as the escape `\n`, which the tool reads as one character, never as `\\n`. Check the readback for a literal backslash.
- **One item per line, with its book.** `- <book>, <unit code> <unit title> <activity> <status>`. A progress line ends with its status or score; a homework line ends with the task; a memo line is a whole sentence ending in `니다.`.
- **Mapping.** Done in the lesson → `progress`. Assigned for next time → `homework`. Plans, carry-overs, reasons, requests and how-to sentences → `lessonMemo`.
- **Names.** Canonical book names, vocabulary and units as the reference lists them. Unit codes and titles are kept literally. A progress line names a unit that `academy_textbook_catalog_get` with view `'toc'` lists for its book.
- **Length.** At most 400 units per text field, where every character above U+007F (every Korean character) counts 2: about 200 Korean characters. Longer text is refused with `invalid_explicit_field_value`.

## 3. Facts that must not change

When aligning, compare the union of `progress`, `homework` and `lessonMemo`, old against new, because sentences move between fields:

1. The three code fields: the readback after a save equals the readback before it.
2. Every date (ignore weekdays).
3. Every number with its unit: score, question count, correct count, item number, page, attempt, unit number, clock time, unit code.
4. Every book: brand, school stage, grade, term, volume.
5. Every printed-paper code (`#…`), item ID, phone number and `HH:MM` time.
6. An empty field stays empty; a filled field stays filled unless the mapping moves all of it.
7. Every activity, unit title, test, score, status (`완료`, `진행 중`, `예정`, `미숙제`), plan, request and reason, with the same meaning, and nothing else: no praise, advice or new fact.

When writing a new record, the facts are the teacher's words for that lesson and what the server shows for it. Add nothing else.

If a rule cannot be met without changing a fact, do not save that record: list it for the teacher with the reason.

## 4. Order of the work

1. **Read.** `academy_dailyrecord_get` (view `'record'`) for the student and lesson date. Keep the old text of every field: it is the original that must be restorable. The old text comes from this read, never from the memory's brief or an `edu lesson` answer, which may be older.
2. **Draft.** Write the new text of the fields that change, by the rules. Check the facts of section 3 yourself.
3. **Show the teacher.** Show old and new text per field, and every record you skipped with its reason. Drafting, review, saving and sending to a parent are separate steps.
4. **Review.** `academy_dailyrecord_review`, directly after a fresh `academy_dailyrecord_get` (view `'record'` or `'intake'`); otherwise the review holds `stale_source`. The review takes only `context`, `observationHandle`, `cismObservationHandle` and `proposals`. Never get past a blocker by changing a fact or by moving text to a field the rules do not allow.
5. **Save** only when the teacher asks to save in this session and no hold in the day's `STATE.md` forbids it: `academy_dailyrecord_save`, one student and date at a time. The save holds `request_instruction_conflict` unless the instruction it is given, in either form the tool takes (a string or `{text}`), is the text recorded for that `requestRef`.
6. **Read back.** `academy_dailyrecord_get` again. Compare each saved field with the reviewed text and the code fields with step 1. Report the result per field. A code's label is the answer's `assessmentCodeLabels` entry, not a label from memory.
7. **Write the memory.** After a save that the readback confirms, run `edu-academy-memory observed <slug> dailyrecord <lesson_date> --at <observedAt of the readback, UTC to the second> --summary <what the saved record says> --tool academy_dailyrecord_get`, as the skill `academy-lesson-day` says. This observation is newer than the date's `edu lesson` capture: the next `edu lesson` read of that date carries `--refresh`, as `academy-lesson-day` says.

An uncertain save (connection lost, unknown whether it landed) is never repeated blindly: `academy_dailyrecord_get` with view `'status'` and then view `'reconcile'` settle it, and the teacher hears the outcome.

## 5. Report to the teacher

Write in Korean; keep field names and codes in their original form.

```text
[<학생> YYYY-MM-DD 수업일지]
- progress: 변경 / 유지
  - 이전: <old text>
  - 새 문안: <new text>
- homework: ...
- lessonMemo: ...
- 코드 필드(attendance, dailyTestAssessment, previousHomeworkAssessment): 변경 없음
- 저장: 안 함 / 저장 후 재조회 일치 / 불일치(<필드>)
- 확인 필요: <건너뛴 이유 또는 교사에게 물을 것>
```
