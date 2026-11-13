//! Invented memo answers through both public reviews and the durable save route.
use edu_academy_core::{
    decision,
    js::{Js, Result, obj},
    schema, semantic,
    wire::TestPorts,
    workflow::{
        Runtime,
        approval::Approval,
        codec,
        review::{self, Policy},
        service::{Backend, Service, Source},
        store::Store,
    },
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const NOW: &str = "2040-01-04T12:00:00.000Z";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Ports;
impl Runtime for Ports {
    fn now(&self) -> String {
        NOW.into()
    }
    fn epoch_ms(&self) -> u64 {
        0
    }
    fn uuid(&self) -> Result<String> {
        Ok(format!("memo-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
    }
}
impl Policy for Ports {
    fn parent_pins(&self) -> Js {
        json(
            serde_json::json!({"sourceRef":"invented-memo-policy","policySha256":"3".repeat(64),"safeguardSourceSha256":"4".repeat(64)}),
        )
    }
    fn learning_pins(&self) -> Js {
        json(serde_json::json!({"bundleSha256":"5".repeat(64),
            "configSha256":"6".repeat(64),"catalogSha256":"7".repeat(64)}))
    }
    fn check_lines(&self, _: &Js) -> bool {
        true
    }
    fn excluded_parent_language(&self, _: &Js) -> bool {
        false
    }
    fn scope_qualified(&self, _: &Js) -> bool {
        true
    }
    fn learning_review(&self, _: &Js, _: &Js, _: &Js) -> Vec<Js> {
        vec![]
    }
}
fn json(v: serde_json::Value) -> Js {
    codec::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn fixture(kind: &str, decision: &str, value: &str, missing: bool) -> (Js, Js) {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/workflow-store-review-fixes-v2/CASES.json"
    ))
    .unwrap();
    let mut context = cases[0]["input"]["context"].clone();
    let field = if missing { "attendance" } else { "lessonMemo" };
    let request = context["currentRequestRef"].clone();
    let mut answer = serde_json::json!({"kind":kind});
    if kind != "not_supplied" {
        answer["requestRef"] = request.clone();
    }
    if kind == "content" {
        answer["text"] = "- Invented new memo".into();
    }
    context["memoAnswer"] = answer;
    context["selectedFields"] = serde_json::json!(["lessonMemo", "attendance"]);
    context["facts"] = serde_json::json!([{"ref":"invented-fact","field":field,"text":"Invented teacher statement",
        "origin":"user_supplied","explicitValue":value,"requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[]}]);
    let input = json(
        serde_json::json!({"context":context,"observationHandle":"invented-held", "proposals":[{
        "field":field,"value":value,"factRefs":["invented-fact"],"decision":{"kind":decision,"requestRef":request}}]}),
    );
    let mut observation = cases[0]["read"]["observation"].clone();
    observation["fields"]["lessonMemo"]["value"] = "- Invented existing memo".into();
    (input, json(observation))
}
fn both(input: &Js, observation: &Js) -> [Js; 2] {
    both_with_events(input, observation).0
}
fn both_with_events(input: &Js, observation: &Js) -> ([Js; 2], Vec<Js>) {
    let ports = TestPorts {
        uuid: "00000000-0000-4000-8000-000000000001".into(),
        now: NOW.into(),
    };
    let mut direct = input.clone();
    direct.set("observation", observation.clone());
    direct.set("learning", Ports.learning_pins());
    let mut events = vec![];
    let decision = decision::review(&direct, &ports, &mut events).unwrap();
    assert!(!events.iter().any(|e| e.get("event").is("fake_dispatch")));
    (
        [
            decision.get("review").clone(),
            review::review(input, observation, observation, &Ports, &Ports).unwrap(),
        ],
        events,
    )
}
struct Read(Js);
impl Source for Read {
    fn read(&mut self, _: &Js, _: &Js, _: &Js) -> Result<Js> {
        Ok(obj(&[
            ("status", "observed".into()),
            ("observation", self.0.clone()),
        ]))
    }
}
struct Write(Vec<Js>);
impl Backend for Write {
    fn configured(&self) -> bool {
        true
    }
    fn save(
        &mut self,
        scope: &Js,
        patches: &Js,
        approval: &mut dyn Approval,
        _: &Js,
    ) -> Result<Js> {
        for p in patches.array() {
            let mut action = p.clone();
            action.set("after", p.get("value").clone());
            action.set("attemptId", "invented-memo-attempt".into());
            action.set(
                "target",
                obj(&[
                    ("teacherKey", scope.get("teacherId").clone()),
                    ("studentKey", scope.get("studentId").clone()),
                    ("date", scope.get("lessonDate").clone()),
                ]),
            );
            assert!(approval.consume(&action)?.get("status").is("verified"));
            self.0.push(p.clone());
        }
        Ok(Js::Arr(vec![])) // No receipt or persistence claim is invented.
    }
}
fn save(mut input: Js, mut observation: Js) -> Vec<Js> {
    let root =
        PathBuf::from(std::env::var("S11_INTAKE_TEST_ROOT").unwrap()).join(Ports.uuid().unwrap());
    edu_academy_core::workflow::store::mkdir(&root).unwrap();
    let store = Store::open(&root.join("core"), &root).unwrap();
    let mut fields = observation.get("fields").clone();
    for proposal in input.get("proposals").array() {
        let field = proposal.get("field").code_units();
        let mut value = fields.get(&field).clone();
        value.set("effectTarget", format!("invented-target-{field}").into());
        value.set(
            "effectRecordKey",
            observation.get("target").get("recordId").clone(),
        );
        fields.set(&field, value);
    }
    observation.set("fields", fields);
    let mut read = Read(observation);
    let mut write = Write(vec![]);
    let mut service = Service {
        store: &store,
        source: &mut read,
        backend: &mut write,
        policy: &Ports,
        runtime: &Ports,
    };
    let intake = service
        .intake(&obj(&[("context", input.get("context").clone())]))
        .unwrap();
    input.set("observationHandle", intake.get("observationHandle").clone());
    let reviewed = service.review(&input, true).unwrap();
    input.set("reviewDigest", reviewed.get("reviewDigest").clone());
    let request_ref = input.get("context").get("currentRequestRef").clone();
    let instruction = json(serde_json::json!({"text":"Invented explicit save instruction"}));
    input.set(
        "request",
        obj(&[("requestRef", request_ref), ("instruction", instruction)]),
    );
    let result = service.save(&input).unwrap();
    assert!(
        result.get("status").is("held"),
        "no receipt can qualify a synthetic save"
    );
    std::fs::remove_dir_all(root).unwrap();
    write.0
}
fn held(kind: &str, choice: &str, value: &str, missing: bool, code: &str) {
    let (input, observation) = fixture(kind, choice, value, missing);
    let results = both(&input, &observation);
    let statuses: Vec<_> = results
        .iter()
        .map(|r| r.get("status").code_units())
        .collect();
    assert_eq!(statuses, ["held", "held"], "both review entry points");
    for result in results {
        assert!(
            result.get("patch").array().is_empty(),
            "{code}: empty patch required"
        );
        let row_codes = result
            .get("rows")
            .array()
            .iter()
            .flat_map(|r| r.get("blockers").array());
        let interview_codes = result
            .get("interviews")
            .array()
            .iter()
            .map(|i| i.get("reasonCode"));
        assert!(
            row_codes.chain(interview_codes).any(|b| b.is(code)),
            "{code}: blocker required"
        );
    }
    assert!(
        save(input, observation).is_empty(),
        "{code}: zero dispatch required"
    );
}
#[test]
fn m1_content_cannot_clear_memo() {
    held("content", "clear", "", false, "memo_clear_answer_required");
}
#[test]
fn m2_deletion_requires_clear_decision() {
    held(
        "clear",
        "replace",
        "- Invented new memo",
        false,
        "memo_clear_answer_requires_clear_decision",
    );
}
#[test]
fn m3_deletion_requires_memo_proposal() {
    held(
        "clear",
        "replace",
        "N",
        true,
        "memo_clear_proposal_required",
    );
}
#[test]
fn m4_explicit_clear_sends_empty_to_reviewed_target() {
    let (input, observation) = fixture("clear", "clear", "", false);
    for result in both(&input, &observation) {
        assert!(result.get("status").is("prepared"));
        assert_eq!(result.get("patch").array().len(), 1);
        let patch = &result.get("patch").array()[0];
        assert!(patch.get("after").is(""));
        assert!(
            edu_academy_core::workflow::same(patch.get("target"), observation.get("target"))
                .unwrap()
        );
    }
    let sent = save(input, observation.clone());
    assert_eq!(sent.len(), 1);
    assert!(sent[0].get("field").is("lessonMemo"));
    assert!(sent[0].get("value").is(""));
    assert!(sent[0].get("before").is("- Invented existing memo"));
    assert!(sent[0].get("effectTarget").is("invented-target-lessonMemo"));
    assert_eq!(
        sent[0].get("effectRecordKey"),
        observation.get("target").get("recordId")
    );
}
#[test]
fn m5_old_kinds_and_clear_shape_checks_remain() {
    held(
        "no_addition",
        "replace",
        "- Invented new memo",
        false,
        "no_addition_preserves_existing_memo",
    );
    held(
        "not_supplied",
        "replace",
        "- Invented new memo",
        false,
        "today_memo_interview_required",
    );
    held(
        "clear",
        "clear",
        "- Invented new memo",
        false,
        "clear_requires_empty_value",
    );
    held(
        "clear",
        "replace",
        "",
        false,
        "explicit_clear_decision_required",
    );
    let (input, observation) = fixture("content", "replace", "- Invented new memo", false);
    assert!(
        both(&input, &observation)
            .iter()
            .all(|r| r.get("status").is("prepared"))
    );
}
fn with_progress(kind: &str) -> (Js, Js) {
    let (mut input, mut observation) = fixture(kind, "clear", "", false);
    let mut context = input.get("context").clone();
    context.set(
        "selectedFields",
        json(serde_json::json!(["lessonMemo", "progress"])),
    );
    let mut facts = context.get("facts").array().to_vec();
    facts.push(json(
        serde_json::json!({"ref":"progress-fact","field":"progress",
        "text":"Sent","origin":"user_supplied","explicitValue":"- Sent",
        "requiredFragments":[],"questionCounts":[],"deadlines":[],"stageClaims":[]}),
    ));
    context.set("facts", Js::Arr(facts));
    let mut proposals = input.get("proposals").array().to_vec();
    let mut progress = proposals[0].clone();
    progress.set("field", "progress".into());
    progress.set("value", "- Sent".into());
    progress.set("factRefs", json(serde_json::json!(["progress-fact"])));
    let mut choice = progress.get("decision").clone();
    choice.set("kind", "replace".into());
    progress.set("decision", choice);
    proposals.push(progress);
    input.set("context", context);
    input.set("proposals", Js::Arr(proposals));
    let mut fields = observation.get("fields").clone();
    fields.set(
        "progress",
        json(serde_json::json!({"state":"known","value":"- Old progress","evidenceRefs":["invented-progress"]})),
    );
    observation.set("fields", fields);
    (input, observation)
}
fn row<'a>(result: &'a Js, field: &str) -> &'a Js {
    result
        .get("rows")
        .array()
        .iter()
        .find(|r| r.get("field").is(field))
        .unwrap()
}
fn patch_fields(result: &Js) -> Vec<String> {
    result
        .get("patch")
        .array()
        .iter()
        .map(|p| p.get("field").code_units())
        .collect()
}
#[test]
fn m6_old_kind_with_clearing_memo_keeps_other_field() {
    for (kind, code) in [
        ("no_addition", "no_addition_preserves_existing_memo"),
        ("not_supplied", "today_memo_interview_required"),
    ] {
        let (input, observation) = with_progress(kind);
        let (results, events) = both_with_events(&input, &observation);
        let dispositions: Vec<_> = results
            .iter()
            .map(|r| row(r, "progress").get("disposition").code_units())
            .collect();
        assert_eq!(
            dispositions,
            ["proposed", "proposed"],
            "both old-kind progress rows"
        );
        for result in results {
            assert!(result.get("status").is("held"));
            assert_eq!(patch_fields(&result), ["progress"]);
            assert_eq!(
                row(&result, "lessonMemo").get("blockers").array(),
                &[Js::from(code)]
            );
            let interviews = result.get("interviews").array();
            assert_eq!(interviews.len(), usize::from(kind == "not_supplied"));
            if kind == "not_supplied" {
                assert!(
                    interviews[0]
                        .get("reasonCode")
                        .is("today_student_specific_memo_required")
                );
            }
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| e.get("event").is("recorded_read"))
                .count(),
            2
        );
    }
}
#[test]
fn m7_content_with_clearing_memo_holds_only_the_memo() {
    let (input, observation) = with_progress("content");
    let results = both(&input, &observation);
    let dispositions: Vec<_> = results
        .iter()
        .map(|r| row(r, "progress").get("disposition").code_units())
        .collect();
    assert_eq!(
        dispositions,
        ["proposed", "proposed"],
        "content blocker is memo-local"
    );
    for result in results {
        assert_eq!(
            row(&result, "lessonMemo").get("blockers").array(),
            &[Js::from("memo_clear_answer_required")]
        );
        assert_eq!(patch_fields(&result), ["progress"]);
        assert!(result.get("interviews").array().is_empty());
        assert!(
            row(&result, "lessonMemo").get("holdActions").array()[0]
                .get("action")
                .is("explicit_teacher_decision")
        );
        assert!(
            edu_academy_core::workflow::same(
                result.get("patch").array()[0].get("target"),
                observation.get("target")
            )
            .unwrap()
        );
    }
    // Service persists only prepared reviews; a held mixed review cannot dispatch.
    assert!(save(input.clone(), observation.clone()).is_empty());
    let mut subset = input.clone();
    subset.set(
        "proposals",
        Js::Arr(
            input
                .get("proposals")
                .array()
                .iter()
                .filter(|p| p.get("field").is("progress"))
                .cloned()
                .collect(),
        ),
    );
    let sent = save(subset, observation.clone());
    assert_eq!(sent.len(), 1);
    assert!(sent[0].get("field").is("progress"));
    assert!(sent[0].get("value").is("- Sent"));
    assert!(sent[0].get("effectTarget").is("invented-target-progress"));
    assert_eq!(
        sent[0].get("effectRecordKey"),
        observation.get("target").get("recordId")
    );
}
#[test]
fn m8_deletion_answer_of_another_request_is_refused() {
    let (mut input, observation) = fixture("clear", "clear", "", false);
    let mut context = input.get("context").clone();
    let mut answer = context.get("memoAnswer").clone();
    answer.set("requestRef", "other-req".into());
    context.set("memoAnswer", answer);
    input.set("context", context);
    let results = both(&input, &observation);
    let statuses: Vec<_> = results
        .iter()
        .map(|r| r.get("status").code_units())
        .collect();
    assert_eq!(
        statuses,
        ["held", "held"],
        "deletion answer is request-bound"
    );
    for result in results {
        assert_eq!(
            row(&result, "lessonMemo").get("blockers").array(),
            &[Js::from("memo_clear_answer_current_request_required")]
        );
        assert!(result.get("patch").array().is_empty());
        assert!(result.get("interviews").array().is_empty());
        assert!(
            row(&result, "lessonMemo").get("holdActions").array()[0]
                .get("action")
                .is("explicit_teacher_decision")
        );
    }
    assert!(save(input, observation).is_empty());
}
fn node(value: &Js, nodes: &mut Vec<serde_json::Value>) -> usize {
    let next = match value {
        Js::Null => serde_json::json!({"kind":"null"}),
        Js::Str(units) => serde_json::json!({"kind":"string","units":units}),
        Js::Arr(items) => {
            serde_json::json!({"kind":"array","items":items.iter().map(|v|node(v,nodes)).collect::<Vec<_>>()})
        }
        Js::Obj(entries) => {
            serde_json::json!({"kind":"object","entries":entries.iter().map(|(k,v)|(k,node(v,nodes))).collect::<Vec<_>>()})
        }
        _ => panic!("fixture uses only JSON strings, arrays, objects and null"),
    };
    let index = nodes.len();
    nodes.push(next);
    index
}
#[test]
fn memo_clear_answer_is_explicit_in_schema_and_semantic_admission() {
    let (input, observation) = fixture("clear", "clear", "", false);
    let mut semantic_input = obj(&[
        ("op", "review".into()),
        ("observation", observation),
        ("context", input.get("context").clone()),
        ("proposals", input.get("proposals").clone()),
    ]);
    for (valid, answer_shape) in [
        (true, input.get("context").get("memoAnswer").clone()),
        (false, json(serde_json::json!({"kind":"clear"}))),
        (
            false,
            json(serde_json::json!({"kind":"clear","requestRef":"r","text":"forbidden"})),
        ),
    ] {
        let mut context = input.get("context").clone();
        context.set("memoAnswer", answer_shape);
        assert_eq!(
            schema::validate("dailyRecordContextSchema", &context).is_ok(),
            valid
        );
        semantic_input.set("context", context);
        let mut nodes = vec![];
        let root = node(&semantic_input, &mut nodes);
        let request = serde_json::from_value(serde_json::json!({"op":"semantic_v3",
            "wire":{"root":root,"nodes":nodes}, "ports":{"uuid":"00000000-0000-4000-8000-000000000001","now":NOW}})).unwrap();
        let answer = semantic::execute(request);
        if valid {
            let frame = codec::parse(answer.unwrap().as_bytes()).unwrap();
            assert!(
                frame
                    .get("outcome")
                    .get("value")
                    .get("review")
                    .get("status")
                    .is("prepared")
            );
        } else {
            assert_eq!(answer.unwrap_err(), "s08_semantic_admission_required");
        }
    }
}
