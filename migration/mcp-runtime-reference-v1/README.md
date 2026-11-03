# S11 unchanged-original reference v1

Read [CONTRACT.md](CONTRACT.md) before running anything. This directory is an
offline migration observer/corpus, not a product entrypoint or S11 acceptance.
Existing installed dependencies only; no installation, Cargo or native runtime.

Verify the immutable reference and its exact source/dependency pins:

```sh
python3 migration/mcp-runtime-reference-v1/judge.py --source-root "$PWD"
python3 migration/mcp-runtime-reference-v1/judge.py --controls
```

An independent reviewer can execute one original mode against a NEW owned
directory. Every effect stays there; never point this at an actual install.
Each mode has its own 90-second wall ceiling. Do not use an aggregate timeout
around all modes or reuse an output path.

```sh
python3 migration/mcp-runtime-reference-v1/observe.py \
  --source-root "$PWD" --mode protocol \
  --output /absolute/owned/new-disposable-protocol
```

Modes: inventory, discovery, protocol, composition, graph, identity, framing,
bounds, launcher, relocation. `inventory` reads only the specifically authorized
public installed pointer/registry metadata and replays it locally. All other
mode endpoints are entirely disposable. No `--dailyrecord-native` is used.
`graph.py` reads the named public installed descriptors/executables, extracts
metadata and writes the requested output; it never executes them or opens their
referenced private payload/config/admission files.

`freeze.py` is one-shot and refuses an existing frozen destination. Never
regenerate these expected outputs from a candidate implementation. Fix forward
in new versioned artifacts with an independently reviewed contract reason.
The manifest's nativeRoot locations contain full syscall traces/effect files;
the committed corpus retains actual raw channels and process metadata. Failed
attempts stay in Root's exact reference-v1/native notes with their own causes.

`judge.py --compare MODE --actual observations.json --mode-root ROOT` reports
specific differences against frozen actual output. Identity hashes are strict;
relocation-derived hash differences are reported, never silently accepted.
Independent native graph and source authorization evidence remain necessary.

`GRAPH-v2.json` is the corrected static graph view. `GRAPH.json` and
`PINS-before.json` preserve early scan attempts; use PINS-before-v2 for the
authoritative source/dependency closure. No original oracle or product source
was updated by this reference.
