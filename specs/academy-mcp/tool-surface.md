# The tool surface of `academy_operations`: `release/surface.json` and the Academy skills

## Purpose

The installed TypeScript Academy server, registered as the MCP server `academy_operations`,
offers 39 tools at commit 76d04632, whose compact `tools[]` JSON is 38,348 B (about 37 KiB):
four of them always answer that they are unavailable, six repeat another tool or read an old
snapshot, and many split one read into several tools. This contract fixes the surface that
replaces them: 15 tools after the existing tools are consolidated, and at most 24 (22
planned) after the 7 new tools of Waves 3 to 5 are added, with the four Academy skills naming
only tools that exist. It is the contract of the regenerated
`modules/academy-workbench/tools/academy-mcp/release/surface.json` and of
`content/skills/academy-*/SKILL.md`. It is relied on by the agents that call the tools, by
the skills, by the release install that copies `surface.json`, and by the units that build
and check the surface.

## Requirements

Size

- R1: After the consolidation of the existing tools, `tools[]` of `surface.json` holds at most 15 tools and its compact JSON is at most 18 KiB (18,432 B), which leaves at least 6 KiB for the 7 new tools under R2. (no test yet; observed by the unit that consolidates the tool surface)
- R2: After the 7 new tools of the table "New tools" are added, `tools[]` holds at most 24 tools (22 planned) and its compact JSON is at most 24 KiB (24,576 B). (no test yet; observed by each unit that adds a new tool)

Accounting

- R3: Each of the 39 tools of the table "Old to new" is accounted for as its row states: a tool marked removed is not in `surface.json`, a tool marked kept is there under its own name, and a tool mapped to a new tool and argument is not there under its old name, while the new tool takes that argument. (no test yet; observed by the unit that consolidates the tool surface)
- R4: For each mapped row, the new tool called with the row's argument and the old tool's other inputs answers, on the same recorded fixture transport, the members the old tool answered with the same values, and sends the same LMS requests. (no test yet; observed by the unit that consolidates the tool surface)
- R5: A `view`, `scope` or `include` value that the tool's table does not list is refused as an invalid input before any LMS request. (no test yet; observed by the unit that consolidates the tool surface)

Clean surface

- R6: No dead stub: no tool of `surface.json` answers every call with an unavailable status, and no description says that the tool is not available. (no test yet; observed by the unit that consolidates the tool surface)
- R7: No duplicate: the names in `tools[]` are distinct, and no two tools, for valid inputs, send the same LMS requests and answer the same members; no description says that a tool is the same as, or returns the same rows as, another. (no test yet; observed by the unit that consolidates the tool surface)
- R8: Every tool name matches `^academy_[a-z0-9_]+$`; no name has the prefix `class2_`. (no test yet; observed by the unit that consolidates the tool surface)
- R9: `tools[]` of `surface.json` equals, member for member, the tools that the server built from the same commit lists, so the file is regenerated and never edited by hand. (no test yet; observed by the unit that consolidates the tool surface)

Skills

- R10: Skill sync: every match of `(?:class2_)?academy_[a-z0-9_]+` in the four files `content/skills/academy-*/SKILL.md`, other than the server name `academy_operations`, is the name of a tool in `surface.json` (100% of the matches). (no test yet; observed by the unit that consolidates the tool surface)
- R11: A skill that names a consolidated tool for a read names the `view`, `scope` or `include` value of the read it means. (no test yet; observed by the unit that consolidates the tool surface)

Writes

- R12: Write-tool separation: every LMS request is classed read or write by its named route and operation (Interface, "Request classes"), never by its HTTP method alone. A `*_save` call sends only login requests, optionally one initial before-state read request, and, for each reviewed change or step, at most one pre-write before-state or `wireSha256` read request, one write request and one readback read request; it sends no browse query and answers no standalone read view such as `intake`, `status` or `reconcile`. (no test yet; observed by the unit that consolidates the tool surface)
- R13: No tool other than a `*_save` tool sends a request classed write; a `*_review` tool sends only requests classed read. (no test yet; observed by the unit that consolidates the tool surface)
- R14: The consolidation trims repeated prose of the descriptions but keeps, in the description of each consolidated tool, every Korean trigger keyword of the descriptions of the old tools it replaces (each Hangul term of those descriptions at commit 76d04632) and their step and parameter contracts (the numbered steps, the parameters and their allowed values). (no test yet; observed by the unit that consolidates the tool surface)

## Interface

### Compact `tools[]` JSON

The bytes of `JSON.stringify(surface.tools)` (no whitespace between tokens, UTF-8). At commit
76d04632: 39 tools, 38,348 B.

| Stage | Tools | Compact `tools[]` |
| --- | --- | --- |
| After consolidation (R1) | at most 15 | at most 18 KiB (18,432 B), at least 6 KiB below the R2 limit |
| After the 7 new tools (R2) | at most 24, 22 planned | at most 24 KiB (24,576 B) |

### Request classes

A request is read or write as `apps/academy/dc-gang-a/src/lms/lmsRouteRegistry.ts` and
`apps/academy/dc-gang-a/src/lms/endpointCandidates.ts` class its named route and operation. The
HTTP method does not decide it: `academy_dailyrecord_save` writes with a GET and reads with a
POST. A route that neither classes is write until the unit that adds it classes it.

### Old to new

| Old tool | Becomes |
| --- | --- |
| `academy_item_label_scope` | removed (unavailable stub) |
| `academy_find_item_labels` | removed (unavailable stub) |
| `academy_review_type_questions` | removed (unavailable stub) |
| `academy_cism_section` | removed (unavailable stub) |
| `academy_student_metadata` | removed (superseded by `academy_dailytest_history`) |
| `academy_student_metadata_get` | removed (superseded by `academy_dailytest_history`) |
| `academy_textbook_section_results` | removed (duplicate of `academy_textbook_section_results_get`) |
| `class2_academy_paper_headers` | removed (folded into `academy_paper_inspect`) |
| `academy_query` | removed (old snapshot) |
| `academy_source_status` | removed (old snapshot) |
| `academy_runtime_identity` | `academy_runtime_inspect(view:'identity')` |
| `academy_capabilities` | `academy_runtime_inspect(view:'capabilities')` |
| `academy_source_read_contracts` | `academy_runtime_inspect(view:'source_contracts')` |
| `academy_paper_read_contracts` | `academy_runtime_inspect(view:'paper_contracts')` |
| `academy_dailyrecord_get` | `academy_dailyrecord_get(view:'record')` |
| `academy_dailyrecord_intake` | `academy_dailyrecord_get(view:'intake')` |
| `academy_dailyrecord_execution_status` | `academy_dailyrecord_get(view:'status')` |
| `academy_dailyrecord_reconcile` | `academy_dailyrecord_get(view:'reconcile')` |
| `academy_dailyrecord_review` | kept |
| `academy_dailyrecord_save` | kept |
| `academy_student_resolve` | kept |
| `academy_daily_test_inventory` | `academy_dailytest_history(includeHeaders:false)` |
| `academy_dailytest_history_with_headers` | `academy_dailytest_history(includeHeaders:true)` |
| `academy_paper_headers` | `academy_paper_inspect(view:'headers')` |
| `academy_paper_definition_lookup` | `academy_paper_inspect(view:'definition')` |
| `academy_printed_paper_lookup` | `academy_paper_inspect(view:'attempt')` |
| `academy_textbook_section_results_get` | kept, with the direct-key fallback of [direct-key-lookup.md](direct-key-lookup.md) |
| `academy_clinic_paper_status_get` | kept |
| `academy_item_section_trace` | `academy_item_lookup(include:['section'])` |
| `academy_item_answer_lookup` | `academy_item_lookup(include:['answer'])` |
| `academy_textbook_catalog` | `academy_textbook_catalog_get(view:'catalog')` |
| `academy_textbook_toc_get` | `academy_textbook_catalog_get(view:'toc')` |
| `academy_textbook_pdf` | `academy_textbook_catalog_get(view:'pdf')` |
| `academy_prestudy_get` | `academy_prestudy_get(view:'videos')` |
| `academy_prestudy_report_get` | `academy_prestudy_get(includeReports:true, reportId)` |
| `academy_cism_weekly_get` | `academy_cism_get(scope:'week')` |
| `academy_cism_month` | `academy_cism_get(scope:'month')` |
| `academy_cism_weekly_review` | kept |
| `academy_cism_weekly_save` | kept |

Totals: 10 removed, 29 mapped or kept into 15 tools (runtime 4 to 1, DailyRecord 6 to 3,
student, DailyTest and paper 6 to 3, textbook, items and clinic 7 to 4, prestudy 2 to 1,
CISM 4 to 3).

### The 15 tools after consolidation and their arguments

| Tool | Discriminating arguments |
| --- | --- |
| `academy_runtime_inspect` | `view`: `identity`, `capabilities`, `source_contracts`, `paper_contracts` |
| `academy_dailyrecord_get` | `view`: `record`, `intake`, `status`, `reconcile` |
| `academy_dailyrecord_review` | none |
| `academy_dailyrecord_save` | none |
| `academy_student_resolve` | none |
| `academy_dailytest_history` | `includeHeaders` (boolean, default `true`), `studentName` (optional) |
| `academy_paper_inspect` | `view`: `headers`, `definition`, `attempt` |
| `academy_textbook_section_results_get` | none |
| `academy_clinic_paper_status_get` | none |
| `academy_item_lookup` | `include`: a non-empty subset of `section`, `answer` |
| `academy_textbook_catalog_get` | `view`: `catalog`, `toc`, `pdf` |
| `academy_prestudy_get` | `view` (optional): `videos`, `incomplete`, `teacher_summary`; `includeReports` (optional boolean); `reportId` (optional) |
| `academy_cism_get` | `scope`: `week`, `month` |
| `academy_cism_weekly_review` | none |
| `academy_cism_weekly_save` | none |

`academy_item_lookup(include:['section','answer'])` answers both old tools' reads in one call.

### New tools

| Tool | Contract |
| --- | --- |
| `academy_official_report_pdf_get` | [studyq-linux/core.md](../studyq-linux/core.md), `render` |
| `academy_paper_print_pdf_get` | [studyq-linux/core.md](../studyq-linux/core.md), `assemble-print` |
| `academy_lms_schedule_get` | [lms-reads.md](lms-reads.md) |
| `academy_clinic_paper_review` | [gated-writes.md](gated-writes.md) |
| `academy_clinic_paper_save` | [gated-writes.md](gated-writes.md) |
| `academy_exam_paper_review` | [gated-writes.md](gated-writes.md) |
| `academy_exam_paper_save` | [gated-writes.md](gated-writes.md) |

### Skills

The four files `content/skills/academy-counseling/SKILL.md`,
`content/skills/academy-dailyrecord-authoring/SKILL.md`,
`content/skills/academy-lesson-day/SKILL.md` and `content/skills/academy-prestudy-review/SKILL.md`.
The pattern of R10 is matched over each file's whole text; `academy_operations` is the server
name, not a tool.

### Tests

`modules/academy-workbench/tools/academy-mcp/tests/tool-surface.test.ts`, run by the step
`bun test (academy-mcp)` of `./check`; it reads `surface.json` and the four skills and drives
the tools over recorded synthetic fixture transports.

## Not included

- The members that each tool answers beyond the parity of R4; each tool's own contract
  states them.
- The text of the descriptions, beyond R6, R7 and R14.
- The Rust replacement's tools; no Rust MCP crate is part of this surface.
- Installing a release generation; `surface.json` is installed by the release install.
- The retired snapshot and non-DailyTest reads `academy_query`, `academy_source_status` and
  `academy_student_metadata_get`: no tool of the new surface answers their old snapshot or
  non-DailyTest rows.
