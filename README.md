# Education platform

The maintained implementation is a Rust Cargo workspace:

- `edu-academy-core`: shared Academy domain semantics and validation.
- `edu-academy-sources`: bounded source adapters and offline tooling.
- `edu-academy-teacher`: the teacher-facing surface (record store, tools, offline harness).
- `edu-academy-ontology`: the `academy` schema and the engine ledger of save attempts.
- `edu-academy-mcp`: MCP transport and synthetic endpoint qualification. Frozen: not a default member; `./check all` covers it; it is retired at the first cutover.

The Academy Rust replacement is not yet qualified for native activation. Retained TypeScript source, migration fixtures and private context remain available to their existing consumers until their specific retirement conditions are met. Repository checks do not authorize Academy calls, deployment, record writes or parent transmission.

Run the one check from any directory of the repository:

```sh
./check        # format, Clippy, offline teacher harness, build, tests: the default members
./check all    # the same over the whole workspace, so the frozen `edu-academy-mcp` is built and tested too
```

The CI job `rust` runs the same file, and GitHub Actions requires that check. On the managed host it takes the build lock and makes (and removes) its private fixture directory by itself; elsewhere the fixture directory comes from `CHECK_FIXTURE_BASE`, else `$TMPDIR`. The build step supplies the fictional endpoint executables the tests use. Retired Bun integration and automated review workflows no longer define the maintained verification route.

The former integration source and its historical evidence are recoverable from Git, including `archive/education-platform-pre-delw1-A-20261105`. Mathematical acceptance, human disposition, teacher acceptance and native activation remain separate decisions. Native profiles, credentials, sessions, databases and student originals retain their owners.

Rust fixture ownership and historical replay recovery are documented in
[docs/fixture-ownership.md](docs/fixture-ownership.md). Current Cargo checks read
crate-owned fixtures; retained migration scripts require their complete original
revision for historical reproduction.
