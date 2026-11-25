# Imported histories

This directory holds the whole tree and history of two former Academy repositories, merged into this one (not
copied), so that nothing is lost when the old repositories are archived. Each subdirectory is the source
repository's tree at the named commit, byte for byte, and its commits are ancestors of this repository's history.
No file under `imported/` is changed after the import.

| Directory | Source repository | Commit |
| --- | --- | --- |
| `dc-gang-a/` | `education-dc-gang-a` | `867db44f826b455b4fd7b0e1f4d281e65f9ce643` |
| `problem-edit/` | `education-problem-edit` | `3b76bbcb9cc30405a9eb4072bece9497a259ac69` |

## Not built, not tested

Nothing in this workspace builds or tests either directory: the Cargo workspace excludes `imported`, `./check` and
the CI job `rust` neither read nor build it, and no tool configuration of the root (TypeScript, bun, lint) picks its
files up. Their own manifests, workflows and checks (`Cargo.toml`, `package.json`, `.github/`, ...) are part of the
recorded tree only; GitHub runs no workflow below the root `.github/`. No test of this repository reads a file of
`imported/` as a fixture.

## Which copy is used

- `dc-gang-a/`: two copies of dc-gang-a exist in this repository, and they have diverged. The Academy's entry points
  use `apps/academy/dc-gang-a/`, which stays as it is. `imported/dc-gang-a/` is the former repository's own state.
- `problem-edit/`: the only copy in this repository. No entry point of this workspace uses it; the teacher's
  problem-edit sessions work here after the cutover of the old repository.

## What survives

The Rust migration, which removes the legacy modules, decides what of `imported/` survives and which of the two copies
of dc-gang-a wins. Until then both stay as they are.
