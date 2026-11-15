# VPS workspace layout

Current: 2026-11-04, after the user-approved repository reorganization and education move. Platform and module main checkouts live in `platforms/`. Module repositories are explicitly approved; this supersedes the earlier default against per-module repositories.

| Root | One purpose | Creation rule |
|---|---|---|
| `platforms/<repo>/` | Main checkout of a platform or module repository | Platforms: agent-platform, ontology-platform, education-platform. Modules: `<platform>-<module>`, e.g. agent-vps-operations or education-dc-gang-a |
| `work/<scope>-<topic>/` | One active task with STATE.md, decisions and evidence | New task state goes here; scopes include ap, ont, edu, ws and mem |
| `work/<id>/wt/<repo>/` | Git worktree belonging to that task | Create with Git; record main checkout and branch in the task state |
| `external/<name>/` | External reference or pinned vendor source | Preserve revision and provenance |
| `scratch/<pane-id>/` | Disposable outputs and build targets | No sole copy of decisions, user data, credentials or recovery |
| `archive/INDEX.md` | Index of verified Google Drive archives | Restore by index; archives grant no execution authority |
| `private/` | Existing private custody | No layout scan, content inspection or implicit move |

Current main checkouts are `/home/palantirkc/platforms/{agent-platform,ontology-platform,education-platform,agent-vps-operations,education-dc-gang-a}`. Main checkout existence does not select a writable branch; use the assigned task and native Git state.

`projects/` retains only the out-of-program D-9 repositories bible-commentary-station, daniel-prayer-operations and park-kyungchan.github.io, plus archived academy-workbench whose local checkout supports custody worktrees. No new task state belongs in `projects/`, `operations/` or top-level `worktrees/`.

Repository renames (2026-11-04): vps-operations → agent-vps-operations; dc-gang-a → education-dc-gang-a; problem-edit → education-problem-edit; academy-observation-module → education-academy-observation. The latter two remain module repositories; obtain their checkout from the assigned task state.

Agent-harness-engineering and housing-board-module are deleted. Math-research-module, gang-a, math-education and academy-workbench are archived; only academy-workbench keeps its projects/ checkout for custody. See PATH-MAP.tsv for historical sources and current dispositions. Retired harness records do not define current runtime policy.

## Runtime and custody boundaries

An agent needs the task STATE.md and named checkout. Read repository/module instructions there; verify current source/configuration before relying on dated memory. Avoid broad repository searches and private custody inspection.

Repository, worktree, task, installed runtime and archive are distinct. A completed task can still contain a live lock, import, mount, cwd or credential reference; completion alone never justifies deletion. Preserve pinned originals and verify recovery plus live consumers before removal.

Owned runtime targets are `/opt/<platform>/<service>/releases/<version>` with a current pointer and `/var/lib/<platform>/<service>` for state. This convention does not authorize moving installed pilots. Keep vendor homes (`/root/.codex`, `.claude`, `.gemini`, `.hindsight`, `.config/muse`, `.config/herdr`) and `/docker/<stack>` identities. Host and container paths are separate.

The Academy pilot at `/root/.local/share/academy-mcp-scoped-20261030` is intentionally stale: immutable `releases/*/endpoints.json` still reference the removed `/home/palantirkc/projects/education-platform`. Academy is not in use; the user approved no compatibility symlink. Academy redesign must rebuild and repoint it before use. This layout update does not repair or activate the pilot.

Live directory names use lowercase ASCII kebab-case. Dates and agent identities belong in state/history. Branch names follow task authority. Modules may be separate repositories or directories within a platform Cargo workspace.

## Completed transitions and verification

On Nov2, agent-platform moved and all 14 selected operations/top-level-worktrees entries moved to work/. Original evidence remains at `work/mem-host-memory/selection-20261102/relocation-completion/REPORT.md`; task-owned checkout slots remain in each destination STATE.md. Nov4 reorganization moved the remaining platform/module main checkouts and repaired education Git registrations and selected CLI/wrapper consumers. Current authority and detailed evidence are in `work/ws-replan/STATE.md` and `preparation/repo-reorg-v2/` plus `preparation/autonomy-v1/` there. Earlier WP plans are historical.

Use native Git move/repair and explicit consumer readback for future relocation. Reconcile dirty/unpushed refs and owners; preserve originals. Never copy .git directories, reset another owner's branch, mass-replace archived documents or add an unowned compatibility link.

Rust `ap` is implemented in agent-platform and installed at `/usr/local/bin/ap`; `ap --help` lists its verbs. Source checks do not establish fresh-session native loading, runtime activation or business acceptance. Native settings and services retain task-specific authority.

This work/ws-layout-standard directory currently holds ordinary work records, not a Git worktree. The Nov4 versioned source for LAYOUT.md and PATH-MAP.tsv is education-platform `docs/workspace-layout/`; the published work-record copies remain the shared entry paths.
