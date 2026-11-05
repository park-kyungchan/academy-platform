---
name: academy-student-context
description: For Korean student reports, use supplied evidence while academy_operations is DISABLED until full Rust cutover; preserve identities and teacher decisions.
---

# Academy student context

`academy_operations` is **DISABLED in all CLIs until the full-surface Rust cutover**. Make no Academy calls and use no direct fallback: no TS bundle, launcher, shell client, browser, HTTP probe or replacement MCP client. Apply [academy-runtime-routing](../academy-runtime-routing/SKILL.md) and its [cutover status](../academy-runtime-routing/references/cutover-status.md). Requested formatting of user-supplied or explicitly authorized preserved evidence remains allowed; label its source, date, scope, coverage and freshness.

Answer the requested scope. For a short question or one correction, give a concise direct answer without a mandatory title, numbering, tutorial or staged wait. For a comprehensive lesson or student report, use [full-report.md](references/full-report.md); keep the requested students, books and material history rather than replacing them with a shallow checklist. The synthetic [report-example.md](references/report-example.md) is a formatting example, never real student evidence.

Resolve the business date from academy context; host UTC is not automatically the lesson date. Reuse facts the user just supplied. Use supplied or explicitly authorized preserved context only for missing history, scoped to the requested date, students, books and activities; defer any fresh operational read during the hold. A saved snapshot is not a fresh service read; a catalog row does not prove activity or unfinished homework.

Keep this identity chain distinct:

`student → book/edition/volume → unit → activity → occurrence/attempt → material/item → stage → observation/teacher decision`

Preserve solving, grading, correction, teacher-final judgment and app entry as separate stages. Printing is separate from assignment and completion. A grouped score is not a score for each paper. Latest applicable teacher corrections supersede the same occurrence; preserve dated earlier states and distinct later assignments. Completed teacher-scoped work is not reopened by stale app wording. Keep real deadlines separate from lesson dates and plan end dates.

For conflicting sources, pagination, occurrence matching, exact counts or a full history, read [evidence-and-stages.md](references/evidence-and-stages.md). Report supported facts and the precise gap; never fill a missing identity or result. If the task forbids further interviews, record unresolved choices in its deferred-decision file and continue independent work.

Book version metadata such as `v3(1P6A)` may appear in domain-owned internal evidence and conversation with the user. Operational book/student/attempt/lesson facts, including edition metadata, never go into Hindsight. Only separately authorized sanitized terminology definitions or student-free engineering lessons may become durable context; they must contain no operational payload. Exclude implementation/version markers only from parent-facing DailyRecord text, using the confirmed canonical book title (for example `가우스 5-1-2`). Keep full internal identity for matching; never erase editions from source records or guess a title from an unknown book.

Current task state controls the hold; installed or cached tools do not release it. After full cutover, use current tool schemas and the exact authorized effect scope. Formatting a report grants no save, message, print, grading or deployment authority. Keep private student originals outside reusable skills.

Use [explain-precisely](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/explain-precisely/SKILL.md) for technical explanations. Engineering reports follow [strategy-dialogue](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/strategy-dialogue/SKILL.md)'s KPI-first convention and [lessons-loop](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/lessons-loop/SKILL.md) for authorized student-free closure candidates. Parent-facing reports and DailyRecords retain their domain format without a mandatory engineering KPI preamble. Follow [Context Governance](https://github.com/park-kyungchan/agent-platform/blob/main/content/runtime-references/CONTEXT-GOVERNANCE.md) for source and memory ownership.

For missing teacher-owned definitions, reuse confirmed answers first. If an actual interview task is assigned, use its exact owning-domain interview skill path from TASK when available; do not assume the deferred skill exists, create it here, or infer an answer. If interviews are held, record the precise unknown in the task's permitted local output.
