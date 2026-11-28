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

- R1: The 15 tools of the consolidation of the existing tools (the table "The 15 tools after consolidation") are in `tools[]` of `surface.json`, and their compact JSON, the 15 tools alone in their surface order, is at most 18 KiB (18,432 B), which leaves at least 5,568 B below the 24,000 B limit of R2 for the 7 new tools. R1 bounds these 15 tools only; R2 bounds the whole surface as the new tools are added. Tests: `tool_surface_r1_r8_fifteen_tools`.
- R2: After the 7 new tools of the table "New tools" are added, `tools[]` holds at most 24 tools (22 planned) and its compact JSON is at most 24,000 B. Until then the running cap holds: while `n` of the 7 new tools are not yet in `surface.json`, the compact JSON is at most 24,000 B − 795 B × `n` and `tools[]` holds at most 24 − `n` tools. Tests: `tool_surface_r2_running_cap`.

Accounting

- R3: Each of the 39 tools of the table "Old to new" is accounted for as its row states: a tool marked removed is not in `surface.json`, a tool marked kept is there under its own name, and a tool mapped to a new tool and argument is not there under its old name, while the new tool takes that argument. Tests: `tool_surface_r3_accounting`.
- R4: For each mapped row, the new tool called with the row's argument and the old tool's other inputs answers, on the same recorded fixture transport, the members the old tool answered with the same values, and sends the same LMS requests. An old input whose name the new tool gives to its discriminating argument is taken under the name the table "The 15 tools after consolidation" gives it: `view` of `academy_dailyrecord_get` (`full`, `compact`) as `detail`, `view` of `academy_capabilities` as `capabilityView`, `view` of `academy_paper_definition_lookup` (`definition`, `header`) as `definitionView`, `prestudyKey` of `academy_prestudy_report_get` as `reportId`, and `displayName` of `academy_dailytest_history_with_headers` as `studentName` (`displayName` is still taken). Tests: `tool_surface_r4_parity`, `tool_surface_r4_bridge_rows`.
- R5: A value of a discriminating argument that the tool's row of the table "The 15 tools after consolidation" does not list is refused as an invalid input before any LMS request. R5 checks only the discriminating arguments that the row lists for that tool: the object-valued `scope` that `academy_dailyrecord_get` takes in the views `intake` and `status` is the DailyRecord scope, not a discriminating argument, and is checked by that view's own input. Tests: `tool_surface_r5_selector_refused`, `tool_surface_r5_bridge_refuses`.

Clean surface

- R6: No dead stub: no tool of `surface.json` answers every call with an unavailable status, and no description says that the tool is not available. Tests: `tool_surface_r6_r7_r14_descriptions`.
- R7: No duplicate: the names in `tools[]` are distinct, and no two tools, for valid inputs, send the same LMS requests and answer the same members; no description says that a tool is the same as, or returns the same rows as, another. Tests: `tool_surface_r6_r7_r14_descriptions`.
- R8: Every tool name matches `^academy_[a-z0-9_]+$`; no name has the prefix `class2_`. Tests: `tool_surface_r1_r8_fifteen_tools`.
- R9: `tools[]` of `surface.json` equals, member for member, the tools that the server built from the same commit lists, so the file is regenerated and never edited by hand. Tests: `tool_surface_r9_built_surface`, `tool_surface_r9_pinned_list`.

Skills

- R10: Skill sync: every match of `(?:class2_)?academy_[a-z0-9_]+` in the four files `content/skills/academy-*/SKILL.md`, other than the server name `academy_operations`, is the name of a tool in `surface.json` (100% of the matches). Tests: `tool_surface_r10_skill_sync`.
- R11: A skill that names a consolidated tool for a read names the `view`, `scope` or `include` value of the read it means. Tests: `tool_surface_r11_skill_selectors`.

Writes

- R12: Write-tool separation: every LMS request is classed read or write by its named route and operation (Interface, "Request classes"), never by its HTTP method alone. A `*_save` call sends only login requests, optionally one initial before-state read request, and, for each reviewed change or step, at most one pre-write before-state or `wireSha256` read request, one write request and one readback read request; it sends no browse query and answers no standalone read view such as `intake`, `status` or `reconcile`. (no test yet; observed by the unit that consolidates the tool surface)
- R13: No tool other than a `*_save` tool sends a request classed write; a `*_review` tool sends only requests classed read. Tests: `tool_surface_r13_no_write_route`, `tool_surface_i1_request_classes`, `tool_surface_i1_resolution_write_forbidden`.
- R14: The consolidation trims repeated prose of the descriptions but keeps, in the description of each consolidated tool, every Korean trigger keyword of the descriptions of the old tools it replaces (each Hangul term of those descriptions at commit 76d04632) and their step and parameter contracts (the numbered steps, the parameters and their allowed values). Tests: `tool_surface_r6_r7_r14_descriptions`.

## Interface

### Compact `tools[]` JSON

The bytes of `JSON.stringify(surface.tools)` (no whitespace between tokens, UTF-8). At commit
76d04632: 39 tools, 38,348 B.

| Stage | Tools | Compact `tools[]` |
| --- | --- | --- |
| The 15 tools of the consolidation (R1) | 15 | at most 18 KiB (18,432 B) for those 15 alone, at least 5,568 B below the R2 limit |
| While `n` of the 7 new tools remain to be added (R2) | at most 24 − `n` | at most 24,000 B − 795 B × `n` |
| After the 7 new tools (R2) | at most 24, 22 planned | at most 24,000 B |

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
| `academy_runtime_inspect` | `view`: `identity`, `capabilities`, `source_contracts`, `paper_contracts`; the old `view` of `academy_capabilities` (`hierarchy`, `operations`, `source_contract`, `workflow_rules`) is `capabilityView` |
| `academy_dailyrecord_get` | `view` (default `record`): `record`, `intake`, `status`, `reconcile`; the old `view` of `academy_dailyrecord_get` (`full`, `compact`) is `detail` |
| `academy_dailyrecord_review` | none |
| `academy_dailyrecord_save` | none |
| `academy_student_resolve` | none |
| `academy_dailytest_history` | `includeHeaders` (boolean, default `true`); `studentName` (or `displayName`) and `studentId`: with `includeHeaders: true` a name is required; with `includeHeaders: false`, `studentId`, or a name that the call resolves to the `studentId` |
| `academy_paper_inspect` | `view`: `headers`, `definition`, `attempt`; the old `view` of `academy_paper_definition_lookup` (`definition`, `header`) is `definitionView` |
| `academy_textbook_section_results_get` | none |
| `academy_clinic_paper_status_get` | none |
| `academy_item_lookup` | `include`: a non-empty subset of `section`, `answer` |
| `academy_textbook_catalog_get` | `view`: `catalog`, `toc`, `pdf` |
| `academy_prestudy_get` | `view` (optional, default `videos`): `videos`, `incomplete`, `teacher_summary`; `includeReports` (optional boolean); `reportId` (optional; together with `includeReports: true` only, as [lms-reads.md](lms-reads.md) states) |
| `academy_cism_get` | `scope`: `week`, `month` |
| `academy_cism_weekly_review` | none |
| `academy_cism_weekly_save` | none |

`academy_item_lookup(include:['section','answer'])` answers both old tools' reads in one call.

### New tools

| Tool | Contract |
| --- | --- |
| `academy_official_report_pdf_get` | [pc-executor.md](pc-executor.md): `StudyQReport.exe` on the user's PC, the Academy server's answer passed verbatim |
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
the tools through the bridge over recorded synthetic fixture endpoints
(`tests/tool-surface-fixture.ts`). Its I1 tests class each LMS request by its named route and
operation with the `write_forbidden` helper of `tests/network-deny.ts`. `surface.json` is
built by `bun scripts/build-surface.ts` from `src/consolidated-tools.ts`.

## Not included

- The members that each tool answers beyond the parity of R4; each tool's own contract
  states them.
- The text of the descriptions, beyond R6, R7 and R14.
- The Rust replacement's tools; no Rust MCP crate is part of this surface.
- Installing a release generation; `surface.json` is installed by the release install.
- The retired snapshot and non-DailyTest reads `academy_query`, `academy_source_status` and
  `academy_student_metadata_get`: no tool of the new surface answers their old snapshot or
  non-DailyTest rows.
