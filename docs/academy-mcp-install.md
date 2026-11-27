# Installing a release of the Academy server

The Academy server (`academy_operations`, source `modules/academy-workbench/tools/academy-mcp`) is installed from a
checkout of this repository by the scripts in that module's `install/` directory, and read back by
`scripts/academy-mcp-installed.sh`. This page is the install of one release, as commands with the lines they print.
Placeholders are in angle brackets; a printed hash is shown as `<12 hex>`. In printed lines `<base>`, `<module>` and
`<G>` stand for the values of `$B`, `$M` and `$G` below.

## What active means

An install base holds `launcher.mjs`, `releases/<generation>/` (one directory per generation, never changed after it is
written) and `release.json`, the pointer. The active generation is the member `generation` of `<base>/release.json`.
A script activates a generation by writing a new pointer beside the old one and renaming it over `release.json`; the
launcher reads the pointer at the start of every server process (`src/runtime-release.ts`). No CLI's registration is
edited: each registration points at the base's launcher, so every new session starts the active generation. A CLI that
is already running may show its cached tool list until it is restarted.

## Inputs that are the host's, never the repository's

- The private configurations under the artifacts root: the files the endpoint specifications in `endpoints.json` name
  beside their bundles. The scripts copy `endpoints.json` from the active generation, checked by its SHA-256, and change
  only the dailyrecord bundle's path and pin in it.
- The routing admission file that `routing.json` names. It is copied with `routing.json` as it is.
- The secret file the endpoints read. No install script opens it.

Nothing of these is in the repository, and nothing of the repository replaces them.

## Before the install

- A clean checkout of this repository at the release's merge commit, `<checkout>`, used by nobody else while the
  install runs. `git -C <checkout> rev-parse HEAD` prints `<commit>`, 40 hexadecimal characters. Each script finds
  the checkout from its own place (the top of the Git work tree it lies in); nothing names it by a written path.
- The module's dependencies in that checkout: `cd <checkout>/modules/academy-workbench/tools/academy-mcp && bun install --frozen-lockfile`.
  The builds use this `node_modules` and nothing else.
- No lesson session runs: each step below activates its generation, and between the steps the server runs a part of
  the release.

```sh
C=<checkout>
M=$C/modules/academy-workbench/tools/academy-mcp
B=/root/.local/share/academy-mcp-host-20261106   # the install base
G=<generation>                                  # for example host-<YYYYMMDD>-<first 7 of commit>-v6
```

Each script refuses an argument it does not know or lacks with its usage and exit 2, before any effect. They write
only under `--base`, `--artifacts` and `--kept`; a build runs in a temporary directory under `--base` that is removed
after it. `--kept <dir>` (default `<base>/pointers`) is where the pointer before each activation is kept.

## 1. The bridge

`loop_install.py` builds `src/neutral-server.ts` at the commit into `server.mjs` with its pin (`bridge-pin.json`),
copies the active generation's routing, registry and endpoints, writes `provenance.json` with `sourceCommit`, and
activates the generation. The surface is pinned in step 4, so it is `none` here.

```sh
python3 -I "$M/install/loop_install.py" <commit> "$G-bridge" none --base "$B"
```

```
restore with: python3 -I <module>/install/loop_install.py --restore <base>/pointers/pointer.before-<G>-bridge.academy-mcp-host-20261106.json --base <base>
activated <G>-bridge from <12 hex> server <12 hex> surface none
tools <N> bytes <B> names_sha <12 hex>
```

The kept pointer of the first line is the undo of the whole release (section 6). The script refuses an active pointer
of a schema other than `academy.mcp-release/1` (`host_pointer_schema_unexpected`) and a base without `release.json`
(`base_pointer_missing@<base>`).

### Admitting students to the paper scopes

`loop_install.py` copies the active `routing.json` as it is. To let a new generation route the paper reads of more
students, give `--paper-admit <endpoint>=<id>[,<id>...]` (repeatable, one endpoint per occurrence) to the same call:
each id is added after the ids of that endpoint's existing `paperScopes` entry, and nothing else of `routing.json`
changes. The contract is `specs/academy-mcp/paper-admit.md`.

```sh
python3 -I "$M/install/loop_install.py" --paper-admit-preview --base "$B" --paper-admit paper1=<id>,<id> --paper-admit paper2=<id>
python3 -I "$M/install/loop_install.py" <commit> "$G-bridge" none --base "$B" --paper-admit paper1=<id>,<id> --paper-admit paper2=<id>
```

```
routing sha256 <64 hex> admitted <count>
```

The preview prints only that line and writes and builds nothing; the install prints the same line, with the hash of the
`routing.json` it writes, before `restore with:`. An id is 1 to 16 digits, the first not 0, in no entry yet; an endpoint
must already have an entry (none is created) and an entry holds at most 1000 ids. A refusal is one code on standard
error with exit 1 and writes nothing in the base: `paper_admit_form`, `paper_admit_id_invalid@<n>`,
`paper_admit_endpoint_unknown@<endpoint>`, `paper_admit_id_present@<n>`, `paper_admit_limit`. `<n>` is the 1-based
position of the id in its list; an id is never printed. The option is refused with `--restore`.

## 2. The endpoints

`endpoint_install.py` builds `src/dailyrecord-server.ts` at the commit, writes it under the artifacts root as
`<artifacts>/<artifact-name>/dailyrecord-server.mjs`, and writes a generation whose `endpoints.json` starts the new
bundle with its pin; everything else is the active generation's. `<artifacts>` is the directory that holds the
directory of the active dailyrecord bundle (the path in the dailyrecord specification of the active `endpoints.json`);
`--owner-uid` is the owner of that bundle (`stat -c %u <its directory>`), and the new directory and bundle get it.
Without `--owner-uid` nothing changes owner. `--probe` lists the tools of the old and the new dailyrecord endpoint.

```sh
python3 -I "$M/install/endpoint_install.py" <commit> "$G-endpoint" dailyrecord-<first 7 of commit> \
  --base "$B" --artifacts <artifacts> --owner-uid <uid> --probe --activate
```

```
staged <G>-endpoint from <12 hex> dailyrecord <12 hex> at <artifacts>/dailyrecord-<first 7 of commit>/dailyrecord-server.mjs
probe dailyrecord of <G>-bridge tools <n> names_sha <12 hex>
probe dailyrecord of <G>-endpoint tools <n> names_sha <12 hex>
activated <G>-endpoint previous <G>-bridge kept <base>/pointers/pointer.before-<G>-endpoint.academy-mcp-host-20261106.json
pointer <G>-endpoint tools <N> names_sha <12 hex>
```

## 3. The composition entries

The bridge composes `academy_dailyrecord_get` and `academy_dailytest_history_with_headers`; their registry entries come
from the checkout's source, without starting an endpoint.

```sh
E=$(mktemp -d)
bun "$M/install/composition_entries.ts" > "$E/entries.json"
```

It prints nothing on the terminal; `entries.json` holds a JSON array of two entries.

## 4. The registry and the release's surface

`registry_surface_install.py` refreshes every registry entry of the dailyrecord endpoint from the tools the new
endpoint lists (`--refresh`), replaces the two composition entries (`--compositions`), pins the release's surface
`release/surface.json`, writes `--commit` as `sourceCommit` of the generation's `provenance.json`, and activates it.
`-` adds no tool.

```sh
python3 -I "$M/install/registry_surface_install.py" "$G" "$M/release/surface.json" dailyrecord - \
  --base "$B" --refresh --compositions "$E/entries.json" --commit <commit> --activate
```

```
kept_not_listed <tool>            (one line for each registry entry the endpoint does not list, if any)
staged <G> registry <n> added [] refreshed <k> surface <12 hex>
activated <G> previous <G>-endpoint kept <base>/pointers/pointer.before-<G>.academy-mcp-host-20261106.json
pointer <G> tools <N> bytes <B> names_sha <12 hex>
```

A surface that names a tool the registry does not hold is refused (`surface_names_unknown_tool@<names>`) before any
generation is written.

## 5. The read-back

The last line of step 4, `pointer <generation> tools <N> bytes <B> names_sha <12 hex>`, is `tools/list` answered
through the launcher. Then:

```sh
"$C/scripts/academy-mcp-installed.sh" --base "$B" <commit>
```

```
generation <G>
commit <commit>
```

`scripts/academy-mcp-installed.sh [--base <dir>] <commit>` reads `<base>/release.json` (default base
`/root/.local/share/academy-mcp-host-20261106`), takes its `generation`, reads `releases/<generation>/provenance.json`
and prints these two lines. A provenance without `sourceCommit` is followed through its `priorGeneration`, at most five
steps, and the second line then ends ` (prior <generation>)`. Exit 0: the commit is the active generation's own and
begins with the given one. Exit 1: it is another commit, or it was found only through a prior generation. Exit 2: no
release record or no commit is found. It changes nothing.

## 6. The restore

```sh
python3 -I "$M/install/loop_install.py" --restore "$B/pointers/pointer.before-$G-bridge.academy-mcp-host-20261106.json" --base "$B"
```

```
restored <the generation active before step 1>
tools <N> bytes <B> names_sha <12 hex>
```

The restore checks that every file the kept pointer pins lies under `<base>/releases/` and has its SHA-256, then
renames the kept bytes over `release.json` and reads `tools/list` back. Any kept pointer restores the state before its
step; the one of step 1 restores the state before the release. No generation is deleted.

## What the release changes for a caller

The errors the lesson days logged from 2026-11-13 to 2026-11-24, one row per group.

| Code | Tool | Input member | After this release |
| --- | --- | --- | --- |
| `tool_input_invalid` | `academy_dailyrecord_intake` | without `context` (the flat form) | Removed by pull request 148: the context members may be given at the top level. |
| `tool_input_invalid` | `academy_dailyrecord_review` | `memoAnswer` `no_addition` and no `requestRef` | Removed by pull request 148. |
| `tool_input_invalid` | `academy_dailyrecord_intake` | `context` as a string | The caller passes an object. |
| `tool_input_invalid` | `academy_dailytest_history_with_headers` | without `lessonDate` | The caller passes `lessonDate`; `today` is taken. |
| `textbook_binding_missing_or_ambiguous` | `academy_textbook_section_results_get` | an inexact `bookLabel` | Removed by pull requests 144 and 147: one binding is picked, else the answer is `interview_required`. |
