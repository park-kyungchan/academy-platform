# Academy native API and field reference

Classroom terms, bilingual search aliases, teacher-bound referents, counting and stage decisions have one active owner: Hindsight `academy-practical-domain-rules` in `coding-agent::shared`. Read its exact source for an ambiguous referent; clinic/DailyTest history entry is `kp-32e0bfff9ddb410886f53001cc65381b`. Do not use historical frequency counts as current routing rules. Original source evidence is retained in the task recovery copy and Git history.

This file retains native tool/field contracts only. Verify the currently registered schema and exact returned values before calls; these names are locators, not an exhaustive current capability inventory.

| Native object | Tool / field locator |
| --- | --- |
| Exact textbook selection and section results | `academy_textbook_section_results_get`; returned `bookLabel`, section titles and per-item `lectureKey` |
| Section index provenance | returned `indexBasis`, including `wrong_answer_note` and `student_paper_search` source labels |
| Clinic source observations | `academy_clinic_paper_status_get` |
| DailyTest source/history and header selection | `academy_daily_test_inventory`, `academy_dailytest_history_with_headers`, `academy_student_metadata_get`; source `paperKey`, `attemptKey`, native pagination fields |
| Printed-code lookup | `academy_printed_paper_lookup`; exact source-issued print code |
| Prestudy | `academy_prestudy_get`, `academy_prestudy_report_get`; native `teacherJudgment` and report fields |
| DailyRecord | `academy_dailyrecord_get`, native intake/review/save/readback contracts; selected field option codes require their current native binding |
| Textbook catalog / sample source | `academy_textbook_catalog`, `academy_textbook_pdf`; use current schema and returned catalog identifiers |
| CISM weekly | `academy_cism_weekly_get`, review/save contracts |

Code and dated source fixtures retain their own owners. This reference supplies no student assignments, completion inference, write authority or current backend-readiness claim.

For a genuinely missing source-qualified unit name, native `academy_textbook_units_get` reads selected TOC rows from a retained PDF SHA256 or an explicitly selected fresh catalog PDF. Its installed schema owns bounds and failure semantics; reuse routing remains in exact Hindsight `academy-practical-protocol`. It creates no local unit-memory database.
