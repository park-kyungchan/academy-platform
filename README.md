# Education platform

The maintained implementation is a Rust Cargo workspace:

- `edu-academy-core`: shared Academy domain semantics and validation.
- `edu-academy-sources`: bounded source adapters and offline tooling.
- `edu-academy-mcp`: MCP transport and synthetic endpoint qualification.

The Academy Rust replacement is not yet qualified for native activation. Retained TypeScript source, migration fixtures and private context remain available to their existing consumers until their specific retirement conditions are met. Repository checks do not authorize Academy calls, deployment, record writes or parent transmission.

Run the pinned Rust checks from the repository root:

```sh
# Match CI: large debug symbols can exceed the bounded synthetic artifact size.
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --bins --examples --locked
source scripts/rust-test-env.sh /tmp/education-rust-fixtures
cargo test --workspace --locked
```

Use a disposable fixture directory and remove it afterwards. The build step supplies the fictional endpoint executables used by the tests. GitHub Actions requires the `rust` check; retired Bun integration and automated review workflows no longer define the maintained verification route.

The former integration source and its historical evidence are recoverable from Git, including `archive/education-platform-pre-delw1-A-20261105`. Mathematical acceptance, human disposition, teacher acceptance and native activation remain separate decisions. Native profiles, credentials, sessions, databases and student originals retain their owners.

Rust fixture ownership and historical replay recovery are documented in
[docs/fixture-ownership.md](docs/fixture-ownership.md). Current Cargo checks read
crate-owned fixtures; retained migration scripts require their complete original
revision for historical reproduction.
