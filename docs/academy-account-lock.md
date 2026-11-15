# Academy account exclusion

Every installed Rust owner that can authenticate must publish a private regular
`account-lock.json` inside its canonical owner root. The document has this exact
shape (the path below is a fictional example):

```json
{"schemaVersion":"academy.account-lock/1","path":"/absolute/owner-published/shared-account.lock"}
```

The document follows OwnerRoot's existing private-file checks (owner UID, mode
without group/other access, single link, same device, bounded stable capture).
The path must be absolute and canonical and name an existing regular file.
Missing configuration, a missing file, a symlink or a nonregular file refuses
account admission. There is no default path and Rust never creates the shared
file. The installed owner must configure the SAME path its TypeScript launch
chain passes to `/usr/bin/flock --nonblock`. Merely selecting a different file
does not coordinate the runtimes. Configuration publication and native activation
remain the installed owner's responsibility; source tests do neither.

`OwnerRoot::account` acquires locks in this fixed order:

1. Open the configured shared file read-only, without create, truncate or symlink
   following; acquire a nonblocking exclusive kernel `flock`.
2. Open/create the existing `account-<account-hash>.lock` under the owner root;
   acquire its existing nonblocking exclusive lock and validate its pending marker.

A busy shared lock returns `owner_account_busy` before the owner marker is even
opened and before any transport request. Both locks remain held for the entire
account lease. `Owner::leased` retains the lease in its shared client transport,
so source/write views retain it after the Owner handle drops. `NativeSession`
retains it through login, reads, effects and reconciliation. Production
`NativeTransport` also retains the lease and refuses dispatch without one
(`owner_account_lease_required`); wrappers must forward `Transport::bind_account`.
The literal synthetic loopback constructor remains usable without a real owner.

Dropping the final lease releases the owner lock and then the shared lock,
including when another inherited open-file description exists. Process exit or
crash releases both kernel locks. Rust never writes, truncates, renames or unlinks
the shared file. The owner file alone retains its existing pending-effect marker
and reconciliation semantics. A marker refusal releases the shared lock too.
Both acquisitions are nonblocking, with the same order for all Rust owners.

The shared pathname and inode must stay stable while any runtime is active.
Lease validation checks that the shared pathname still names the admitted inode
before production dispatch and owner-marker operations. Kernel flock is advisory:
clients that do not participate cannot be excluded. Same-UID malicious path
replacement is outside OwnerRoot's trust boundary. No lock service, registry,
launcher change or browser exclusion is introduced.

Synthetic checks are in `crates/edu-academy-sources/tests/account_lock.rs` and
`tests/owner_transport.rs`: external launcher exclusion, transport count zero,
reverse exclusion, drop/process-death release, unchanged shared inode/size/mtime,
and missing/nonregular/unconfigured refusals. All files are disposable fixtures;
no production lock or LMS endpoint is used.

The capture family `dailyrecord` uses the same NativeSession and Recorder as the
textbook and DailyTest families. Its single lease covers every date's sequential
read and publication; no per-date session or separate account lock is opened.
The spool's `.lesson-record.lock` protects only local partition replacement and
never substitutes for the account lease. An invented in-memory transport test in
`dailyrecord_tests.rs` verifies one login for three dates, shared-lock exclusion
until session drop, and source-only raw recording.

`edu lesson` opens the same NativeSession only for a run that captures, and holds its lease
for every date of that run. It is the one Rust owner that retries: when `NativeSession::open`
ends with `owner_account_busy` it opens again every 250 ms until the configured
`lockWaitSeconds` have passed, sending no request meanwhile, then ends with that code. Each
attempt takes the locks in the order above; nothing is held between attempts. Line 1 and the
run log name the seconds it waited.
