# Current U3 source status — 2026-11-05

This notice supersedes the pre-#78 implementation and maintenance guidance in this metadata entrypoint. Earlier catalog repository roles, records, findings, workflow owner names and repository-plan observations retain their original dated meaning; they do not establish an implementation available at head.

After education-platform PR #78, `apps/academy/gang-a/` contains only the retained dependency closure: 29 excluded files required by retained dc-gang-a source and tests, including their manifest, lock and static resources. It is not a maintained or complete gang-a implementation. No current implementation claim or instruction to implement against this partial directory remains in force.

The following seven U3 directories are archived, unavailable as complete implementations at head, and have no current maintenance claim:

| Directory | Current checkout disposition |
| --- | --- |
| `apps/academy/gang-a/` | Only the 29-file dependency closure remains. |
| `apps/academy/month-end-workbench/` | Removed; archived source only. |
| `apps/academy/month-end-web/` | Removed; archived source only. |
| `modules/math-research-module/` | Removed; archived research and chronology only. |
| `packages/curriculum/` | Removed; archived export and missing-original observations only. |
| `packages/mathematics-core/` | Removed; archived source only. |
| `apps/mathcrew/` | Removed; archived source only. |

The full pre-deletion implementation is recoverable from tag `archive/education-platform-pre-delw1-C-20261105`, commit `a81006644762446839970ff6d6e0069efb17f1d4` (`a81006`), in education-platform Git history. The seven verified Drive archives are under `gdrive:vps-archive/2026-11/education-platform-*__ec0d220__20261105.tar.zst`; the program archive `INDEX.md` and the original wave's `C-drive-recovery.json` record their verification. This notice reuses that recovery evidence; it does not claim a fresh Drive readback.

In `repository-plan.json`, each `currentSourceStatus` supersedes that row's old `observedRole` / `phaseOneRole` implementation guidance. The education-platform, gang-a, math-education and math-research-module rows describe pre-deletion observations. In the catalog, `CTX-U3-RETIREMENT-20261105` supersedes `CTX-ED-MAINTENANCE` without altering its stored bytes. The selector returns only revision metadata for superseded records by default; `--history` returns their prose with effective historical status. It follows explicit scoped current successors, not dates or ID ordering.

The old `ACX-VERIFY-001` plan/catalog finding about the month-end aggregate route, `ACX-RP-03` research-owner proposal, and `ACX-DECISION-001` instruction to use a current month-end entrypoint are dated pre-deletion guidance. They do not direct a caller to execute or improve these removed packages. The workflow `OwnerId` labels are design identities, not proof of an installed or maintained implementation. Retained MCP documents keep their bytes; the parent module README separately qualifies their month-end locators.

The maintained platform implementation is the Rust Cargo workspace described in the platform README. It is not yet qualified for Academy native activation. Before any implementation work, use the current task and qualify a supported owner and exact source revision. Archive consumption remains `locator_only_requalify_before_use`; archived sources and retained dependencies grant no operational authority or custody transfer.
