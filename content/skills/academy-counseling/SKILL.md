---
name: academy-counseling
description: "Use when the teacher prepares, holds or closes a counseling with a parent or a student (상담 준비, 상담 양식, 학부모 상담, 상담 결과, 상담 종료): the form read from the student's memory, the result written back after the call, a copy only at the teacher's word, and the follow-up as a reminder, through edu-academy-memory counsel."
---

# Academy counseling

The user is the teacher. A counseling is one conversation with a parent or a student about one student, on one date. The memory keeps it as one object keyed `<slug>|<date>` with the status `prepared`, `held` or `closed`. The command `edu-academy-memory` reads and writes it; its subcommands, output and refusal codes are in the README of the crate `edu-academy-memory`. Lesson work and its rules (one `academy_operations` call at a time, the one LMS account, effects only at the teacher's request, failures) are those of the skill `academy-lesson-day`: this skill adds only the counseling.

## 1. Prepare

1. `edu-academy-memory brief <slug>`: the student's card, bindings, newest observations and notes, open reminders and the newest counselings. Read the student's file under `consultations/` of edu-lessons as the skill `academy-lesson-day` reads it. Do not read the per-student memory files under `students/<slug>/`. The brief's `memory_as_of` is the last lesson date the memory closed: name it in the form's background, and treat an observation of a later date as not yet closed.
2. A live read only for a fact the form needs that the brief does not hold, or that the teacher asks to be read fresh, as section 2 of `academy-lesson-day` says: `due` first, the binding's exact keys, one call at a time, `observed` after each read. A DailyRecord is read with `edu lesson record <date> <student>` and its fallbacks as that section's part "DailyRecord through `edu lesson`" says, and an observation older than the `captured_at` of the same date's `edu lesson` read is stale there: the form uses the newer reading, after it was written back.
3. Write the form in Korean, in these sections, in this order; leave out a line no labelled source fills, and label each fact as `academy-lesson-day` labels it:

```text
[상담 준비] <학생> | <YYYY-MM-DD> | <학부모/학생>
1. Purpose (상담 목적): <교사가 말한 목적>
2. Background (배경): <상담에 이른 사정, 이전 상담>
3. Lessons so far (지금까지의 수업): <교재·단원별 진행, 날짜>
4. Results (결과): <섹션 결과, DailyTest, 클리닉, 예습: 출처와 observedAt>
5. Homework (숙제): <수행 현황, 미숙제>
6. Points for the call (상담 포인트): <교사가 전할 것, 물을 것>
```

4. Show the form to the teacher. Ask, with the CLI's question tool where the CLI has one, whether a copy of the form leaves the host and where to. A copy leaves the host only at the teacher's word.
5. When the teacher accepts the form, write it to a file and run `edu-academy-memory counsel prepare <slug> <date> --purpose <text> --form @<file>`, with `--stored-at <where the copy went>` when a copy left the host. `counsel prepare` runs once per counseling: the engine refuses a second one for the same student and date (`revision_conflict`), so `--stored-at` cannot be added later. A copy the teacher asks for after the prepare is made only at the teacher's word, and the teacher hears that the memory cannot record where it went.

## 2. After the call

1. The teacher's words about the call become `edu-academy-memory counsel result <slug> <date> --result @<file>`: what the teacher said was discussed and agreed, in the teacher's words; add nothing. The status becomes `held`.
2. A follow-up is a reminder: `edu-academy-memory remind open <id> --text <text> --student <slug> [--due <date>] --provenance teacher_confirmed` (an id `R-YYYYMMDD-<KEY>`, at most 32 bytes). A follow-up that is done is `edu-academy-memory remind close <id>`.
3. When the teacher says the matter is closed, `edu-academy-memory counsel close <slug> <date>`. The status becomes `closed`.

A message to a parent is an effect: it happens only when the teacher asks for it in this session, and never as a side effect of a counseling step.

## 3. Rules

- Every write prints `{"action","key","outcome"}`; `already_applied` means the store held the values. A refusal (`{"error":{...}}`, exit 2, or `engine_missing`, exit 3) is told to the teacher with its code and field, and the same call is not repeated unchanged. `counseling_missing` from `counsel result` or `counsel close` means no counseling was prepared for that student and date.
- Keep credentials, answer-key literals and parent contact details out of the form, the result and every file you write.
- Write in Korean to the teacher; keep command names, field names and codes in their original form.
