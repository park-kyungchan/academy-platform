# S08 retained pure candidate V3

Source-only candidate on `rust/s08-pure-semantics-20261102`, based on accepted leaf `b8f1206b76f6a72b2381e134162059d6d53a8e90`. Original source is `cc6e81f1be2fcb56f011041e3d606e21edf76da3`; independent reference foundation is `ff12a74eeb26400e00d3a6676332aaab1a098cb4`. Active expectations remain `migration/pure-semantics/expected-v2.json`, SHA256 `9073faa3c80aced0bcc01f77ed99e4d1984b218134bc1d11910e15933d202578`. Invalid v1 remains historical. Current user/STATE/adopted ZIP govern, including the S08/EDU-03/04/05/06/17/18/JUDGE-01 boundaries. Lead owns overall initiative, integration, builds and independent Wonder review.

No compiler was authorized or invoked by the author. Static formatting, locked metadata, Cargo registry resolution/fetch and focused JavaScript checks are not Rust compilation or semantic parity. All 202 selected operations now have candidate paths, but no native 202-case result is claimed. See `migration/pure-semantics/candidate-v3/checks.json` for actual final static command records and `preflight-01/report.json` for input-lowering preparation only. Seven Rust source-literal tests are supplied but unexecuted.

## API and source ownership

| Rust API | Original unchanged owner | Boundary |
| --- | --- | --- |
| `leaf::{journal_wire_width,cism_week,cism_month_dates}` | lessonJournalTemplates.ts / cismWeeklyProtocol.ts | Existing accepted leaf.rs bytes and old wire requests preserved |
| `js::{Js,Text,encode,digest,quote}` with `Family::{Learning,Admission,Cism}` | learning/model.ts, dailyrecord-save-admission.ts, cismWeeklyProtocol.ts | Three separate encoder branches, UTF-16 strings/keys, insertion/index enumeration, binary64, explicit descriptors |
| `knowledge::{calendar_date,assert_knowledge,known,unknown,immutable_copy}` | learning/model.ts | Learning date years 0000–9999, separate from CISM; cloned JSON order and +0; owned Rust values do not assert JS runtime freeze identity |
| `templates::{render,constants}` | lessonJournalTemplates.ts, parentInternalLanguage.ts, CISM constants | Exact text, revision, refs, blocker ordering; rendering never approves |
| `decision::review` | DailyRecordService intake/review, dailyrecord-context.ts, read-failure.ts | Synthetic recorded composition only, mandatory `TestPorts` before intake/hash; no durable workflow, production policy or authentication default |
| `decision::fake_http_action` | applyJournalHttpAction with baseline.ts local fake | In-memory target only; correct effectRecordKey and events; no actual HTTP/locks/approval authority |
| `semantic::execute`, `wire::{Wire,Node,TestPorts}` | New candidate adapter | Explicit V3 input admission; separate nonzero adapter channel |

`Js::Str(Vec<u16>)` and object keys retain lone surrogates. Object members are an insertion-ordered vector; encoder key sorting compares u16 sequences. `code_units()` maps unpaired surrogate code units to a nonmatching placeholder only for source regexp/ASCII-identifier predicates; it is never a renderer, hash input or normalization path. Text concatenation, matching of exact fact fragments and quoting operate on original units. Parent regexp folding changes ASCII A–Z only. Dependencies are exact `ryu-js=1.0.2` (ECMAScript numeric formatting), `sha2=0.10.9`, `regex=1.12.2`; all previous lock versions are retained. Registry Cargo.lock checksums pin new transitives. Official API references: https://docs.rs/ryu-js/1.0.2/ryu_js/ , https://docs.rs/sha2/0.10.9/sha2/ , https://docs.rs/regex/1.12.2/regex/ .

`constants.json`, `workflow-context.json`, and `hold-actions.json` are direct source-owned constant data, not copied expected frames. Constants came from the actual unchanged module exports; workflow context and action guidance were evaluated from their exact source literals. `candidate-v3/source-data.mjs` verifies them against original source and validates source pins. Reference outcomes are read only by the external comparator driver, never by Rust.

The source Zod schema's declared property order is preserved before review hashing, including optional explicitValue/origin defaults. Source diagnostic coordinates refer to the pinned TypeScript throw sites; error hashes are computed from the original exception name and message. Missing observation fields construct Zod4.4.3 invalid_type diagnostics in source field order. Other schema-invalid context/proposal/observation forms require the explicitly proposed adapter admission; they are not returned as invented source errors.

## Protocol V3

The old `width`, `week`, `month` requests, refusal frames, maximum stdin size 1048576, stderr, statuses and duplicate-member rejection remain unchanged. New requests are:

```json
{"op":"semantic_v3","wire":{"root":0,"nodes":[{"kind":"object","entries":[[[111,112],1]]},{"kind":"string","units":[99,111,110,115,116,97,110,116,115]}]}}
```

That example is an explicit flat encoding of `{op:'constants'}`. `wire.mjs` is the executable lowering specification. It never computes a semantic output. Integer references index nodes. Node kinds are `null`, `undefined`, `hole`, `bool{value}`, `number{bits}` (16 hexadecimal binary64 bits), `string{units}`, `array{items}`, `object{entries}`, `null_prototype{entries}`, `symbol_keys{entries}`, `accessor{entries}`, `decorated{items,entries}`, `sparse{length}`, `date{iso}`, `bigint`, `function`, `symbol`, `cycle`. Entries are `[UTF16_key,node_index]` pairs. No JS code executes. `symbol_keys` and `accessor` describe the reference constructors: extra own symbol and constant-return own getter respectively; getters emit `accessor_read` on each original-equivalent read. `cycle` represents the selected self-referential object constructor. These are explicit source descriptor representations, not claims of a general JS object engine.

The full request contains the original `op` and arguments in its decoded root object. New semantic operations are `template`, `constants`, `json`, `knowledge`, `known`, `unknown`, `calendar_date`, `review`, `http_action`. Descriptor tags are lowered only in the arguments where baseline.ts invokes materialize; business strings/objects are not interpreted as tags. Every decoded source return/throw uses the frozen frame schema with exact event order. Actual child status remains independent. One compact line is emitted using the JS-compatible stringifier.

`review` requires outer `ports:{uuid,now}`. The comparison driver explicitly supplies the source baseline UUID and UTC clock before any original-equivalent intake, review body or digest. The Rust library has no deterministic production default. Synthetic learning and parent policy pins/callbacks are named as such. They prove composition only, never actual namespace, catalog, parent-language, entitlement, authentication, durable approval or policy authority.

Malformed envelopes/nodes, duplicate JSON fields, invalid graph references/cycles, duplicate ordinary member keys, >65536 nodes/decoded visits, decode depth>96, unsupported number-bit text or code units and stdin overflow are adapter failures: status64, no stdout, `s08_protocol_input_rejected`. Valid but outside the proposed semantic admission returns status64, no stdout, `s08_semantic_admission_required`. Source calendar/knowledge/encoder/template/action errors return status0 with the original source throw shape. Output failure remains status74; encoding an unadmitted output returns64 `s08_semantic_output_rejected`. No control flags or external transport/store APIs exist in Rust.

## Exact exit evidence and later obligations

| Vector | Candidate boundary | Evidence still required / later owner |
| --- | --- | --- |
| EDU-03 | Known/unknown, missing/null/blank/zero/not_performed; literal source option -1/0; no completion derivation | Lead native 202 comparison + Wonder; broader catalog/learning authority is unported, not silently delegated or completed |
| EDU-04 | Previous homework uses independent earlier target; fake action verifies physical effect record before approval/write | Lead native comparison; HTML/occurrence/parser joins S09; writable admission, native lock/journal S10; thin real-port integration S11 |
| EDU-05 | Field-local blockers, insertion dedup, independent valid patch retained under held review | Lead native comparison; real policy binding and durable execution disposition S10/S11; a prepared patch is no save permission |
| EDU-06 | Nonempty set replacement hold, exact noop, explicit replace/clear and no_addition, exact teacher fact lines | Lead native comparison; actual user approval and source before/reconciliation S10 |
| EDU-17 | Three encoders, exact units/escapes/order/numbers/bytes/SHA; complete template text/refs | Lead native comparison; draft admission below requires independent decision; no broad JSON or JS runtime identity parity claim |
| EDU-18 | Accepted CISM week/month leaf unchanged; separate learning date parser | Rerun 54 source cases + 15 original admission probes with new binary; CISM DOM/source handler qualification S09 |
| JUDGE-01 | New driver calls unchanged qualified comparator on actual children; 202 inputs retained without normalization | Lead actual 202 native results, existing 17-control foundation remains frozen; new 13 adapter probes require actual binary and independent review |

Not ported, and not claimed discharged by 202 source-only outputs: full learning catalog/context/record/provenance/export closure, immutableCopy runtime property/freeze identities, general Zod schema exception closure, liveSaveRequestEvidence/save-admission schema/createDailyRecordSaveAdmission tuple and one-time consumption, CISM parser/source selector/write-parameter encoding. The original DESIGN.md names these precisely. Pure uncharacterized exports need a new source-backed corpus/admission decision before their own S08 exit; parser/source reading belongs to S09, durable admission/old journals/participating kernel locks/physical-field uncertainty to S10, and thin MCP/complete host-Hermes release/recovery to S11. Those dependency joins remain held; this candidate starts none of them. S05 history/budget/HTML/account/root gaps remain. Actual P8 verdict and P9 switches/deletion/stability/recovery conditions remain separate.

## Compact handoff

1. Goal: Lead builds/reviews this exact local candidate against the unchanged independently reviewed 202-case source reference; do not treat static author checks as acceptance.
2. Checkout/common Git: `/home/palantirkc/work/ws-replan/wt/education-platform`, `/home/palantirkc/projects/education-platform/.git`; branch above; base b8f1206. Changes are confined to this crate, pinned workspace manifests/lock and new candidate-v3. `leaf.rs`, S05 and every existing S08 artifact remain byte-identical.
3. Allowed author effects were local edits, static formatting/metadata/fetch and focused source/Node checks, then a local commit. No compiler, model children, live/private/HTTP request, lock/journal, save/print/send, global/native config, push/merge/deploy or task STATE/ORCHESTRATION write occurred. No running children/builds remain.
4. Expected output: local committed source candidate; raw static statuses and prepare-only lowering in candidate-v3; exact Lead commands in `candidate-v3/LEAD-COMMANDS.sh`; unresolved decision in `ADMISSION-PROPOSAL-v3.md`. Seven Rust tests await Lead compilation. An initial fmt parse failure was corrected before final static checks; it is not claimed a pass.
5. Current task recovery authority remains `/home/palantirkc/work/ws-replan/STATE.md`, ORCHESTRATION.md/CURRENT-PLAN.md/adopted ZIP. Next action: Lead obtains sole build slot, compiles exact revision, runs all202 + backward54 + original15 + proposed13, preserves raw failures and asks independent Wonder to challenge both semantics and proposed admission. Lead preserves native session logs/retention and closes this bounded pane before a fresh next task. Hindsight reference remains coding-agents0.8.2/Hindsight1.1.0 revision a5a619b5ca59c5c39e0f96be6f7a31fabd136bd5 and `/docker/hindsight/HINDSIGHT_OFFICIAL_PER_CALL.md`; reuse existing Rust initiative, never a new initiative or authority from memory.
