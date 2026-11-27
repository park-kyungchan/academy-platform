# Admitting students to the paper scopes at install: `loop_install.py --paper-admit`

## Purpose

The TypeScript Academy server routes the paper reads of a student only to an endpoint whose
`paperScopes` entry in the release's `routing.json` lists that student's id
(`modules/academy-workbench/tools/academy-mcp/src/paper-scope-routing.ts`). Today an install
copies `routing.json` of the active generation unchanged, so a new student is admitted only
by editing an installed file by hand. The option `--paper-admit` of
`modules/academy-workbench/tools/academy-mcp/install/loop_install.py` admits student ids to
an existing entry while it installs a new generation, and leaves everything else of the
routing as it was. It is relied on by the owner who installs a generation and by the
server, which loads the result. Its option `--check-roster-students` refuses, before an install writes
anything, a routing that leaves an active roster student in no entry or in two, so that no
paper read of a roster student ends in `paper_scope_unresolved`.

## Requirements

- R1: Without `--paper-admit`, the new generation's `routing.json` is the active generation's `routing.json` byte for byte, and the new pointer's `routing.sha256` is the active pointer's. (no test yet; ars-ts-release observes it)
- R2: The option is `--paper-admit <endpoint>=<id>[,<id>...]` of the install form: `<endpoint>` matches `[a-z][a-z0-9_]{0,63}` and the list is one or more ids joined by single commas, without spaces; a value of any other shape (no `=`, an empty endpoint, an empty list, an empty id between commas) refuses with `paper_admit_form`. Tests: `paper_admit_r2_form`.
- R3: The option may be given more than once, one endpoint per occurrence; the same endpoint in two occurrences refuses with `paper_admit_form`. Tests: `paper_admit_r3_endpoint_twice`.
- R4: The same id twice in the option, in one list or in the lists of two occurrences, refuses with `paper_admit_form`. Tests: `paper_admit_r4_id_twice`.
- R5: The option given with `--restore` refuses with `paper_admit_form`. Tests: `paper_admit_r5_restore`.
- R6: An id that is not 1 to 16 ASCII digits with a first digit other than 0 refuses with `paper_admit_id_invalid@<n>`. Tests: `paper_admit_r6_id_invalid`.
- R7: `<n>` in a refusal is the 1-based position of the id in the list of its occurrence, never the id itself. Tests: `paper_admit_r7_position_not_id`.
- R8: No id given to the option appears on standard output or standard error, whether the call succeeds or is refused. Tests: `paper_admit_r8_no_id_printed`.
- R9: An endpoint that has no `paperScopes` entry in the active `routing.json` (also when that file has no `paperScopes`) refuses with `paper_admit_endpoint_unknown@<endpoint>`, `<endpoint>` as given; no entry is ever created. Tests: `paper_admit_r9_endpoint_unknown`.
- R10: An id that is already in any `paperScopes` entry of the active `routing.json`, of the same endpoint or of another, refuses with `paper_admit_id_present@<n>`. Tests: `paper_admit_r10_id_present`.
- R11: When an entry would hold more than 1000 ids after the admission, the call refuses with `paper_admit_limit`. Tests: `paper_admit_r11_limit`.
- R12: The checks run in the order R2 to R5, R6, R9, R10, R11, the occurrences in the order given and the ids of a list in its order, and the first check that fails is the call's one refusal. Tests: `paper_admit_r12_order`.
- R13: A call refused with a code of this option ends with exit status 1 and the code as the last line of standard error, and writes nothing in the base: no file or directory under the base is created, changed or removed (`release.json`, `releases/`, the kept pointers, a build directory), so the active generation stays active. Tests: `paper_admit_r13_refusal_writes_nothing`.
- R14: In the new generation's `routing.json`, the `studentIds` of each admitted endpoint's entry are the active entry's ids in their order, followed by the admitted ids in the order of the option's list. Tests: `paper_admit_r14_ids_appended`.
- R15: Admitting keeps every other member of `routing.json` byte for byte: `admissionPath` and `admissionSha256` are the same strings when present and stay absent when absent, no member is added, and every `paperScopes` entry of an endpoint not admitted to has the same endpoint and the same ids in the same order, the entries in their order. Tests: `paper_admit_r15_other_members_kept`.
- R16: The new `routing.json` is valid under the strict routing schema of `src/runtime-release.ts` (the members `admissionPath`, `admissionSha256` and `paperScopes` only) and under `paperScopesSchema`, so the server loads the new generation. (no test yet; ars-ts-release observes it)
- R17: The new pointer's `routing` names `releases/<generation>/routing.json` of the new generation, and its `sha256` is the sha256 of that file's bytes. (no test yet; ars-ts-release observes it)
- R18: `registry.json` and `endpoints.json` of the new generation are those of the active generation byte for byte, with or without the option. (no test yet; ars-ts-release observes it)
- R19: The active `routing.json` is the file the base's `release.json` names under `routing`, and it is admitted to only after its bytes match that entry's `sha256`. Tests: `paper_admit_r19_hash_checked`.
- R20: `loop_install.py --paper-admit-preview --base <dir> --paper-admit <endpoint>=<id>[,<id>...]...` reads the active `routing.json` as an install does (R19), applies the checks and refusals of R2 to R12, and prints the one line `routing sha256 <hex> admitted <count>` on standard output with exit status 0, where `<hex>` is the sha256 of the `routing.json` an install with the same options would write and `<count>` is the number of admitted ids; it writes nothing in the base and builds nothing, and it takes no `<commit> <generation> <surface.json|none>`. Tests: `paper_admit_r20_preview_writes_nothing`, `paper_admit_r14_ids_appended`.
- R21: An install that is not refused prints the same line `routing sha256 <hex> admitted <count>` once, before the line of its activation, where `<hex>` is the sha256 of the new `routing.json` as R17 pins it and equals the sha256 that the preview of the same options prints. (no test yet; ars-ts-release observes it)
- R22: A roster is covered when every one of its student ids is in exactly one `paperScopes` entry: an id in no entry, or in two, is a student whose paper reads the server refuses with `paper_scope_unresolved`. Tests: `paper_admit_r22_roster_covered`.
- R23: The option `--check-roster-students <id>[,<id>...]`, given to the install form or to the preview of R20, checks that the `routing.json` the call would write (the active one, with the admissions of the same call) covers the roster that the option lists, before any file of the base is written: an id in no entry refuses with `roster_student_unrouted@<n>`, an id in more than one entry with `roster_student_duplicate@<n>`; a covered roster changes nothing else of the call. Tests: `paper_admit_r23_r25_check_roster_students_preflight`.
- R24: The list of R23 is one or more ids joined by single commas, without spaces and without an id twice, or the call refuses with `roster_students_form`; an id that is not 1 to 16 ASCII digits with a first digit other than 0 refuses with `roster_student_id_invalid@<n>`. Tests: `paper_admit_r23_r25_check_roster_students_preflight`.
- R25: `<n>` in a refusal of R23 and R24 is the 1-based position of the id in the option's list, never the id; such a refusal ends with exit status 1, its code as the last line of standard error, nothing on standard output, and writes nothing in the base. Tests: `paper_admit_r23_r25_check_roster_students_preflight`.

## Interface

### Form

```
python3 -I loop_install.py <commit> <generation> <surface.json|none> --base <dir> [--kept <dir>] [--paper-admit <endpoint>=<id>[,<id>...]]...
```

The same options with `--paper-admit-preview --base <dir>` in place of the positional words and `--base`
(R20). An example with invented ids: `--paper-admit paper1=90001,90002 --paper-admit paper2=90003`
admits two ids to the entry of `paper1` and one to the entry of `paper2`.

### Refusal codes

| Code | When |
| --- | --- |
| `paper_admit_form` | R2, R3, R4, R5 |
| `paper_admit_id_invalid@<n>` | R6 |
| `paper_admit_endpoint_unknown@<endpoint>` | R9 |
| `paper_admit_id_present@<n>` | R10 |
| `paper_admit_limit` | R11 |
| `roster_student_unrouted@<n>`, `roster_student_duplicate@<n>` | R23 |
| `roster_students_form`, `roster_student_id_invalid@<n>` | R24 |

`<n>` is a position (R7). Every other refusal of the install keeps its code and behaviour
(for example `copy_hash_mismatch@routing`, `generation_exists@<path>`).

### Limits of `paperScopesSchema`

| Limit | Value |
| --- | --- |
| Entries in `paperScopes` | 1 to 32 |
| Endpoint of an entry | `paper1` or `paper2`, each at most once |
| Ids of an entry | 1 to 1000, distinct within the entry |
| An id | 1 to 16 ASCII digits, the first not 0 |

The schema allows one id in two entries, but the server then routes that student to no
endpoint (it needs exactly one match), so the option refuses an id present in any entry
(R10). Because no entry is created (R9), the entry limit cannot be passed by an admission;
the server reads a `routing.json` of at most 4 MiB, which 32 full entries do not reach.

### The preview

```
python3 -I loop_install.py --paper-admit-preview --base <dir> --paper-admit <endpoint>=<id>[,<id>...]...
```

Its one line of output is `routing sha256 <hex> admitted <count>` (R20). An active
`routing.json` that is not JSON refuses with `routing_invalid`; the contract says no more
about a routing that the server's strict schema refuses.

### The result

`<base>/releases/<generation>/routing.json`, one JSON document, pinned by the new
`<base>/release.json` as `{"path": ..., "sha256": ...}` under `routing`. An example of the
members before and after an admission of the invented id `90002` to `paper1`:

```json
{"admissionSha256":"<64 hex>","paperScopes":[{"endpoint":"paper1","studentIds":["90001"]},{"endpoint":"paper2","studentIds":["90003"]}]}
{"admissionSha256":"<64 hex>","paperScopes":[{"endpoint":"paper1","studentIds":["90001","90002"]},{"endpoint":"paper2","studentIds":["90003"]}]}
```

### The roster check

```
python3 -I loop_install.py <commit> <generation> <surface.json|none> --base <dir> [--paper-admit ...]... --check-roster-students <id>[,<id>...]
python3 -I loop_install.py --paper-admit-preview --base <dir> [--paper-admit ...]... --check-roster-students <id>[,<id>...]
```

An example with invented ids: `--check-roster-students 90001,90003` before an install that
routes `90001` to `paper1` and `90003` to `paper2`. The list is the active roster's student
ids, as the owner reads them; the option reads no roster itself.

### Tests

`scripts/test-academy-mcp-install.sh`, section 5, on a scratch base with invented ids. The
rules R22 to R25 are tested by
`modules/academy-workbench/tools/academy-mcp/tests/paper-scope-routing.test.ts`, which runs
`python3 -I install/loop_install.py --paper-admit-preview` with the option on a scratch base. Its
refusals run through the install itself, which refuses before any build; a success is read
through `--paper-admit-preview`, which builds and writes nothing. The test for R14 and R15
compares the preview's hash with the hash of the expected `routing.json`, computed apart from
the script. A build needs the module's dependencies and the `bun` of the host, so no test
installs a generation; the rules that need one are observed by the install of ars-ts-release.

## Not included

- Removing an id from an entry, moving an id between endpoints, or creating an entry.
- `--restore`, which keeps writing back a kept pointer as it does; a kept pointer of an
  earlier generation restores that generation's routing.
- The whitespace and member order of the new `routing.json` beyond R15; only its values and
  its pin are promised.
- An active `routing.json` that the server's strict schema refuses: the server does not load
  such a release, and this contract does not say what the option does with it.
- `endpoint_install.py`, `registry_surface_install.py` and the server as a whole.
