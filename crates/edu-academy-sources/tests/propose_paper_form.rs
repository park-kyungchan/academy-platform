//! Invented pages only: pin membership, offline binary proposal and output privacy.
use edu_academy_core::templates::from_json;
use edu_academy_sources::{
    dom::Document, student_course::PAPER_PATH, student_metadata::PaperFormPin,
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
};

const NAMES: [&str; 10] = [
    "clg_no",
    "cls_no",
    "p_pageno",
    "check_da_test",
    "check_fa_test",
    "check_na_test",
    "stu_name",
    "chioce_cls",
    "sort_date1",
    "sort_date2",
];
const CANARIES: [&str; 4] = ["VALUE_CANARY", "LABEL_CANARY", "NAME_CANARY", "KEY_CANARY"];
fn action() -> String {
    format!("{PAPER_PATH}?p_process=UserBySearchTestResult&ass_no=1001")
}
fn page() -> String {
    let mut s = String::from("<form id=KEY_CANARY name=search method=post>");
    for name in NAMES {
        match name {
            "chioce_cls" => s.push_str("<select name=chioce_cls><option value=VALUE_CANARY>LABEL_CANARY</option><option value=other>Other invented label</option></select>"),
            "check_da_test" | "check_fa_test" | "check_na_test" => s.push_str(&format!("<input name={name} type=checkbox value=VALUE_CANARY checked>")),
            _ => s.push_str(&format!("<input name={name} type=text value=VALUE_CANARY>")),
        }
    }
    s + "<input name=NAME_CANARY type=checkbox value=VALUE_CANARY checked></form>"
}
fn candidate() -> Value {
    let mut fields: Vec<Value> = NAMES.iter().map(|name| {
        let checkbox = name.starts_with("check_");
        let select = *name == "chioce_cls";
        json!({"name":name,"tag":if select {"select"} else {"input"},"type":if select {None} else {Some(if checkbox {"checkbox"} else {"text"})},"value":if checkbox || select || *name == "clg_no" {Some("VALUE_CANARY")} else {None},"checked":checkbox.then_some(true),"options":if select {vec![json!({"value":"VALUE_CANARY","label":"LABEL_CANARY"}),json!({"value":"other","label":"Other invented label"})]} else {vec![]}})
    }).collect();
    fields.push(json!({"name":"NAME_CANARY","tag":"input","type":"checkbox","value":"VALUE_CANARY","checked":true,"options":[]}));
    json!({"schemaVersion":"academy.paper-search-form/1","action":null,"method":"post","fields":fields})
}
fn parse(v: &Value) -> edu_academy_sources::data::Result<PaperFormPin> {
    PaperFormPin::parse(&from_json(v.clone()))
}
fn verifies(v: &Value, p: &str) -> bool {
    parse(v).unwrap().verify(&Document::parse(p)).is_ok()
}
#[test]
fn action_four_pairs_and_exact_membership() {
    let absent = page();
    let present = absent.replace(
        "method=post",
        &format!("method=post action=\"{}\"", action()),
    );
    let null = candidate();
    let mut string = null.clone();
    string["action"] = action().into();
    assert!(verifies(&null, &absent));
    assert!(verifies(&string, &present));
    assert!(!verifies(&null, &present));
    assert!(!verifies(&string, &absent));
    let mut ten = null.clone();
    ten["fields"].as_array_mut().unwrap().pop();
    assert!(!verifies(&ten, &absent)); // unlisted eleventh control
    assert!(!verifies(
        &null,
        &absent.replace(
            "<input name=NAME_CANARY type=checkbox value=VALUE_CANARY checked>",
            ""
        )
    ));
    let mut duplicate = null.clone();
    duplicate["fields"]
        .as_array_mut()
        .unwrap()
        .push(null["fields"][0].clone());
    assert_eq!(parse(&duplicate).err().unwrap().message, "contract_drift");
    let mut many = null.clone();
    for n in 11..64 {
        let mut f = null["fields"][0].clone();
        f["name"] = format!("invented_{n}").into();
        many["fields"].as_array_mut().unwrap().push(f);
    }
    assert!(parse(&many).is_ok());
    let mut extra = null["fields"][0].clone();
    extra["name"] = "invented_65".into();
    many["fields"].as_array_mut().unwrap().push(extra);
    assert_eq!(many["fields"].as_array().unwrap().len(), 65);
    assert!(parse(&many).is_err());
    let mut missing_required = null.clone();
    missing_required["fields"][0]["name"] = "invented_replacement".into();
    assert!(parse(&missing_required).is_err());
}
struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let path = PathBuf::from(std::env::var("S09_NATIVE_FIXTURE_ROOT").unwrap())
            .join(format!("form-{label}-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn run(&self, body: &Path, out: &Path) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_academy-capture"))
            .args(["propose-paper-form", "--body"])
            .arg(body)
            .arg("--out")
            .arg(out)
            .output()
            .unwrap();
        for bytes in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(bytes);
            for canary in CANARIES {
                assert!(!text.contains(canary), "canary output");
            }
            for name in NAMES {
                assert!(!text.contains(name), "page name output");
            }
        }
        assert!(output.stderr.is_empty());
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn binary_candidate_verifies_and_each_single_drift_refuses() {
    let f = Fixture::new("success");
    let body = f.0.join("body");
    let out = f.0.join("pin");
    fs::write(&body, page()).unwrap();
    let result = f.run(&body, &out);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        json!({"status":"propose_paper_form_ok","fields":11,"options":2,"checked_boxes":4,"action_present":false})
    );
    assert_eq!(
        fs::metadata(&out).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let v: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    assert_eq!(v, candidate());
    assert!(verifies(&v, &page()));
    for p in [
        page().replace("<option value=other>Other invented label</option>", ""),
        page().replace("LABEL_CANARY", "Changed invented label"),
        page().replace("</form>", "<input name=NEW_NAME_CANARY></form>"),
        page().replace(
            "method=post",
            &format!("method=post action=\"{}\"", action()),
        ),
        page().replace(
            "name=NAME_CANARY type=checkbox value=VALUE_CANARY checked",
            "name=NAME_CANARY type=checkbox value=VALUE_CANARY",
        ),
    ] {
        assert!(!verifies(&v, &p));
    }
    // A present, allowed string action also proposes and verifies.
    fs::write(
        &body,
        page().replace(
            "method=post",
            &format!("method=post action=\"{}\"", action()),
        ),
    )
    .unwrap();
    let out2 = f.0.join("pin2");
    let result = f.run(&body, &out2);
    assert!(result.status.success());
    let v: Value = serde_json::from_slice(&fs::read(out2).unwrap()).unwrap();
    assert!(verifies(&v, &fs::read_to_string(body).unwrap()));
}
#[test]
fn binary_refusals_do_not_publish_or_echo() {
    let f = Fixture::new("refusals");
    let body = f.0.join("VALUE_CANARY");
    let out = f.0.join("NAME_CANARY");
    fs::write(&body, page()).unwrap();
    fs::write(&out, "retained original").unwrap();
    let result = f.run(&body, &out);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read(&out).unwrap(), b"retained original");
    fs::remove_file(&out).unwrap();
    for input in [f.0.join("missing_LABEL_CANARY"), f.0.clone()] {
        assert_eq!(f.run(&input, &out).status.code(), Some(2));
        assert!(!out.exists());
    }
    for p in [
        "<div name=NAME_CANARY>LABEL_CANARY VALUE_CANARY KEY_CANARY</div>".into(),
        page().replace("method=post", "method=get"),
        page().replace("name=clg_no", "name=NAME_CANARY"),
        page().replace(
            "method=post",
            "method=post action='https://invalid.example/VALUE_CANARY'",
        ),
        page().replace("name=clg_no", "disabled name=clg_no"),
        page().replace("name=chioce_cls", "disabled name=chioce_cls"),
        page().replace("LABEL_CANARY", &"LABEL_CANARY".repeat(25)),
    ] {
        fs::write(&body, p).unwrap();
        let result = f.run(&body, &out);
        assert_eq!(result.status.code(), Some(2));
        assert!(!out.exists());
    }
    fs::write(&body, [0xff]).unwrap();
    assert_eq!(f.run(&body, &out).status.code(), Some(2));
    assert!(!out.exists());
}
