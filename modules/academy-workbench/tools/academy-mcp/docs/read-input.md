# Read tool input: the words a teacher or a model uses

Source: `src/read-input.ts`. Tests: `tests/read-input-flex.test.ts`. What these inputs change in the release (registered schemas, `release/surface.json`): [release v6 differences](release-v6-differences.md#input-schemas).

## Request identity

`purpose` and `currentRequestRef` are optional on every read tool of the module. The DailyRecord write tools keep their `context`; `academy_dailyrecord_intake` also takes the context members, and the scope written out as `sourceNamespace`, `teacherId`, `studentId` and `lessonDate`, at the top level (its flat form, [release v6 differences](release-v6-differences.md#input-schemas)). When a member is left out, the server fills it:

- `purpose`: the tool's name;
- `currentRequestRef`: `auto-<uuid>`, new for each call.

The answer carries the values used. A supplied value works as before. `academy_review_type_questions` and `academy_pool_question_content` take `purpose` from a fixed list, so the default there is `teacher_requested_review`.

## Dates

The date members (`lessonDate` of `academy_dailyrecord_get`, `academy_student_resolve`, `academy_student_metadata_get` and `academy_dailytest_history_with_headers`; `scope.lessonDate` of `academy_student_metadata`; `period.from` and `period.through` of the metadata queries; `date` of `academy_cism_weekly_get`) accept:

| Form | Example |
| --- | --- |
| `YYYY-MM-DD` | `2026-11-16` |
| today | `today`, `오늘` |
| yesterday | `yesterday`, `어제` |
| month and day | `11월 16일`, `11월 19일(목)`, `11/16`, `11-16` |

The server resolves each date in Asia/Seoul before any source read.

- A month and day takes the year among last, this and next year that puts the date nearest to today. On a tie it takes the earlier date. For example, on 2026-12-30 `1월 2일` is 2027-01-02; on 2026-01-03 `12월 30일` is 2025-12-30.
- A weekday after the date is accepted but not checked against the date.
- The answer states the resolved `YYYY-MM-DD`. When the read fails after the date was resolved, the failure answer states it too: `lessonDate` (`academy_student_metadata`, `academy_student_metadata_get`, `academy_student_resolve`, `academy_dailyrecord_get`, `academy_dailytest_history_with_headers`) or `date` (`academy_cism_weekly_get`).
- Any other text, or a date that does not exist (`11월 31일`), is refused with the one code `date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day`.

The write tools `academy_cism_weekly_review` and `academy_cism_weekly_save` still take an exact `YYYY-MM-DD`.

## Book names

`bookLabel` of `academy_textbook_section_results_get`, `academy_item_section_trace` and `academy_clinic_paper_status_get` accepts the bound label or a loose name.

1. A binding whose label equals the input byte for byte wins.
2. Otherwise the server reads these parts from the input and from each bound label:
   - the grade and term: `5-1`, `초5-1`, `5학년 1학기`, `중1-1`;
   - a term written alone: `1학기`, `2학기`;
   - the volume: `-2` in `5-1-2`, `2권`;
   - every other word of any length, one letter too, for example the brand `가우스`, `다빈치`, `오일러`, `시그마`, `개념연산` or `쎈`, and `하`. Only the words `교재`, `책`, `수학`, `학년`, `학기`, `권`, `초등`, `중등`, `고등`, `중학`, `개정` and `과정` are left out.

   The curriculum mark (`22개정`, `2022개정`) is ignored. A binding agrees when it has every part of the input: the same grade and term, the term written alone, the volume, and each word as text in its label.
3. The result:
   - exactly one binding agrees: the server uses it. `bookSelection` names it: `{requested, boundLabel, match: "exact" | "loose"}`;
   - several bindings agree: `interview_required` with those labels, and `bookSelection.match` `several`;
   - none agrees: `interview_required` with all bound labels, and `bookSelection.match` `none`.

Example with the bound labels `(22개정) 초5-1 가우스 2권 초5-1 발전 2권(2022개정)` and `(22개정) 초5-1 가우스 1권 초5-1 발전 1권(2022개정)`:

- `가우스 5-1-2`, `초5-1 가우스 2권` and `가우스 5학년 1학기 2권` select the first;
- `가우스 5-1` asks with both;
- `다빈치 5-1-2` asks with both.

With `(22개정) 초5-1 가우스 1권 초5-1 발전 1권(2022개정)` and `(22개정) 초5-1 다빈치 2권(2022개정)` bound, `쎈 5-1-2` and `가우스 하` ask with both: no label has `쎈` or `하`. With `(22개정) 초5-1 가우스 2권 초5-1 발전 2권(2022개정)` and `(22개정) 초5-2 가우스 2권 초5-2 발전 2권(2022개정)` bound, `가우스 2학기` selects the second.
