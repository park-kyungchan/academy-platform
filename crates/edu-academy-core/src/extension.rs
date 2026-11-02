//! Extended comparison boundary. Receives only the actual request and explicit ports.
use crate::{admission, cism, decision, frozen::FrozenValue, js::*, learning, schema, wire::Wire};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub op: String,
    pub wire: Wire,
    pub ports: Option<crate::wire::TestPorts>,
}
fn observed(v: &Js) -> Js {
    match v {
        Js::Undefined | Js::Hole => obj(&[("$observed", "undefined".into())]),
        Js::Num(n) if !n.is_finite() || *n == 0.0 && n.is_sign_negative() => obj(&[
            (
                "$observed",
                if n.is_nan() {
                    "NaN"
                } else if *n == f64::INFINITY {
                    "Infinity"
                } else if *n == f64::NEG_INFINITY {
                    "-Infinity"
                } else {
                    "0"
                }
                .into(),
            ),
            ("negativeZero", (*n == 0.0 && n.is_sign_negative()).into()),
        ]),
        Js::Date(s) => obj(&[("$observed", "Date".into()), ("iso", Js::Str(s.clone()))]),
        Js::Arr(a) => Js::Arr(a.iter().map(observed).collect()),
        Js::Obj(p) | Js::NullPrototype(p) => {
            Js::Obj(p.iter().map(|(k, v)| (k.clone(), observed(v))).collect())
        }
        _ => v.clone(),
    }
}
fn is_object(v: &Js) -> bool {
    matches!(v, Js::Obj(_) | Js::Arr(_) | Js::NullPrototype(_))
}
fn strict_same(a: &Js, b: &Js) -> bool {
    !is_object(a) && !is_object(b) && a == b
}
fn observe_frozen(result: Js, args: &[Js]) -> Js {
    let frozen = FrozenValue::from_validated(result);
    let result = frozen.value();
    let undef = Js::Undefined;
    let first = args.first().unwrap_or(&undef);
    let second = args.get(1).unwrap_or(&undef);
    let mut mutation = vec![];
    for (label, target) in [
        ("root", result),
        ("value", result.get("value")),
        ("refs", result.get("evidenceRefs")),
        ("a", result.get("a")),
    ] {
        if is_object(target) {
            let failure = frozen
                .try_define("__probe", Js::Num(1.0))
                .expect_err("immutable API refuses writes");
            mutation.push(obj(&[
                ("label", label.into()),
                ("status", "throw".into()),
                ("error", failure.value()),
            ]));
        }
    }
    let own_keys = match result {
        Js::Arr(a) => (0..a.len()).map(|i| i.to_string().into()).collect(),
        _ => enumerate(result.props())
            .iter()
            .map(|(k, _)| Js::Str(k.clone()))
            .collect(),
    };
    obj(&[
        ("value", observed(result)),
        ("rootSame", strict_same(result, first).into()),
        (
            "valueSame",
            if is_object(result) && result.has("value") {
                strict_same(result.get("value"), first).into()
            } else {
                Js::Null
            },
        ),
        (
            "refsSame",
            strict_same(result.get("evidenceRefs"), second).into(),
        ),
        (
            "sharedAfter",
            if result.get("a").truthy() && result.get("b").truthy() {
                strict_same(result.get("a"), result.get("b")).into()
            } else {
                Js::Null
            },
        ),
        ("allFrozen", frozen.all_frozen().into()),
        (
            "prototype",
            if matches!(result, Js::Obj(_)) {
                "Object".into()
            } else if matches!(result, Js::Arr(_)) {
                "other".into()
            } else {
                Js::Null
            },
        ),
        ("ownKeys", Js::Arr(own_keys)),
        ("mutation", Js::Arr(mutation)),
    ])
}
struct LocalOwner<'a> {
    current: admission::SharedRequest,
    events: &'a mut Vec<Js>,
    consume_result: bool,
    throw_read: bool,
    throw_consume: bool,
}
impl admission::Owner for LocalOwner<'_> {
    fn read_current_request(&mut self) -> Result<admission::SharedRequest> {
        self.events
            .push(obj(&[("event", "local_current_request".into())]));
        if self.throw_read {
            return Err(Fault::error("synthetic_read_failure"));
        }
        Ok(self.current.clone())
    }
    fn consume_once(&mut self, key: &str, record: &admission::OwnerRecord) -> Result<bool> {
        self.events.push(obj(&[
            ("event", "local_consume_once".into()),
            ("key", key.into()),
            ("record", record.snapshot()),
            ("frozen", record.is_shallow_frozen().into()),
            ("requestFrozen", false.into()),
            (
                "sameRequest",
                Rc::ptr_eq(record.request(), &self.current).into(),
            ),
        ]));
        if self.throw_consume {
            return Err(Fault::error("synthetic_consume_failure"));
        }
        Ok(self.consume_result)
    }
}
fn assign(target: &mut Js, changes: &Js) {
    for (k, v) in changes.props() {
        target.set(&String::from_utf16_lossy(k), v.clone());
    }
}
fn outcome(r: Result<Js>) -> Js {
    match r {
        Ok(v) => obj(&[("kind", "return".into()), ("value", observed(&v))]),
        Err(e) => obj(&[("kind", "throw".into()), ("error", e.value())]),
    }
}
fn run(
    input: &Js,
    ports: Option<&crate::wire::TestPorts>,
    events: &mut Vec<Js>,
    trace: &mut decision::ReviewTrace,
) -> Result<Js> {
    let op = input.get("op").code_units();
    match op.as_str() {
        "exports" => Ok(crate::evidence::convert(
            serde_json::from_str(include_str!("source-exports.json"))
                .expect("source compatibility inventory"),
        )),
        "learning" => {
            let name = input.get("fn").code_units();
            trace.calls.push(format!("learning.{name}").into());
            let args = input.get("args").array();
            let value = learning::call(&name, args)?;
            if [
                "immutableCopy",
                "known",
                "unknown",
                "contextFromLessonKey",
                "resolveProblemPath",
            ]
            .contains(&name.as_str())
            {
                Ok(observe_frozen(value, args))
            } else {
                Ok(value)
            }
        }
        "json" | "direct" => {
            let family = if input.get("family").is("learning") {
                Family::Learning
            } else if input.get("family").is("admission") {
                Family::Admission
            } else {
                Family::Cism
            };
            let name = if op == "direct" {
                input.get("fn").code_units()
            } else {
                "canonicalJson/hash".into()
            };
            trace
                .calls
                .push(format!("{}.{name}", input.get("family").code_units()).into());
            let value = input.get("value");
            if op == "direct" {
                if name == "canonicalJson" {
                    return Ok(encode(value, family, events)?
                        .map(Js::from)
                        .unwrap_or(Js::Undefined));
                }
                return encode(value,family,events)?.map(|b|sha(&b).into()).ok_or_else(||Fault::type_error("The \"data\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received undefined"));
            }
            let bytes = encode(value, family, events)?;
            let hashed=encode(value,family,events)?.ok_or_else(||Fault::type_error("The \"data\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received undefined"))?;
            let mut v = obj(&[("sha256", sha(&hashed).into())]);
            v.set("bytes", bytes.map(Js::from).unwrap_or(Js::Undefined));
            Ok(v)
        }
        "schema" => {
            let name = input.get("schema").code_units();
            trace
                .calls
                .push(format!("{}.{name}.parse", input.get("module").code_units()).into());
            let parsed = schema::validate(&name, input.get("value"))?;
            let mut r = obj(&[("parsed", parsed)]);
            if input.has("instant") {
                let ms = crate::instant::epoch_ms(
                    input.get("instant"),
                    crate::instant::Grammar::DailyRecord,
                );
                r.set(
                    "dateParse",
                    ms.map(|ms| Js::Num(ms as f64)).unwrap_or(Js::Num(f64::NAN)),
                );
                r.set("dateParseFinite", ms.is_some().into());
            }
            Ok(r)
        }
        "cism_write" => {
            trace.calls.push("cism.cismWriteParameters".into());
            cism::write_parameters(input.get("report"), input.get("field"), input.get("after"))
        }
        "cism_date" => {
            trace.calls.push("cism.cismDate".into());
            let value = input.get("value");
            if !value.string() || crate::leaf::cism_week(&value.code_units()).is_err() {
                return Err(Fault::error("cism_date_rejected"));
            }
            Ok(Js::Date(text(&format!(
                "{}T00:00:00.000Z",
                value.code_units()
            ))))
        }
        "parser" => {
            trace.calls.push("cism.parseCismReport".into());
            cism::parse_report(
                &input.get("html").code_units(),
                &input.get("date").code_units(),
                input.get("teacher"),
                &input.get("now").code_units(),
                events,
            )
        }
        "admission" => {
            trace
                .calls
                .push("admission.createDailyRecordSaveAdmission".into());
            let launch = Rc::new(RefCell::new(input.get("admission").clone()));
            let mut approval = admission::Admission::new(launch.clone())?;
            let mut owner = LocalOwner {
                current: Rc::new(RefCell::new(input.get("current").clone())),
                events,
                consume_result: true,
                throw_read: false,
                throw_consume: false,
            };
            let mut results = vec![];
            for step in input.get("steps").array() {
                if step.has("current") {
                    owner.current = Rc::new(RefCell::new(step.get("current").clone()));
                }
                if step.has("consumeResult") {
                    owner.consume_result = step.get("consumeResult").truthy();
                }
                owner.throw_read = step.get("throwRead").truthy();
                owner.throw_consume = step.get("throwConsume").truthy();
                if step.has("mutateAdmission") {
                    assign(&mut launch.borrow_mut(), step.get("mutateAdmission"));
                }
                if step.has("mutateEntry") {
                    let mut admission = launch.borrow_mut();
                    let mut entries = admission.get("admitted").array().to_vec();
                    if let Some(entry) = entries.first_mut() {
                        assign(entry, step.get("mutateEntry"));
                    }
                    admission.set("admitted", Js::Arr(entries));
                }
                trace.calls.push("admission.consumeApproval".into());
                results.push(outcome(
                    approval.consume_approval(step.get("action"), &mut owner),
                ));
            }
            Ok(obj(&[
                ("admissionDigest", approval.admission_digest().into()),
                ("results", Js::Arr(results)),
            ]))
        }
        "review" => {
            trace.calls.push("daily.DailyRecordService.intake".into());
            decision::review_extended(
                input,
                ports.ok_or_else(|| Fault::admission("explicit_test_ports_required"))?,
                events,
                trace,
            )
        }
        "live_evidence" => {
            trace.calls.push("admission.liveSaveRequestEvidence".into());
            let mut results = vec![];
            for step in input.get("steps").array() {
                trace
                    .calls
                    .push("admission.liveSaveRequestEvidence.read".into());
                let result = if step.get("missing").truthy()
                    || step.get("bytes").code_units().len() > 16384
                {
                    Err(Fault::error("dailyrecord_save_current_request_unavailable"))
                } else {
                    crate::evidence::parse_bytes(&step.get("bytes").code_units())
                };
                results.push(outcome(result));
            }
            Ok(Js::Arr(results))
        }
        _ => Err(Fault::admission("unknown_extension_operation")),
    }
}
pub fn execute(request: Request) -> std::result::Result<String, &'static str> {
    if request.op != "semantic_v4" {
        return Err("s08_protocol_input_rejected");
    }
    let input = request.wire.decode()?;
    let mut events = vec![];
    let mut trace = decision::ReviewTrace {
        calls: vec![],
        uuids: vec![],
    };
    let result = run(&input, request.ports.as_ref(), &mut events, &mut trace);
    if result.as_ref().is_err_and(|e| e.name == "AdapterAdmission") {
        return Err("s08_semantic_admission_required");
    }
    let frame = obj(&[
        ("outcome", outcome(result)),
        ("events", Js::Arr(events)),
        (
            "effects",
            obj(&[
                ("deniedTransportAttempts", 0usize.into()),
                ("forbiddenFakeDispatches", 0usize.into()),
            ]),
        ),
        (
            "metadata",
            obj(&[
                ("sourceCalls", Js::Arr(trace.calls)),
                ("uuidValues", Js::Arr(trace.uuids)),
            ]),
        ),
    ]);
    stringify(&frame)
        .map(|s| s + "\n")
        .map_err(|_| "s08_semantic_output_rejected")
}
