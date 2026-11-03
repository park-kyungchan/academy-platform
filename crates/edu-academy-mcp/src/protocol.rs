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
        self.handle_inner(request, None, Limits::default())
    }
    pub fn handle_supervised(
        &mut self,
        request: &Js,
        control: &mut dyn Control,
        limits: Limits,
    ) -> Result<Option<Js>> {
        self.handle_inner(request, Some(control), limits)
    }
    fn handle_inner(
        &mut self,
        request: &Js,
        control: Option<&mut dyn Control>,
        limits: Limits,
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
