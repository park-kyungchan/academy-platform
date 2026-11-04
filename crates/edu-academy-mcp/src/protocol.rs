//! Original-compatible legacy session lifecycle; native dispatch is an explicit
//! owner-controlled entry, separate from the metadata-only library entry.
use crate::{
    Error, Js, Result,
    audit::{Effect, qualification_gap, tool_result},
    dispatch::{self, Reply},
    obj,
    process::{Control, Limits},
    release::{ConnectionGate, ReleaseGraph},
    string,
};
pub const SUPPORTED_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];
pub struct RpcSession {
    graph: ReleaseGraph,
    gate: ConnectionGate,
    endpoint: Option<String>,
    pub client_info: Option<Js>,
    pub negotiated_version: Option<String>,
    pub initialized: bool,
}
impl RpcSession {
    pub fn new(graph: ReleaseGraph) -> Self {
        Self {
            graph,
            gate: ConnectionGate::default(),
            endpoint: None,
            client_info: None,
            negotiated_version: None,
            initialized: false,
        }
    }
    pub fn endpoint(graph: ReleaseGraph, endpoint: String) -> Result<Self> {
        if !graph.endpoints.contains_key(&endpoint) {
            return Err(Error::new("release_endpoint_absent"));
        }
        let mut session = Self::new(graph);
        session.endpoint = Some(endpoint);
        Ok(session)
    }
    pub fn check_graph(&mut self) -> Result<()> {
        self.gate.check(self.graph.verify_unchanged())
    }
    pub fn io_receipt(&self, value: &Js) -> Result<()> {
        self.graph
            .receipt(&format!("{}-io.json", crate::audit::occurrence()?), value)
    }
    /// Metadata-only library entry: installing global process ownership is never implicit.
    pub fn handle(&mut self, request: &Js) -> Result<Option<Js>> {
        self.handle_inner(request, None, Limits::default(), None)
    }
    pub fn handle_supervised(
        &mut self,
        request: &Js,
        control: &mut dyn Control,
        limits: Limits,
    ) -> Result<Option<Js>> {
        self.handle_inner(request, Some(control), limits, None)
    }
    /// Explicit owner loan for this call only. The default CLI remains unconfigured.
    pub fn handle_business<T, C>(
        &mut self,
        request: &Js,
        business: &mut crate::business::DailyRecord<'_, '_, T, C>,
    ) -> Result<Option<Js>>
    where
        T: edu_academy_sources::http::Transport,
        C: edu_academy_sources::owner_workflow::CredentialProvider,
    {
        self.handle_inner(request, None, Limits::default(), Some(business))
    }
    /// Separate loan for precisely the two history source routes. All other
    /// messages traverse the unchanged RpcSession lifecycle/refusal path.
    pub fn handle_history<P: edu_academy_sources::history::Port>(
        &mut self,
        request: &Js,
        source: &mut crate::business::History<P>,
    ) -> Result<Option<Js>> {
        if !valid_envelope(request) {
            return Err(Error::new("mcp_stdio_input_or_transport_invalid"));
        }
        let name = request.get("params").get("name");
        let target = request.get("method").is("tools/call")
            && request.has("id")
            && (name.is("academy_student_metadata_get")
                || name.is("academy_paper_headers")
                || self.endpoint.as_deref() == Some("dailyrecord")
                    && name.is("academy_runtime_read_admission"));
        if !target {
            return self.handle(request);
        }
        let id = request.get("id");
        let outcome = (|| -> Result<Js> {
            self.check_graph()?;
            let params = request.get("params");
            if !matches!(params, Js::Obj(_))
                || (params.has("arguments") && !matches!(params.get("arguments"), Js::Obj(_)))
            {
                return Err(Error::new("tool_input_invalid"));
            }
            let input = if params.has("arguments") {
                params.get("arguments").clone()
            } else {
                obj(&[])
            };
            let endpoint = self
                .endpoint
                .as_deref()
                .ok_or_else(|| Error::new("business_endpoint_mismatch"))?;
            let name = string(name)?;
            if name != "academy_runtime_read_admission" {
                let entry = crate::endpoint::history_entry(&self.graph, endpoint, &name)?;
                if let Err(e) = crate::validation::validate(entry.tool.get("inputSchema"), &input) {
                    return Ok(crate::business::tool_error(e.code));
                }
            } else if !input.props().is_empty() {
                return Ok(crate::business::tool_error("tool_input_invalid"));
            }
            source.check_history_binding(&self.graph, endpoint)?;
            let input = match crate::dailytest_history::driver_input(&name, &input) {
                Ok(input) => input,
                Err(e) => return Ok(crate::business::tool_error(e.code)),
            };
            self.check_graph()?;
            let result = source.history_call(&name, &input);
            self.check_graph()?;
            match result {
                Ok(value) => tool_result(value),
                Err(e) => Ok(crate::business::tool_error(&e.message)),
            }
        })();
        let outcome = match self.check_graph() {
            Ok(()) => outcome,
            Err(e) => Err(e),
        };
        Ok(Some(match outcome {
            Ok(value) => rpc_result(id, value),
            Err(e) => business_rpc_error(id, e.code, Effect::NotDispatched, false),
        }))
    }
    fn handle_inner(
        &mut self,
        request: &Js,
        control: Option<&mut dyn Control>,
        limits: Limits,
        business: Option<&mut dyn crate::business::Business>,
    ) -> Result<Option<Js>> {
        if !valid_envelope(request) {
            return Err(Error::new("mcp_stdio_input_or_transport_invalid"));
        }
        let method = string(request.get("method"))?;
        // Valid notifications observe/latch the graph before any state mutation.
        if let Err(e) = self.check_graph() {
            return Ok(if request.has("id") {
                Some(rpc_error(request.get("id"), -32603, e.code))
            } else {
                None
            });
        }
        if !request.has("id") {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            self.check_graph()?;
            return Ok(None);
        }
        let id = request.get("id");
        if method == "tools/call"
            && let Some(business) = business
        {
            return Ok(Some(self.call_business(
                request.get("params"),
                id,
                business,
            )));
        }
        let mut dispatched = false;
        let outcome = match method.as_str() {
            "initialize" => self.initialize(request.get("params")).map(Reply::Tool),
            "ping" => Ok(Reply::Tool(obj(&[]))),
            "tools/list" => {
                let params = request.get("params");
                if request.has("params")
                    && (!matches!(params, Js::Obj(_))
                        || (params.has("cursor") && !params.get("cursor").string()))
                {
                    Err(Error::new("tool_input_invalid"))
                } else {
                    self.list()
                        .map(|tools| Reply::Tool(obj(&[("tools", Js::Arr(tools))])))
                }
            }
            "tools/call" => self.call(request.get("params"), id, control, limits, &mut dispatched),
            _ => Err(Error::new("method_not_found")),
        };
        // Dispatch already performed its final graph readback with its effect
        // stage intact. Never replace a possible-write failure with zero effects.
        if !dispatched && let Err(error) = self.check_graph() {
            return Ok(Some(rpc_error(id, -32603, error.code)));
        }
        Ok(Some(match outcome {
            Ok(Reply::Tool(result)) => rpc_result(id, result),
            Ok(Reply::ReceiptFailure { occurrence, effect }) => {
                let mut response = rpc_error(id, -32603, "receipt_write_unavailable_reconcile");
                let mut error = response.get("error").clone();
                let mut data = obj(&[
                    ("occurrence", occurrence.into()),
                    ("automaticRetryAllowed", false.into()),
                ]);
                if !matches!(effect, Effect::NotDispatched) {
                    data.set("effectStatus", "unknown".into());
                }
                error.set("data", data);
                response.set("error", error);
                response
            }
            Err(error) if error.code == "method_not_found" => {
                rpc_error(id, -32601, "Method not found")
            }
            Err(error) => rpc_error(
                id,
                if error.code == "tool_input_invalid" {
                    -32602
                } else {
                    -32603
                },
                error.code,
            ),
        }))
    }
    fn call_business(
        &mut self,
        params: &Js,
        id: &Js,
        business: &mut dyn crate::business::Business,
    ) -> Js {
        let mut effect = Effect::NotDispatched;
        let mut entered = false;
        let mut unknown_name = None;
        let outcome = (|| -> Result<Js> {
            if !matches!(params, Js::Obj(_))
                || !params.get("name").string()
                || (params.has("arguments") && !matches!(params.get("arguments"), Js::Obj(_)))
            {
                return Err(Error::new("tool_input_invalid"));
            }
            let name = string(params.get("name"))?;
            let args = if params.has("arguments") {
                params.get("arguments").clone()
            } else {
                obj(&[])
            };
            let Some(endpoint) = self.endpoint.as_deref() else {
                return Err(Error::new("business_endpoint_mismatch"));
            };
            if endpoint != "dailyrecord" {
                return Err(Error::new("business_endpoint_mismatch"));
            }
            if name == "academy_endpoint_identity" {
                if !args.props().is_empty() {
                    return Ok(crate::business::tool_error("tool_input_invalid"));
                }
                return tool_result(self.graph.endpoint_identity(endpoint)?);
            }
            let entry = crate::endpoint::business_entry(&self.graph, endpoint, &name)?;
            if entry.is_none() && name != "academy_runtime_read_admission" {
                unknown_name = Some(name);
                return Err(Error::new("business_tool_not_found"));
            }
            if let Some(entry) = entry {
                if let Err(error) =
                    crate::validation::validate(entry.tool.get("inputSchema"), &args)
                {
                    return Ok(crate::business::tool_error(error.code));
                }
            } else if !args.props().is_empty() {
                return Ok(crate::business::tool_error("tool_input_invalid"));
            }
            // A retained but unimplemented registered tool never falls through
            // to the driver or masquerades as one of its seven business routes.
            if !crate::business::TOOLS.contains(&name.as_str()) {
                return crate::endpoint::call_unimplemented(
                    entry.expect("registered route"),
                    &args,
                );
            }
            business.check_binding(&self.graph)?;
            let input = match business.validate(&name, &args) {
                Ok(input) => input,
                Err(error) => return Ok(crate::business::tool_error(&error.message)),
            };
            self.check_graph()?;
            entered = true;
            effect = crate::business::effect(&name);
            let result = business.call(&name, &input);
            // Readback happens even after a Fault, without an automatic retry.
            self.check_graph()?;
            match result {
                Ok(value) => tool_result(value),
                Err(error) => {
                    let mut result = crate::business::tool_error(&error.message);
                    let mut evidence = effect.failure_evidence();
                    evidence.set("automaticRetryAllowed", false.into());
                    evidence.set(
                        "localWorkflowEffects",
                        "unknown_reconcile_owner_store".into(),
                    );
                    result.set("_meta", obj(&[("academy/businessEffect", evidence)]));
                    Ok(result)
                }
            }
        })();
        // Cover schema/unknown/binding/identity failures as well as business I/O.
        if let Err(error) = self.check_graph() {
            return business_rpc_error(id, error.code, effect, entered);
        }
        match outcome {
            Ok(result) => rpc_result(id, result),
            Err(error) if error.code == "business_tool_not_found" => rpc_error(
                id,
                -32602,
                &format!("Tool {} not found", unknown_name.as_deref().unwrap_or("")),
            ),
            Err(error) if error.code == "tool_input_invalid" => rpc_error(id, -32602, error.code),
            Err(error) => business_rpc_error(id, error.code, effect, entered),
        }
    }
    fn list(&self) -> Result<Vec<Js>> {
        let Some(endpoint) = &self.endpoint else {
            return self.graph.registry.list();
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut tools = vec![obj(&[
            ("name", "academy_endpoint_identity".into()),
            (
                "description",
                "Pinned endpoint artifact identity; not business qualification".into(),
            ),
            (
                "inputSchema",
                obj(&[
                    ("type", "object".into()),
                    ("additionalProperties", false.into()),
                ]),
            ),
        ])];
        for entry in self
            .graph
            .registry
            .entries()
            .iter()
            .filter(|e| &e.endpoint == endpoint)
        {
            if matches!(
                entry.upstream_name.as_str(),
                "academy_dailyrecord_get" | "academy_dailytest_history_with_headers"
            ) || !seen.insert(entry.upstream_name.clone())
            {
                continue;
            }
            let mut tool = entry.tool.clone();
            tool.set("name", entry.upstream_name.clone().into());
            tools.push(tool);
        }
        if endpoint == "dailyrecord" {
            tools.push(obj(&[
                ("name", "academy_runtime_read_admission".into()),
                (
                    "description",
                    "Read-port qualification; unavailable until actual owner port joins".into(),
                ),
                (
                    "inputSchema",
                    obj(&[
                        ("type", "object".into()),
                        ("additionalProperties", false.into()),
                    ]),
                ),
            ]));
        }
        Ok(tools)
    }
    fn initialize(&mut self, params: &Js) -> Result<Js> {
        if !matches!(params, Js::Obj(_))
            || !params.get("protocolVersion").string()
            || !matches!(params.get("capabilities"), Js::Obj(_))
        {
            return Err(Error::new("tool_input_invalid"));
        }
        let client = client_info(params.get("clientInfo"))?;
        let requested = string(params.get("protocolVersion"))?;
        let version = if SUPPORTED_VERSIONS.contains(&requested.as_str()) {
            requested
        } else {
            SUPPORTED_VERSIONS[0].into()
        };
        // SDK2.3.1 stdio accepts repeated initialize and pre-initialize requests.
        self.negotiated_version = Some(version.clone());
        self.client_info = Some(client);
        Ok(obj(&[
            ("protocolVersion", version.into()),
            ("capabilities", obj(&[("tools", obj(&[]))])),
            (
                "serverInfo",
                obj(&[
                    ("name", "muse-academy-runtime".into()),
                    ("version", "1.0.0".into()),
                ]),
            ),
        ]))
    }
    fn call(
        &mut self,
        params: &Js,
        id: &Js,
        control: Option<&mut dyn Control>,
        limits: Limits,
        dispatched: &mut bool,
    ) -> Result<Reply> {
        if !matches!(params, Js::Obj(_))
            || !params.get("name").string()
            || (params.has("arguments") && !matches!(params.get("arguments"), Js::Obj(_)))
        {
            return Err(Error::new("tool_input_invalid"));
        }
        let name = string(params.get("name"))?;
        let args = if params.has("arguments") {
            params.get("arguments").clone()
        } else {
            obj(&[])
        };
        if let Some(endpoint) = &self.endpoint {
            if name == "academy_endpoint_identity" {
                if !args.props().is_empty() {
                    return Err(Error::new("tool_input_invalid"));
                }
                return tool_result(self.graph.endpoint_identity(endpoint)?).map(Reply::Tool);
            }
            if name == "academy_runtime_read_admission" && endpoint == "dailyrecord" {
                return qualification_gap(
                    "workflow_source_port_unqualified",
                    Effect::NotDispatched,
                )
                .map(Reply::Tool);
            }
            return crate::endpoint::call_unimplemented_for(&self.graph, endpoint, &name, &args)
                .map(Reply::Tool);
        }
        if name == "academy_runtime_identity" {
            if let Some(control) = control {
                *dispatched = true;
                return dispatch::call(
                    &self.graph,
                    &mut self.gate,
                    &name,
                    &args,
                    self.client_info.as_ref(),
                    id,
                    control,
                    limits,
                );
            }
            if !args.props().is_empty() {
                return Err(Error::new("tool_input_invalid"));
            }
            return tool_result(self.graph.identity()).map(Reply::Tool);
        }
        if let Some(control) = control {
            *dispatched = true;
            return dispatch::call(
                &self.graph,
                &mut self.gate,
                &name,
                &args,
                self.client_info.as_ref(),
                id,
                control,
                limits,
            );
        }
        let entry = match self.graph.registry.get(&name) {
            Ok(entry) => entry,
            Err(error) => {
                return qualification_gap(error.code, Effect::NotDispatched).map(Reply::Tool);
            }
        };
        if let Err(error) = crate::validation::validate(entry.tool.get("inputSchema"), &args) {
            return qualification_gap(error.code, Effect::NotDispatched).map(Reply::Tool);
        }
        let entry =
            match crate::routing::select(&self.graph.routing, &self.graph.registry, entry, &args) {
                Ok(entry) => entry,
                Err(error) => {
                    return qualification_gap(error.code, Effect::NotDispatched).map(Reply::Tool);
                }
            };
        self.gate.check(self.graph.verify_unchanged())?;
        crate::endpoint::call_unimplemented(entry, &args).map(Reply::Tool)
    }
}
fn client_info(value: &Js) -> Result<Js> {
    let invalid = || Error::new("tool_input_invalid");
    if !matches!(value, Js::Obj(_)) || !value.get("name").string() || !value.get("version").string()
    {
        return Err(invalid());
    }
    let mut out = obj(&[("name", value.get("name").clone())]);
    for key in ["title", "icons", "version", "websiteUrl", "description"] {
        if !value.has(key) {
            continue;
        }
        let v = value.get(key);
        if key != "icons" {
            if !v.string() {
                return Err(invalid());
            }
            out.set(key, v.clone());
            continue;
        }
        if !matches!(v, Js::Arr(_)) {
            return Err(invalid());
        }
        let mut icons = vec![];
        for icon in v.array() {
            if !matches!(icon, Js::Obj(_)) || !icon.get("src").string() {
                return Err(invalid());
            }
            let mut parsed = obj(&[("src", icon.get("src").clone())]);
            for key in ["mimeType", "sizes", "theme"] {
                if !icon.has(key) {
                    continue;
                }
                let val = icon.get(key);
                let valid = match key {
                    "sizes" => matches!(val, Js::Arr(_)) && val.array().iter().all(Js::string),
                    "theme" => val.is("light") || val.is("dark"),
                    _ => val.string(),
                };
                if !valid {
                    return Err(invalid());
                }
                parsed.set(key, val.clone());
            }
            icons.push(parsed);
        }
        out.set("icons", Js::Arr(icons));
    }
    Ok(out)
}
pub fn valid_envelope(v: &Js) -> bool {
    matches!(v, Js::Obj(_))
        && v.get("jsonrpc").is("2.0")
        && v.get("method").string()
        && !v.props().iter().any(|(key, _)| {
            !["jsonrpc", "method", "id", "params"]
                .iter()
                .any(|name| key == &crate::text(name))
        })
        && (!v.has("params") || matches!(v.get("params"), Js::Obj(_)))
        && (!v.has("id")
            || v.get("id").string()
            || matches!(v.get("id"), Js::Num(n) if n.is_finite() && n.fract()==0.0 && n.abs()<=9007199254740991.0))
}
pub fn rpc_result(id: &Js, result: Js) -> Js {
    obj(&[
        ("result", result),
        ("jsonrpc", "2.0".into()),
        ("id", id.clone()),
    ])
}
pub fn rpc_error(id: &Js, code: i32, message: &str) -> Js {
    obj(&[
        ("jsonrpc", "2.0".into()),
        ("id", id.clone()),
        (
            "error",
            obj(&[
                ("code", Js::Num(f64::from(code))),
                ("message", message.into()),
            ]),
        ),
    ])
}

// SDK output failures are RPC InternalError, with effect evidence retained by
// this Rust boundary. In particular a failed save output is never zero effects.
fn business_rpc_error(id: &Js, code: &str, effect: Effect, entered: bool) -> Js {
    let mut response = rpc_error(
        id,
        -32603,
        if code == "mcp_result_serialization_failed" {
            "Internal error"
        } else {
            code
        },
    );
    let mut error = response.get("error").clone();
    let mut data = effect.failure_evidence();
    data.set("reasonCode", code.into());
    data.set("automaticRetryAllowed", false.into());
    data.set("businessDispatched", entered.into());
    if entered {
        data.set(
            "localWorkflowEffects",
            "unknown_reconcile_owner_store".into(),
        );
    }
    error.set("data", data);
    response.set("error", error);
    response
}
