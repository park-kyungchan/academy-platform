//! Offline candidate proposal. Only the explicit body and new output are opened.
use crate::{
    data::{Error, Result},
    dom::{self, Document},
    js_text::decode_bounded,
    student_metadata::PaperFormPin,
};
use edu_academy_core::templates::from_json;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
};

fn refused(code: &str) -> Error {
    Error::refusal(code)
}

pub fn run(args: impl Iterator<Item = String>) -> Result<Value> {
    let mut args = args;
    let mut paths = HashMap::new();
    while let Some(arg) = args.next() {
        if !["--body", "--out"].contains(&arg.as_str())
            || paths
                .insert(
                    arg,
                    args.next().ok_or_else(|| refused("propose_args_refused"))?,
                )
                .is_some()
        {
            return Err(refused("propose_args_refused"));
        }
    }
    let body = paths
        .get("--body")
        .ok_or_else(|| refused("propose_args_refused"))?;
    let out = paths
        .get("--out")
        .ok_or_else(|| refused("propose_args_refused"))?;
    let input = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(body)
        .map_err(|_| refused("propose_body_refused"))?;
    if !input
        .metadata()
        .map_err(|_| refused("propose_body_refused"))?
        .is_file()
    {
        return Err(refused("propose_body_refused"));
    }
    let mut bytes = Vec::new();
    input
        .take(8_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| refused("propose_body_refused"))?;
    let source = decode_bounded(&bytes, 8_000_000, 2_000_000)
        .map_err(|_| refused("propose_body_encoding_or_bound_refused"))?;
    let d = Document::parse(&source);
    let forms = d.select("form[name=search]");
    if forms.len() != 1 {
        return Err(refused("propose_form_count_refused"));
    }
    let form = forms[0];
    let nodes = dom::select(form, "[name]");
    let fields: Vec<Value> = nodes
        .iter()
        .map(|node| {
            let checkbox = node.value().attr("type") == Some("checkbox");
            let pinned_value = node.value().attr("name") == Some("clg_no")
                || checkbox
                || node.value().name() == "select";
            json!({
                "name": node.value().attr("name"),
                "tag": node.value().name(),
                "type": node.value().attr("type"),
                "value": pinned_value.then(|| dom::value(*node)),
                "checked": checkbox.then(|| node.value().attr("checked").is_some()),
                "options": dom::select(*node, "option").iter().map(|option| json!({
                    "value": dom::value(*option), "label": dom::content(*option)
                })).collect::<Vec<_>>()
            })
        })
        .collect();
    let candidate = json!({
        "schemaVersion": "academy.paper-search-form/1",
        "action": form.value().attr("action"),
        "method": form.value().attr("method").map(str::to_ascii_lowercase),
        "fields": fields
    });
    let pin = PaperFormPin::parse(&from_json(candidate.clone()))
        .map_err(|_| refused("propose_pin_parse_refused"))?;
    pin.verify(&d)
        .map_err(|_| refused("propose_pin_verify_refused"))?;
    let payload =
        serde_json::to_vec_pretty(&candidate).map_err(|_| refused("propose_serialize_refused"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(out)
        .map_err(|_| refused("propose_output_refused"))?;
    if file
        .write_all(&payload)
        .and_then(|_| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(out);
        return Err(refused("propose_output_write_refused"));
    }
    Ok(json!({
        "status": "propose_paper_form_ok",
        "fields": fields.len(),
        "options": fields.iter().map(|f| f["options"].as_array().expect("array").len()).sum::<usize>(),
        "checked_boxes": fields.iter().filter(|f| f["checked"] == true).count(),
        "action_present": form.value().attr("action").is_some()
    }))
}
