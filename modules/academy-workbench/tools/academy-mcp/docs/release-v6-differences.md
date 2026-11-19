# Release v6 against v5: saved and dailyrecord endpoints

Release v6 builds its saved endpoint from `src/server.ts` and its dailyrecord endpoint from `src/dailyrecord-server.ts` on `main` (`bun run build:endpoints`). Release v5's tools are recorded in `tests/fixtures/release-v5-tools.json` and its surface in `release/surface.json`. `tests/release-tools.test.ts` compares each v5 tool of these two endpoints with what its program registers now and fails on any difference this page does not list. The paper endpoints' tools are compared separately, with the paper server.

A tool that a program registers and v5 does not route to that program is not a difference: for example `academy_endpoint_identity`, or `academy_textbook_units_get`, which is registered and not on the surface.

## Admission gates (saved endpoint only)

Commit `1a038a6` removed `src/owner-admission.ts` and `src/metadata-admission.ts`: no student list, date window or metadata selector list gates a read. v5's saved bundle was built before that commit and holds both gates; v6's saved bundle holds neither. A launch that still passes `--dailyrecord-admission` or `--metadata-admission` (a path and a SHA-256) is accepted and the arguments gate nothing (`src/dailyrecord-tools.ts`). v5's dailyrecord bundle was built after `1a038a6` and holds neither gate, so the dailyrecord endpoint has no such difference.

## Input schemas

`academy_dailyrecord_get` and `academy_dailytest_history_with_headers` are compositions of the bridge (`src/neutral-server.ts`), so their v6 schemas are the bridge's `dailyRecordGetSchema` and `dailyTestHeaderRouteSchema` as `z.toJSONSchema(…, {io: 'input'})`. The test compares each schema member by member and `required` as a set, so a difference is named by the JSON pointer of the member that differs; every difference lies under a pointer listed below, and every pointer listed has one.

What changed:

| Change | v5 | v6 | Reason |
| --- | --- | --- | --- |
| Request identity: `/properties/purpose`, `/properties/currentRequestRef` and `/required` | both required, strings of 1 to 256 characters | optional: the server fills a member left out (`purpose`: the tool name; `currentRequestRef`: a new `auto-<uuid>`), and the answer returns the values used; the members' descriptions say so | The read tools take the words a teacher or a model uses ([read input](read-input.md)). `academy_student_resolve` and `academy_item_answer_lookup` keep their members without description; only their `required` changes there. |
| `academy_review_type_questions` `purpose` | required, `teacher_requested_review` or `protocol_verification` | optional, default `teacher_requested_review` | The same: a model need not name a purpose from a fixed list. |
| Date members | `YYYY-MM-DD` only: a `pattern` (`academy_cism_weekly_get` `date`: `format: date` and a calendar `pattern`) | a string of 1 to 40 characters with no `pattern` or `format`, described as `YYYY-MM-DD`, `today`/`오늘`, `yesterday`/`어제`, `M월 D일`, `M/D` or `M-D`, resolved in Asia/Seoul; an unreadable date is refused with `date_form_unsupported_use_yyyy_mm_dd_today_yesterday_or_month_day` | The same ([read input](read-input.md#dates)). The metadata queries' `period` sits at `oneOf/0` (`student_papers`) and `oneOf/4` (`printed_paper_metadata`). |
| `academy_dailyrecord_get` `lessonDate` | default `today`, either `today` or a `pattern` date; described as a date that must fall inside the owner-admitted range | default `today`, a date member as above | The date forms above. The date window was removed by `1a038a6`; v5's registry entry for this bridge composition was not refreshed with the dailyrecord endpoint's tools and still stated it. |
| `academy_dailyrecord_get` `/properties/view` | absent | optional, `full` or `compact` | Added by `d181232` (compact view); the same unrefreshed registry entry predates that commit. |

Tools and the members that differ:

| Tool | Members that differ (JSON pointers) | Changes |
| --- | --- | --- |
| `academy_dailyrecord_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/properties/lessonDate`, `/properties/view`, `/required` | identity, `lessonDate`, `view` |
| `academy_dailytest_history_with_headers` | `/properties/currentRequestRef`, `/properties/purpose`, `/properties/lessonDate`, `/required` | identity, date |
| `academy_student_resolve` | `/properties/lessonDate`, `/required` | identity, date |
| `academy_student_metadata_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/properties/lessonDate`, `/properties/queries/items/oneOf/0/properties/period`, `/properties/queries/items/oneOf/4/properties/period`, `/required` | identity, dates |
| `academy_student_metadata` | `/properties/currentRequestRef`, `/properties/purpose`, `/properties/scope/properties/lessonDate`, `/properties/queries/items/oneOf/0/properties/period`, `/properties/queries/items/oneOf/4/properties/period`, `/required` | identity, dates |
| `academy_textbook_section_results_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity; `bookLabel` keeps its schema and takes a loose book name |
| `academy_textbook_section_results` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity |
| `academy_clinic_paper_status_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity; `bookLabel` as above |
| `academy_item_section_trace` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity; `bookLabel` as above |
| `academy_item_answer_lookup` | `/required` | identity |
| `academy_prestudy_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity |
| `academy_prestudy_report_get` | `/properties/currentRequestRef`, `/properties/purpose`, `/required` | identity |
| `academy_review_type_questions` | `/properties/purpose`, `/required` | `purpose` default |
| `academy_cism_weekly_get` | `/properties/date` | date |
| `academy_pool_question_content` | `/properties/purpose`, `/required` | `purpose` default `teacher_requested_review`. A tool of the pool server, which neither v5 nor these two endpoints route; the test checks its v6 form. |

Every other tool of the two endpoints has the input schema it had in v5; among them the DailyRecord write tools, which keep their `context`, and `academy_cism_weekly_review` and `academy_cism_weekly_save`, whose `date` stays an exact `YYYY-MM-DD`.

## Surface

`release/surface.json` displays each changed tool with its v6 inputs: `required` is the registered `required` restricted to the members the surface displays, so `purpose` and `currentRequestRef` are displayed and not required; a displayed date member has no `pattern` or `format`; the descriptions name the date forms and the loose book name. `academy_review_type_questions` stays displayed without members. `tests/release-tools.test.ts` checks both rules. The bridge validates a call against its registry, not the surface (`src/neutral-server.ts`), so the loose inputs reach a caller only from a registry that is the `--discover` output of v6's endpoint programs.

No tool of v5 routed to these two endpoints is missing from v6.
