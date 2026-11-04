//! Named offline adapter. Explicit requests only; no reference, case inventory or default root.
#![forbid(unsafe_code)]
use edu_academy_sources::{data::*, dom, http, native, owner_root, presentation, saved};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
};
fn read_text(v: &Js) -> Result<String> {
    if !v.string() {
        return Err(Error::error("offline_text_required"));
    }
    String::from_utf16(v.units()).map_err(|_| Error::error("offline_scalar_source_required"))
}
fn now() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let days = (ms / 86400000) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        ms / 3600000 % 24,
        ms / 60000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
fn uuid() -> String {
    use std::io::Read;
    let mut b = [0u8; 16];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut b)
        .unwrap();
    b[6] = (b[6] & 15) | 64;
    b[8] = (b[8] & 63) | 128;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6],
        b[7],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}
fn trace_file(v: &Js) -> Result<Option<fs::File>> {
    use std::os::unix::fs::MetadataExt;
    if !v.has("effectTrace") {
        return Ok(None);
    }
    let s = read_text(v.get("effectTrace"))?;
    let p = Path::new(&s);
    let parent = p
        .parent()
        .ok_or_else(|| Error::error("effect_trace_path_invalid"))?;
    let st = fs::metadata(parent).map_err(|_| Error::error("effect_trace_path_invalid"))?;
    if !p.is_absolute()
        || fs::canonicalize(parent).ok().as_deref() != Some(parent)
        || st.uid() != rustix::process::geteuid().as_raw()
        || st.mode() & 0o077 != 0
    {
        return Err(Error::error("effect_trace_path_invalid"));
    }
    let fd = rustix::fs::open(
        p,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::APPEND
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|_| Error::error("effect_trace_open_failed"))?;
    let file = fs::File::from(fd);
    let st = file
        .metadata()
        .map_err(|_| Error::error("effect_trace_stat_failed"))?;
    if !st.is_file()
        || st.nlink() != 1
        || st.uid() != rustix::process::geteuid().as_raw()
        || st.mode() & 0o077 != 0
    {
        return Err(Error::error("effect_trace_ownership_required"));
    }
    Ok(Some(file))
}
fn run(v: &Js) -> Result<Js> {
    let time = now();
    match string(v.get("op")).as_str() {
        "inventory" => Ok(strings(&[
            "inventory",
            "native_config_schema",
            "native_configuration_probe",
            "legacy_paths",
            "saved_query",
            "saved_records",
            "paginate",
            "tool_bytes",
            "literal_key",
            "resolve_name",
            "pinned_local",
            "owner_saved",
            "owner_capture",
            "unadmitted_day_record",
            "dom",
            "source_function",
            "presentation",
            "http_session",
            "native_session",
            "operation_admission",
        ])),
        "saved_query" => {
            saved::load(Path::new(&read_text(v.get("root"))?))?.query(v.get("input"), &time)
        }
        "saved_records" => saved::load(Path::new(&read_text(v.get("root"))?))?
            .records(v.get("query"))
            .map(Js::Arr),
        "paginate" => saved::paginate(
            v.get("rows").array(),
            v.get("query"),
            &read_text(v.get("snapshot"))?,
            number(v.get("limit")).unwrap_or(f64::NAN),
            if v.has("cursor") {
                Some(v.get("cursor"))
            } else {
                None
            },
            number(v.get("byteBudget")).unwrap_or(f64::INFINITY),
            false,
            1,
        ),
        "native_config_schema" => saved::native_config(v.get("input")),
        "native_configuration_probe" => {
            if v.get("args").array().is_empty() {
                Ok(Js::Null)
            } else if v.get("args").array().len() != 3
                || !v.get("args").array()[0].is("--dailyrecord-native")
            {
                Err(Error::error("dailyrecord_native_arguments_invalid"))
            } else if rustix::process::geteuid().as_raw() != 10000
                || std::env::var("HERMES_HOME").as_deref() != Ok("/opt/data/profiles/dc-gang-a-dev")
            {
                Err(Error::error("native_dailyrecord_owner_required"))
            } else {
                Err(Error::error("offline_native_activation_unavailable"))
            }
        }
        "legacy_paths" => {
            let module = Path::new("modules/academy-workbench/tools/academy-mcp");
            let repo = module.parent().unwrap().parent().unwrap();
            Ok(obj(&[
                ("packageRoot", module.to_string_lossy().to_string().into()),
                ("repoRoot", repo.to_string_lossy().to_string().into()),
                (
                    "resultRoot",
                    repo.join(".local/rea-api")
                        .to_string_lossy()
                        .to_string()
                        .into(),
                ),
                (
                    "reaEntry",
                    module
                        .join("node_modules/rea-agents/scripts/rea.mjs")
                        .to_string_lossy()
                        .to_string()
                        .into(),
                ),
                (
                    "rootBasis",
                    "original_module_location_not_cwd_or_explicit_evidence".into(),
                ),
            ]))
        }
        "tool_bytes" => Ok(saved::tool_bytes(v.get("value")).into()),
        "literal_key" => native::literal_key(v.get("value")),
        "resolve_name" => Ok(native::resolve_name(v.get("name"), v.get("rows"))),
        "pinned_local" => {
            let c = owner_root::legacy_pinned(
                Path::new(&read_text(v.get("path"))?),
                &read_text(v.get("sha256"))?,
            )?;
            Ok(obj(&[
                ("sha256", c.sha256.into()),
                ("bytes", base64(&c.bytes, false).into()),
            ]))
        }
        "unadmitted_day_record" => {
            let root = owner_root::OwnerRoot::open(Path::new(&read_text(v.get("root"))?))?;
            let mut captures = vec![];
            for file in v.get("files").array() {
                captures.push(root.read(
                    &read_text(file.get("path"))?,
                    &read_text(file.get("sha256"))?,
                    2_000_000,
                )?);
            }
            let association = edu_academy_sources::evidence::OwnerAssociation {
                account: v.get("account").units().to_vec(),
                instance: v.get("instance").units().to_vec(),
                existing_owner_evidence: v.get("existingOwnerAssociation").clone(),
            };
            edu_academy_sources::evidence::project_day_record(
                &association,
                &captures,
                number(v.get("mainIndex")).unwrap_or(0.) as usize,
                number(v.get("xmlIndex")).unwrap_or(1.) as usize,
                v.get("selection"),
            )
        }
        "owner_capture" => {
            let root = owner_root::OwnerRoot::open(Path::new(&read_text(v.get("root"))?))?;
            let c = root.read(
                &read_text(v.get("path"))?,
                &read_text(v.get("sha256"))?,
                number(v.get("cap")).unwrap_or(4194304.) as usize,
            )?;
            Ok(obj(&[
                ("sha256", c.sha256.clone().into()),
                ("bytes", base64(&c.bytes, false).into()),
                ("identity", c.identity.value()),
            ]))
        }
        "owner_saved" => {
            let root = owner_root::OwnerRoot::open(Path::new(&read_text(v.get("root"))?))?;
            let (ctx, _captures) = root.saved(&read_text(v.get("manifestSha256"))?)?;
            ctx.query(v.get("input"), &time)
        }
        "dom" => {
            let source = read_text(v.get("html"))?;
            let doc = dom::Document::parse(&source);
            let duplicates = doc.select("#duplicate");
            let tbody = doc.select("#repair tbody");
            let handler = doc
                .select("#handler")
                .first()
                .copied()
                .map(dom::content)
                .unwrap_or_default();
            Ok(obj(&[
                ("spans", doc.span_observation()),
                ("duplicateIdCount", duplicates.len().into()),
                (
                    "firstDuplicateTag",
                    duplicates
                        .first()
                        .map_or(Js::Undefined, |e| e.value().name().to_uppercase().into()),
                ),
                (
                    "repairTbody",
                    tbody
                        .first()
                        .map_or(Js::Undefined, |e| e.value().name().to_uppercase().into()),
                ),
                (
                    "syntheticTbodyRange",
                    tbody
                        .first()
                        .and_then(|e| doc.range(*e))
                        .map_or(Js::Null, |r| r.value()),
                ),
                ("sourceFunction", dom::source_function(&handler, "onFoPrg")?),
                ("scriptExecuted", false.into()),
                ("resourceCount", doc.select("[src]").len().into()),
            ]))
        }
        "source_function" => {
            dom::source_function(&read_text(v.get("source"))?, &read_text(v.get("name"))?)
        }
        "presentation" => {
            let html = read_text(v.get("html"))?;
            let helper = read_text(v.get("helpers"))?;
            let value = if v.has("sourceRangeOverride") {
                let r = v.get("sourceRangeOverride");
                if number(r.get("start")).unwrap_or(-1.) < 0.
                    || number(r.get("end")).unwrap_or(-1.) < 0.
                {
                    return Err(Error::error(
                        "test_pool_presentation_source_range_unqualified",
                    ));
                }
                presentation::inspect_with(&html, &helper, |_, _| {
                    Some(dom::Range {
                        start: number(r.get("start")).unwrap_or(0.) as usize,
                        end: number(r.get("end")).unwrap_or(0.) as usize,
                    })
                })?
            } else {
                presentation::inspect(&html, &helper)?
            };
            let mut spans = vec![];
            let raw = |source: &str, r: &Js| {
                let u = text(source);
                let start = number(r.get("start")).unwrap() as usize;
                let end = number(r.get("end")).unwrap() as usize;
                Js::Str(u[start..end].to_vec())
            };
            let span = |kind: &str, source: &str, r: &Js| {
                obj(&[
                    ("kind", kind.into()),
                    ("range", r.clone()),
                    ("raw", raw(source, r)),
                ])
            };
            for c in value.get("controls").array() {
                spans.push(span("control", &html, c.get("sourceRange")));
                for o in c.get("options").array() {
                    spans.push(span("option", &html, o.get("sourceRange")));
                }
            }
            for f in value.get("functionEvidence").array() {
                let source = if f.get("source").is("selector") {
                    &html
                } else {
                    &helper
                };
                let mut s = span("function", source, f.get("sourceRange"));
                s.set("name", f.get("name").clone());
                spans.push(s);
                for l in f.get("literals").array() {
                    spans.push(span("literal", source, l.get("sourceRange")));
                }
            }
            for f in value.get("filterDeclarations").array() {
                spans.push(span("filter", &html, f.get("sourceRange")));
            }
            for c in value.get("gridColumns").array() {
                spans.push(span("column", &html, c.get("sourceRange")));
            }
            Ok(obj(&[
                ("value", value),
                ("requiredOriginalSpans", Js::Arr(spans)),
            ]))
        }
        "native_session" => {
            let port = native::RecordedPort::new(v.get("exchanges").array().to_vec());
            let mut backend = native::Backend::new(port, v.get("admission").clone());
            let results = v
                .get("scopes")
                .array()
                .iter()
                .map(|s| observe(backend.read(s, &time, &uuid())))
                .collect();
            Ok(obj(&[
                ("results", Js::Arr(results)),
                ("events", Js::Arr(backend.port.events)),
            ]))
        }
        "http_session" => {
            let trace = trace_file(v)?;
            let transport =
                http::RecordedTransport::new(v.get("recordings").array().to_vec(), trace);
            let mut client = http::Client::new(v.get("principal").clone(), transport)?;
            let mut results = vec![];
            for command in v.get("commands").array() {
                let r = match string(command.get("method")).as_str() {
                    "authenticate" => client.authenticate(
                        &read_text(v.get("username"))?,
                        &read_text(v.get("password"))?,
                    ),
                    "discover" => client.discover(command.get("input"), &time),
                    "execute" => client.execute(command.get("input"), &time),
                    "inspectWire" => client.inspect_wire(command.get("input")),
                    "journalState" => client.journal_state(command.get("input"), &time),
                    _ => Err(Error::error("offline_http_method_unselected")),
                };
                results.push(observe(r));
                if command.get("stopUnlessAuthenticated").truthy() && !client.authenticated {
                    break;
                }
            }
            let before = client.authenticated;
            client.close();
            Ok(obj(&[
                ("results", Js::Arr(results)),
                ("calls", Js::Arr(client.transport.calls)),
                ("cookieProof", Js::Arr(client.transport.cookie_proof)),
                ("authenticatedBeforeClose", before.into()),
                ("authenticatedAfterClose", client.authenticated.into()),
            ]))
        }
        "operation_admission" => {
            let pairs = |key: &str| {
                v.get(key)
                    .array()
                    .iter()
                    .map(|p| (string(&p.array()[0]), string(&p.array()[1])))
                    .collect::<Vec<_>>()
            };
            let operation = http::admit_operation(
                &string(v.get("origin")),
                &string(v.get("path")),
                &string(v.get("method")),
                &pairs("query"),
                &pairs("body"),
            );
            operation.ok_or_else(|| Error::error("s09_recorded_operation_denied"))
        }
        _ => Err(Error::error("offline_operation_unselected")),
    }
}
fn main() {
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let result = line
            .map_err(|_| Error::error("offline_input_read_failed"))
            .and_then(|s| parse(&s))
            .and_then(|v| run(&v));
        if writeln!(out, "{}", json(&observe(result))).is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
    }
}
