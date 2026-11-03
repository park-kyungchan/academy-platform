//! Retained legacy MCP wire surface. Full SDK error-edge qualification is pending.
use crate::{
    Error, Js, Result,
    audit::{Effect, qualification_gap, tool_result},
    obj,
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
    pub client_info: Option<Js>,
    pub negotiated_version: Option<String>,
    pub initialized: bool,
}
impl RpcSession {
    pub fn new(graph: ReleaseGraph) -> Self {
        Self {
            graph,
            gate: ConnectionGate::default(),
            client_info: None,
            negotiated_version: None,
            initialized: false,
        }
    }
    pub fn handle(&mut self, request: &Js) -> Result<Option<Js>> {
        if !valid_envelope(request) {
            return Err(Error::new("mcp_stdio_input_or_transport_invalid"));
        }
        let method = string(request.get("method"))?;
        if !request.has("id") {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return Ok(None);
        }
        let id = request.get("id");
        if let Err(error) = self.gate.check(self.graph.verify_unchanged()) {
            return Ok(Some(rpc_error(id, -32603, error.code)));
        }
        let outcome = match method.as_str() {
            "initialize" => self.initialize(request.get("params")),
            "ping" => Ok(obj(&[])),
            "tools/list" => {
                let params = request.get("params");
                if request.has("params")
                    && (!matches!(params, Js::Obj(_))
                        || (params.has("cursor") && !params.get("cursor").string()))
                {
                    Err(Error::new("tool_input_invalid"))
                } else {
                    self.graph
                        .registry
                        .list()
                        .map(|tools| obj(&[("tools", Js::Arr(tools))]))
                }
            }
            "tools/call" => self.call(request.get("params")),
            _ => Err(Error::new("method_not_found")),
        };
        if let Err(error) = self.gate.check(self.graph.verify_unchanged()) {
            // This increment never dispatches. Later write adapters must preserve their own effect stage.
            return Ok(Some(rpc_error(id, -32603, error.code)));
        }
        Ok(Some(match outcome {
            Ok(result) => rpc_result(id, result),
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
    fn initialize(&mut self, params: &Js) -> Result<Js> {
        if !matches!(params, Js::Obj(_))
            || !params.get("protocolVersion").string()
            || !matches!(params.get("capabilities"), Js::Obj(_))
            || !matches!(params.get("clientInfo"), Js::Obj(_))
            || !params.get("clientInfo").get("name").string()
            || !params.get("clientInfo").get("version").string()
        {
            return Err(Error::new("tool_input_invalid"));
        }
        let requested = string(params.get("protocolVersion"))?;
        let version = if SUPPORTED_VERSIONS.contains(&requested.as_str()) {
            requested
        } else {
            SUPPORTED_VERSIONS[0].into()
        };
        self.negotiated_version = Some(version.clone());
        self.client_info = Some(params.get("clientInfo").clone());
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
    fn call(&mut self, params: &Js) -> Result<Js> {
        if !matches!(params, Js::Obj(_))
            || !params.get("name").string()
            || (params.has("arguments") && !matches!(params.get("arguments"), Js::Obj(_)))
        {
            return Err(Error::new("tool_input_invalid"));
        }
        let name = string(params.get("name"))?;
        let arguments = if params.has("arguments") {
            params.get("arguments").clone()
        } else {
            obj(&[])
        };
        if name == "academy_runtime_identity" {
            if !arguments.props().is_empty() {
                return Err(Error::new("tool_input_invalid"));
            }
            return tool_result(self.graph.identity());
        }
        let entry = match self.graph.registry.get(&name) {
            Ok(entry) => entry,
            Err(error) => return qualification_gap(error.code, Effect::NotDispatched),
        };
        if let Err(error) = crate::validation::validate(entry.tool.get("inputSchema"), &arguments) {
            return qualification_gap(error.code, Effect::NotDispatched);
        }
        let entry = match crate::routing::select(
            &self.graph.routing,
            &self.graph.registry,
            entry,
            &arguments,
        ) {
            Ok(entry) => entry,
            Err(error) => return qualification_gap(error.code, Effect::NotDispatched),
        };
        self.gate.check(self.graph.verify_unchanged())?;
        crate::endpoint::call_unimplemented(entry, &arguments)
    }
}
fn valid_envelope(v: &Js) -> bool {
    if !matches!(v, Js::Obj(_))
        || !v.get("jsonrpc").is("2.0")
        || !v.get("method").string()
        || v.props().iter().any(|(key, _)| {
            !["jsonrpc", "method", "id", "params"]
                .iter()
                .any(|name| key == &crate::text(name))
        })
        || (v.has("params") && !matches!(v.get("params"), Js::Obj(_)))
    {
        return false;
    }
    if !v.has("id") {
        return true;
    }
    matches!(v.get("id"), Js::Str(_))
        || matches!(v.get("id"), Js::Num(n) if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9007199254740991.0)
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
