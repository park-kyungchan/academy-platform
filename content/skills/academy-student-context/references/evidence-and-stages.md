# Evidence and learning-state distinctions

## Establish evidence before writing

- Treat newly supplied user facts as evidence without making an unnecessary source call. When history or current state is needed, use available academy-owned context entrypoints and the existing custom MCP/read interfaces. Discover relevant tools and repository memory before asking the user for already-recorded facts. Keep source, code, semantic, data and native ownership unchanged.
- Narrow retrieval by business date → class/cohort → student → relevant books/activities. Consume every required page; a first page or a selected facet is not complete history. Read separate dated lesson overlays when they contain observations absent from history facets. Distinguish discovery metadata from source-body ingestion.
- Track coverage and freshness accurately: event date, recorded/retrieved date, source revision and reporting date differ. Saved/hosted/local sources may disagree. “Latest available snapshot” is not “refreshed today.” If a source is unavailable, provide the supported portion and a precise gap rather than inventing a current state.
- Keep concise source references close to the claims they support, or group them by subsection. Mention a common freshness limit once; attach local uncertainty only where it affects a decision. Do not overwhelm the teacher with internal IDs or transport details.

## Reconcile identities and updates

Keep the following dimensions distinct; an unknown dimension stays unknown:

`student → book identity/edition → unit → activity kind → occurrence/attempt → material/item → stage → observation/decision`

- Preserve school level, term, edition and volume when known. Identical unit numbers across books are not the same unit. A title/code/hash crosswalk is only a candidate until supported; do not join mismatched book namespaces by unit number alone. Do not invent a missing volume.
- Distinguish regular class, makeup lesson and assessment occurrences. Work completed at a makeup lesson stays on that date, not duplicated as a later class achievement. A planned end time is not an actual finish time.
- Distinguish textbook work, textbook-internal corrections, concept blank tests, DT, unit/whole-book assessments, separate original-error clinics and similar-problem clinics. Preserve the academy's actual activity labels rather than forcing every event into these examples.
- Preserve assessment original, print, distribution, attempt, score, corrections, repeat-error check, teacher final judgment, and app entry as separate stages. Report only the stages observed. A printed paper is not an assigned or completed paper; a score or app label alone is not teacher-final completion.
- Apply the latest applicable user/teacher correction to its exact book, activity and occurrence. Preserve the earlier state as dated history when it explains the change. A scoped teacher-final completion must not be reopened merely because an older snapshot, raw low score or stale checklist remains. A genuinely later distinct assignment may still be pending.
- If two apparently conflicting observations cannot be linked to the same occurrence, keep both with their dates and explain the unresolved link. “Latest timestamp wins” is insufficient when records concern different stages or materials.
- Preserve the teacher's interpretation as an attributed judgment. Do not convert “teacher judges ready to retry” into a passed assessment or a generalized ability diagnosis.

## Status, homework and counting rules

- `미확인` means no adequate evidence; `미완료/미제출` needs evidence of an actual assignment or obligation that remains open. Catalog rows, unobserved activities, verification slots and future units are not pending homework.
- Describe intermediate states precisely: `배정 확인`, `출력 확인`, `풀이 완료`, `채점 완료`, `오답 수정 완료`, `교사 최종완료`, `앱 입력 완료`. Do not infer the unobserved stages from one confirmed stage.
- Preserve cancellation/replacement history, then clearly state the final active homework. Remove cancelled or replaced work from the current pending count. A teaching-plan end date is not automatically every assignment's deadline.
- Keep scope and conditions explicit: “if time remains,” “after the score is confirmed,” and “after teacher item selection” are conditional next actions, not already assigned homework.
- Keep raw attempt counts, unique item labels, paper counts, assignment counts and DT rounds separate. State the counting unit when giving a total. A non-100 score, a zero score, duplicate route observation, or a metadata object is not evidence of a currently unfinished paper.
- Distinguish rubric/target from actual performance. Use only the applicable student's assessment rule; do not globalize a threshold, difficulty tier, retest period or DT-homework exception from an example.
- Keep historical scores labeled with their activity order. Do not imply they determine present correction status. Preserve exact threshold inclusivity when supplied; leave an ambiguous boundary unresolved rather than guessing.

## Finish the requested report

Identify active assigned work separately from proposed teaching actions and questions. For preparation requests, give actionable next steps and preserve dependencies without treating proposed work as assigned. For updates, state what changed and retain still-relevant prior completion.

Before delivery, check:

- Correct student, class, business date, book and occurrence for every decision.
- No unknown catalog rows counted as overdue; no cancelled task left pending.
- No completed teacher-scoped work reopened or broadened to other activities.
- Scores/counts/deadlines and plan dates retain their original meaning.
- Coverage/freshness limits and unresolved identity links remain visible.

An optional improvement is a short `오늘 바로 볼 요약` before the numbered students, with up to three compact groups: `오늘 할 일 / 새로 확정 / 확인 필요`. Offer or use it when requested or when it materially helps a long report; keep the user's detailed hierarchy underneath. Do not replace the default format with this enhancement.

Formatting a report grants no authority to save student records, send messages, print, grade, deploy or change owners. Follow the user's actual authorized scope. Keep real student originals out of this reusable skill; use existing private custody for any requested artifacts.
