# S11 Rust facade — first source increment

This is source-only preparation, uncompiled and unexecuted. It is not full S11, protocol parity, native graph admission or activation. The accepted original-only qualifier at051 is unchanged.

`edu-academy-mcp` contains the retained legacy initialize/version preference, ping, tools/list, tools/call envelopes, typed IDs, ordered UTF-16 Js codec, source-metadata registry and a guarded JSON Schema subset. SDK2.3.1 source anchors are server dist/mcp-DIH4cS6P.mjs:1338, src-Cqbh3MYc.mjs:6486/6540/7159 and core dist/auth-BNDyLTqp.mjs:4. Exact SDK invalid-parameter issue serialization and all regexp/format edges still require qualification. No defaults or domain refinements are invented at the bridge.

The one embedded identity-tool metadata object is copied from the public original inventory stdout at migration/mcp-runtime-reference-v1/frozen/inventory/commands/neutral/stdout.bin (SHA25625ab9db16ecf5bcb4f4cdc2a8fee795a2616bc88c5a8e1428e046e7919774fd0). No business answers/captures are embedded in production code. Release registries supply the other tool metadata. Configured routes deliberately return isError/qualification gap because their actual Rust adapters are not in this grant; tools/list is metadata, not a claim of implemented operations.

## Versioned graph

V3 chooses terminal mismatch refusal for a whole MCP connection. Startup requires `--install-root /absolute/canonical/private/root`; no cwd or home defaults, legacy conversion or hot loading. The root has an owner-readable release.json (<=65536 bytes):

- schemaVersion=academy.mcp-release/3 and generation matching1..64 [A-Za-z0-9._-], excluding . and ...
- bridge, launcher, build, routing, endpoints, registry and owner pins, each {path:absolute,sha256:lowercase64hex}.
- build/owner/routing/endpoints/registry/endpoint manifests <=4MiB; immutable pinned artifacts <=64MiB. These are V3 artifact budgets, not domain or MCP response limits.

Owner manifest: strict {schemaVersion:academy.rust-owner/1,kind:fictional,uid,installRoot,evidenceRoot,stateRoot}. Each root must already exist, be distinct, private and owned by the effective UID, with canonical no-follow ancestry that is not writable by other users. Native/container kind is explicitly refused until actual owner binding is implemented and independently proven. Source/evidence and mutable state roots have distinct roles. Nothing creates, migrates or erases a store.

Build manifest: strict {schemaVersion:academy.rust-build/1,sourceRevision:40hex,semanticContractVersion,cargoLockSha256:64hex,artifacts:[{role,path,sha256}]}. It binds declared immutable artifacts but does not establish loaded dynamic-library closure, compiler provenance or native consumer loading by itself.

Routing retains {admissionPath,admissionSha256,paperScopes?}; its admission is included in pinned graph reads. Public registry entries retain strict {endpoint,upstreamName,tool}, where tool metadata remains source-owned. Endpoints map exact endpoint names to {executable:pin,identity:pin,args:[--install-root,root,--endpoint,name]}. Endpoint identity is strict {schemaVersion:academy.rust-endpoint/1,endpoint,semanticContractVersion,launchArgumentsSha256,artifacts};1..128 artifacts and exactly one matching entry. Roles are entry/source/policy/admission/config/dependency. All declared immutable pins are checked; no shell, Bun, caller command or credential environment is selected.

Every request checks the frozen pointer/artifacts/root identity before handling and before returning. Any observed change/removal latches refusal, even if original bytes are restored. New connections independently open a complete graph. Pin validation is bounded descriptor-relative file admission with before/after metadata and a same-descriptor second byte read; it is not an atomic multi-file snapshot or protection against every malicious same-UID mutation.

The launcher verifies its loaded executable inode/digest, verifies the bridge graph and replaces itself with the native bridge through exec. Stdio/native exit status are preserved. The bridge also verifies its loaded executable. A moved executable with a stale path pin refuses; a legitimate move requires a new separately qualified complete graph, with data/state provenance unchanged. No runtime installation/config publication is performed.

## Limit and effect scopes

The original stdio-diagnostics.ts input adapter alone establishes1MiB LF-inclusive input lines. It reports malformed lines/EOF fragments without payloads, accepts CRLF/split input, and caps its diagnostics at16; protocol diagnostics have their own cap16. Pinned SDK default child receive buffer is10MiB, a distinct stream contract. No generic1MiB output/domain cap is introduced. Original product `_meta` addition has a524288-byte SOFT omission boundary: a valid600000-byte result remains intact, including its dual text/structured representation.

Possible/returned effects remain unknown, with no automatic retry. Confirmed readback evidence retains its actual count/status. Durable workflow fences, owner locks and reconciliation are next-increment joins to existing core APIs; this increment does not create replacement receipts/journals.

## Open implementation and proof gaps

- Safe parent SIGTERM/cancellation integration, kill/reap/drain/escaped-descendant and same-inode flock proof. Existing rustix1.1.2 can send group signals and set parent-death/subreaper attributes, but that does not implement the required lifecycle. `require_supervised_execution` refuses; there is no child-spawn fallback.
- Core intake/durable observation issuance, Source/SourcePort/HttpPort/Policy adapters and product dispatch/result/failure receipts. Existing core/source files remain unchanged.
- Saved capabilities/status/labels, paper routes, metadata/item/prestudy/textbook/CISM owner source adapters, operational HTTP/TLS/cookie/auth and native owner/profile/UID graph proof. Configured endpoint mode refuses pending these ports.
- Pending-write graph switches and recovery against actual existing journals; native argv/stdio/status/cleanup evidence; reference generated-role binding and real candidate comparisons. No new recorder/grader or replay of accepted originals.

Authored tests cover framing/typed IDs/omissions, source metadata bounds, metadata soft omission, graph pointer restoration, source tamper, legacy refusal, no fake configured success, explicit roots/symlinks and stale relocation pins. Filesystem tests require explicit S11_TEST_ROOT under allocated own notes, retain all fictional artifacts and execute no external endpoints. They are unexecuted. Cargo.lock remains protected and lacks the new workspace package entry, so Root must allocate lock generation before --locked compilation.
