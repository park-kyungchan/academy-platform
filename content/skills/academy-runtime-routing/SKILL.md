---
name: academy-runtime-routing
description: For Academy operations, academy_operations is DISABLED in all CLIs until full-surface Rust cutover; use supplied evidence only, with no direct fallback.
---

# Academy runtime routing

`academy_operations` is **DISABLED in all CLIs until the full-surface Rust cutover**. Current task holds apply even if an old session still exposes tools. Make no live Academy calls. There is no direct fallback: do not run a TS bundle, launcher or shell client, use a browser, probe HTTP endpoints, or construct another MCP client. Keep the retained bundle, units, sockets and host configuration untouched.

During this hold, use only user-supplied or explicitly authorized preserved evidence, labeled with source, date, scope, coverage and freshness. A saved snapshot is not current service state. Stop work that requires a fresh operational read and report the exact gap; do not probe to diagnose it. Formatting supplied evidence remains allowed within the task's scope.

Read [cutover-status.md](references/cutover-status.md) for historical deployment facts, the current STATE locator and the complete future cutover gate. The following operational semantics apply only after that gate and explicit current task authority release the route; they do not authorize calls during the hold.

Choose current native reads when current records matter. Preserved-evidence tools expose only their stated coverage/freshness. Exact roster/book identities and complete required pages matter; opaque evidence IDs and native account IDs are different. A missing catalog entry does not prove that no learning activity occurred.

Read the installed tool's current intake/review/save schema and required context rather than copying an old observation-ticket lifetime or assuming the bridge is read-only. A review is separate from a save. A current save instruction authorizes only its exact scope, with original values preserved and native readback afterward. Parent transmission, printing and paper generation are separate effects. Follow the current user's task-specific holds and authorized effects; historical handoffs do not impose a permanent read-only scope.

Inspect structured domain status and safe failure metadata as well as MCP isError. A dated student_name_not_found or partial_coverage result does not establish attendance or a service outage. For an upstream read failure, stop dependent work, locate and fix the stated boundary, then verify actual readback before proceeding. After an uncertain mutation, reconcile the native target before any retry. Keep the existing shared account lock and release it before waiting for user input. A closed work folder may still supply that lock; inspect live consumers before removal.

For teacher-facing identity/stage conventions, use the exact owning-task `academy-student-context/SKILL.md` path supplied by TASK. Do not assume a global skill alias or a public dependency on a private teaching repository. Short answers stay short.

Operational student, book, attempt and lesson facts never go into Hindsight. Keep credentials and student payloads out of settings, reusable fixtures and public artifacts. Follow [Context Governance](https://github.com/park-kyungchan/agent-platform/blob/main/content/runtime-references/CONTEXT-GOVERNANCE.md); only authorized, sanitized student-free engineering lessons may reach [lessons-loop](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/lessons-loop/SKILL.md). Engineering explanations use [explain-precisely](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/explain-precisely/SKILL.md) and engineering reports use [strategy-dialogue](https://github.com/park-kyungchan/agent-platform/blob/main/content/skills/strategy-dialogue/SKILL.md)'s KPI-first convention. Parent-facing outputs retain their domain format.
