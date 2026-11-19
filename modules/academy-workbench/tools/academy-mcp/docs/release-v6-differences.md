# Release v6 against v5: saved and dailyrecord endpoints

Release v6 builds its saved endpoint from `src/server.ts` and its dailyrecord endpoint from `src/dailyrecord-server.ts` on `main` (`bun run build:endpoints`). Release v5's tools are recorded in `tests/fixtures/release-v5-tools.json` and its surface in `release/surface.json`. `tests/release-tools.test.ts` compares each v5 tool of these two endpoints with what its program registers now and fails on any difference this page does not list. The paper endpoints' tools are compared separately, with the paper server.

A tool that a program registers and v5 does not route to that program is not a difference: for example `academy_endpoint_identity`, or `academy_textbook_units_get`, which is registered and not on the surface.

## Admission gates (saved endpoint only)

Commit `1a038a6` removed `src/owner-admission.ts` and `src/metadata-admission.ts`: no student list, date window or metadata selector list gates a read. v5's saved bundle was built before that commit and holds both gates; v6's saved bundle holds neither. A launch that still passes `--dailyrecord-admission` or `--metadata-admission` (a path and a SHA-256) is accepted and the arguments gate nothing (`src/dailyrecord-tools.ts`). v5's dailyrecord bundle was built after `1a038a6` and holds neither gate, so the dailyrecord endpoint has no such difference.

## Input schemas

| Tool and JSON pointer | v5 | v6 | Reason |
| --- | --- | --- | --- |
| `academy_dailyrecord_get` `/properties/view` | absent | optional, `full` or `compact` | Added by `d181232` (compact view). v5's registry entry for this bridge composition was not refreshed with the dailyrecord endpoint's tools and predates that commit. |
| `academy_dailyrecord_get` `/properties/lessonDate/description` | an explicit date must fall inside the owner-admitted range | any `YYYY-MM-DD` date | The date window was removed by `1a038a6`; the same unrefreshed registry entry still states it. |

`academy_dailyrecord_get` and `academy_dailytest_history_with_headers` are compositions of the bridge (`src/neutral-server.ts`), so their v6 schemas are the bridge's `dailyRecordGetSchema` and `dailyTestHeaderRouteSchema` as `z.toJSONSchema(…, {io: 'input'})`. Every other tool of the two endpoints, and `academy_dailytest_history_with_headers`, has the input schema it had in v5.

No tool of v5 routed to these two endpoints is missing from v6.
