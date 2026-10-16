# Private student context import, 2026-10-16

This is a reviewed, real-data evidence package, not a synthetic fixture, production database migration, or official DailyRecord save. The owner explicitly approved publishing these student records to the private `park-kyungchan/dc-gang-a` repository after being told that repository readers can see them and that Git history retains them. Subsequent scope includes all student Space records and historically verified Dots evidence. Unknown coverage stays unknown; retrieval does not establish truth or final teacher completion.

## Authorization and limits

Approval source messages: `Sentinel_57bb695dc14881919c875a9ebba705ca`, `Sentinel_48a8f630b8b88191887c928a640eb8d7`, `Sentinel_afe3b44721a08191978db16967f738eb`. These identifiers locate the owner's approval; they are not authentication tokens or runtime permissions.

Only the private repository, separate review branch and unmerged PR are authorized. No merge or auto-merge, public mirror, credential export, database overwrite, source record deletion, Space deletion, grading decision, official save, notification or parent-message delivery is performed by this package. Repository privacy and exact destination must be rechecked before publication. Other repository policies are unchanged. This is not standing authorization to publish unrelated future student data.

### Subsequent repository closeout instruction

The current user subsequently instructed the Lead to review, correct and explicitly merge the repositories' outstanding PRs, then prepare clean latest-main checkouts for a new Orchestrator. This supersedes the earlier Git-merge hold for the exact reviewed private PR32/PR33 source changes. It does not authorize production application, Academy writes, student decisions, transmission, public publication or future evidence imports. Original package manifests retain their historical `mergeAllowed:false` declarations and source hashes; those documents do not grant runtime authority. Exact-head independent review, current checks, repository privacy and merge readback remain required. Automatic merge is still prohibited.

JSONL artifacts are parsed one nonempty line at a time. The existing recursive credential-field policy applies to every decoded record, including nested values and escaped keys. Malformed records fail with a line number and without echoing their content. Matching file hashes alone do not establish that an artifact passes these content checks.

## Ownership and types

`context/private-students/2026-10-16/manifest.json` declares namespace `academy.private-student-context/1.0`, data class `real_student_evidence`, file hashes, provenance kinds and explicit source coverage. Portable schemas and validator live under `harness/private-student-context/`; they must never auto-discover or read `config/`, runtime databases, unrelated `data/`, or browser state.

The package retains distinct entities: student identity; lesson occurrence and schedule; textbook edition; major/middle/minor unit; learning section; problem; assignment revision; assessment attempt; source observation; teacher statement; teacher decision; unresolved mapping. Teacher reports, imported historical observations, current server reads and assistant inference must never be silently promoted into each other. Matching a name alone does not establish a native student binding. A textbook section is not an assigned task by itself.

Book correction completion, wrong-answer clinic completion and similar clinic completion remain separate. Paper preparation, solving, grading, correction, teacher judgment and official persistence remain separate. Planned departure does not establish actual departure. Superseded homework remains in lineage rather than disappearing. Unknown means unknown, not failure, absence, nonperformance or completion.

Preserve source message/page/block identifiers, source timestamps at their actual precision, import timestamps, revision/supersession relations and artifact hashes. Do not invent retrieval timestamps, grades, missing source content or canonical IDs. Space snapshots preserve what was stored; copied assistant claims remain secondary evidence unless their underlying observation can be verified. Original private assistant operational notes are not a source to publish.

## Boundary with existing application

The original LMS remains owner of official DayRecord and source grades. Native application/PostgreSQL records retain their owner and are not mutated. Existing synthetic tests and fixtures remain unchanged. The import supplies no runtime credential, source transport qualification or automatic persistence adapter. It is not an implementation of the general operational-policy graph: student evidence must not be assigned fictitious policy identities merely to satisfy that graph's schema.

Derived prose may help review but is not independent evidence. DATA / LOGIC / ACTION / GOVERNANCE belong in separately named sections; graph containment and supersession links must be acyclic. A complete source export can still have unresolved semantic mappings.

### Existing evidence schema compatibility

The semantic reference is `park-kyungchan/gang-a` at `7b5cf56dd6f19ffe56d180b61c03d8366f5f98b1`, `packages/domain/evidence.ts`, schema `academy.evidence/2.0.1`. Preserve its user-report/imported-snapshot/direct-observation/inference distinctions and separate source clocks. The local `EvidenceCompatibility` type documents an unresolved conversion, not a new maintained copy of that canonical Zod validator. Today's `academy.class-context/1.0-draft` and Space exports are retained in their original source formats with manifest wrappers; they must not claim full evidence/2.0.1 compliance. In particular, unknown native student identity, missing observation receipt or source revision blocks conversion to direct LMS observation rather than being invented. A future admitted native import must validate with the actual owner schema and current target-bound mapping.

## Validation and review

Run `bun run harness/private-student-context/validate.ts` against the explicit package directory. For dependency-free inspection only, Node 24 TypeScript stripping can run the same validator; that is not the repository's Bun/native acceptance. Run its focused synthetic test file and inspect every publication path and full diff for credentials, unrelated private information and accidental source disclosure. Hash validation and pattern scanning are safeguards, not proof that all sensitive material is appropriate.

Publish only reviewed explicit paths. Bind validation/review results to exact bytes and later the remote branch SHA. Confirm the PR's actual head and applicable CI. This source-data review is not teacher acceptance or application deployment. Never claim the full repository check passed if only these portable checks ran.
