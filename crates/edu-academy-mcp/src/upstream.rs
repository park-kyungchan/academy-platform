//! A single native child's legacy MCP conversation. No business-result fixtures.
use crate::{
    Error, Js, Result, encode,
    framing::SDK_CHILD_RECEIVE_BUFFER_BYTES,
    obj, parse,
    process::{Context, OwnedChild},
    protocol::SUPPORTED_VERSIONS,
};
pub struct Client {
    pending: Vec<u8>,
    next: u32,
    pub failure_metadata: Option<Js>,
}
impl Default for Client {
    fn default() -> Self {
        Self {
            pending: vec![],
            next: 0,
            failure_metadata: None,
        }
    }
}
impl Client {
    fn frame(&mut self, child: &mut OwnedChild, context: &mut Context<'_>) -> Result<Js> {
        loop {
            if let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
                let line = self.pending.drain(..=end).collect::<Vec<_>>();
                // The original child SDK ReadBuffer discards SyntaxError lines.
                if let Ok(message) = parse(String::from_utf8_lossy(&line).as_bytes()) {
                    return Ok(message);
                }
                continue;
            }
            let mut chunk = [0u8; 65536];
            if let Some(n) = child.read(&mut chunk, context)? {
                if n > SDK_CHILD_RECEIVE_BUFFER_BYTES.saturating_sub(self.pending.len()) {
                    return Err(Error::new("upstream_receive_buffer_exceeded"));
                }
                self.pending.extend_from_slice(&chunk[..n]);
            } else {
                context.pause()?;
            }
        }
    }
    fn send(
        &mut self,
        child: &mut OwnedChild,
        context: &mut Context<'_>,
        message: &Js,
    ) -> Result<()> {
        let mut bytes = encode(message)?;
        bytes.push(b'\n');
        child.write(&bytes, context)
    }
    pub fn request(
        &mut self,
        child: &mut OwnedChild,
        context: &mut Context<'_>,
        method: &str,
        params: Js,
        before_send: impl FnOnce(),
    ) -> Result<Js> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| Error::new("upstream_request_id_exhausted"))?;
        let id = Js::Num(f64::from(self.next));
        let message = obj(&[
            ("jsonrpc", "2.0".into()),
            ("id", id.clone()),
            ("method", method.into()),
            ("params", params),
        ]);
        // Serialization/graph/control checks precede the possible-write boundary.
        let mut bytes = encode(&message)?;
        bytes.push(b'\n');
        context.verify()?;
        context.check()?;
        before_send();
        child.write(&bytes, context)?;
        loop {
            let frame = self.frame(child, context)?;
            if !frame.get("jsonrpc").is("2.0") {
                return Err(Error::new("upstream_envelope_invalid"));
            }
            if frame.has("method") {
                if frame.has("id") {
                    return Err(Error::new("upstream_request_unqualified"));
                }
                continue; // No notification response; no capability for server requests.
            }
            if frame.get("id") != &id {
                return Err(Error::new("upstream_response_id_mismatch"));
            }
            if frame.has("error") {
                return Err(Error::new("upstream_rpc_error"));
            }
            if !matches!(frame.get("result"), Js::Obj(_)) {
                return Err(Error::new("upstream_result_invalid"));
            }
            return Ok(frame.get("result").clone());
        }
    }
    pub fn call(
        &mut self,
        child: &mut OwnedChild,
        context: &mut Context<'_>,
        name: &str,
        args: Js,
        before_send: impl FnOnce(),
    ) -> Result<Js> {
        let result = self.request(
            child,
            context,
            "tools/call",
            obj(&[("name", name.into()), ("arguments", args)]),
            before_send,
        )?;
        if !matches!(result.get("content"), Js::Arr(_)) {
            return Err(Error::new("upstream_tool_result_invalid"));
        }
        Ok(result)
    }
    pub fn connect(
        &mut self,
        child: &mut OwnedChild,
        context: &mut Context<'_>,
        endpoint: &str,
    ) -> Result<()> {
        let init = self.request(
            child,
            context,
            "initialize",
            obj(&[
                ("protocolVersion", SUPPORTED_VERSIONS[0].into()),
                ("capabilities", obj(&[])),
                (
                    "clientInfo",
                    obj(&[
                        ("name", "muse-academy-bounded-adapter".into()),
                        ("version", "1.0.0".into()),
                    ]),
                ),
            ]),
            || {},
        )?;
        if !SUPPORTED_VERSIONS
            .iter()
            .any(|v| init.get("protocolVersion").is(v))
            || !matches!(init.get("capabilities").get("tools"), Js::Obj(_))
            || !init.get("serverInfo").get("name").string()
            || !init.get("serverInfo").get("version").string()
        {
            return Err(Error::new("upstream_initialize_unqualified"));
        }
        self.send(
            child,
            context,
            &obj(&[
                ("jsonrpc", "2.0".into()),
                ("method", "notifications/initialized".into()),
            ]),
        )?;
        let list = self.request(child, context, "tools/list", obj(&[]), || {})?;
        if !matches!(list.get("tools"), Js::Arr(_)) {
            return Err(Error::new("upstream_tool_inventory_mismatch"));
        }
        if let Err(error) = context
            .graph
            .registry
            .verify_upstream(endpoint, list.get("tools").array())
        {
            let missing = context
                .graph
                .registry
                .entries()
                .iter()
                .filter(|entry| {
                    entry.endpoint == endpoint
                        && !matches!(
                            entry.upstream_name.as_str(),
                            "academy_dailyrecord_get" | "academy_dailytest_history_with_headers"
                        )
                        && !list
                            .get("tools")
                            .array()
                            .iter()
                            .any(|tool| tool.get("name").is(&entry.upstream_name))
                })
                .map(|entry| entry.upstream_name.clone())
                .collect::<std::collections::BTreeSet<_>>();
            self.failure_metadata = Some(obj(&[(
                "inventory",
                obj(&[
                    ("endpoint", endpoint.into()),
                    (
                        "missingTools",
                        Js::Arr(missing.into_iter().map(Into::into).collect()),
                    ),
                ]),
            )]));
            return Err(error);
        }
        let identity = self.call(child, context, "academy_endpoint_identity", obj(&[]), || {})?;
        if identity.get("isError").truthy()
            || encode(identity.get("structuredContent"))?
                != encode(&context.graph.endpoint_identity(endpoint)?)?
        {
            return Err(Error::new("release_endpoint_identity_mismatch"));
        }
        if endpoint == "dailyrecord" {
            let read = self.call(
                child,
                context,
                "academy_runtime_read_admission",
                obj(&[]),
                || {},
            )?;
            if read.get("isError").truthy() {
                return Err(Error::new("workflow_source_port_unqualified"));
            }
            let admission = read_admission(&context.graph.admission()?);
            let sha = crate::digest(&encode(&admission)?);
            if !read.get("structuredContent").get("status").is("configured")
                || !read
                    .get("structuredContent")
                    .get("admissionSha256")
                    .is(&sha)
            {
                return Err(Error::new("release_upstream_admission_mismatch"));
            }
        }
        Ok(())
    }
}
pub fn read_admission(owner: &Js) -> Js {
    let mut value = obj(&[
        "sourceNamespace",
        "teacherId",
        "allowedFrom",
        "allowedThrough",
    ]
    .map(|key| (key, owner.get(key).clone())));
    if owner.get("schemaVersion").is("academy.owner-admission/2") {
        value.set(
            "studentScope",
            if owner.get("studentScope").absent() {
                "configured".into()
            } else {
                owner.get("studentScope").clone()
            },
        );
    }
    value
}
