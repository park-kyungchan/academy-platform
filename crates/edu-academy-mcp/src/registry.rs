//! Pinned product registry metadata. No embedded business-result fixture.
use crate::{Error, Js, Result, identity::valid_name, parse, string};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Entry {
    pub endpoint: String,
    pub upstream_name: String,
    pub tool: Js,
}
pub struct ToolRegistry {
    entries: Vec<Entry>,
}
impl ToolRegistry {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let raw = parse(bytes)?;
        if !matches!(&raw, Js::Arr(_)) || raw.array().len() > 256 {
            return Err(Error::new("release_registry_unqualified"));
        }
        let mut entries = Vec::new();
        let mut names = BTreeSet::new();
        for value in raw.array() {
            if !matches!(value, Js::Obj(_))
                || value.props().len() != 3
                || !["endpoint", "upstreamName", "tool"]
                    .iter()
                    .all(|key| value.has(key))
            {
                return Err(Error::new("release_registry_unqualified"));
            }
            let endpoint = string(value.get("endpoint"))?;
            let upstream_name = string(value.get("upstreamName"))?;
            let tool = value.get("tool");
            let name = string(tool.get("name"))?;
            if !valid_name(&endpoint, 64)
                || !valid_name(&upstream_name, 128)
                || !valid_name(&name, 128)
                || name == "academy_runtime_identity"
                || !matches!(tool, Js::Obj(_))
                || !names.insert(name)
            {
                return Err(Error::new("discovery_tool_name_collision"));
            }
            crate::validation::check_schema(tool.get("inputSchema"))?;
            entries.push(Entry {
                endpoint,
                upstream_name,
                tool: tool.clone(),
            });
        }
        Ok(Self { entries })
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn get(&self, name: &str) -> Result<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.tool.get("name").is(name))
            .ok_or_else(|| Error::new("unknown_tool"))
    }
    pub fn list(&self) -> Result<Vec<Js>> {
        let mut tools = vec![parse(include_bytes!("runtime-identity-tool.json"))?];
        tools.extend(self.entries.iter().map(|entry| entry.tool.clone()));
        Ok(tools)
    }
    pub fn verify_endpoints<'a>(&self, endpoints: impl Iterator<Item = &'a str>) -> Result<()> {
        let available = endpoints.collect::<BTreeSet<_>>();
        if self
            .entries
            .iter()
            .any(|entry| !available.contains(entry.endpoint.as_str()))
        {
            return Err(Error::new("release_payload_unqualified"));
        }
        Ok(())
    }
    pub fn verify_upstream(&self, endpoint: &str, listed: &[Js]) -> Result<()> {
        // These names are bridge compositions, not required endpoint tools.
        let compositions = [
            "academy_dailyrecord_get",
            "academy_dailytest_history_with_headers",
        ];
        for entry in self.entries.iter().filter(|entry| {
            entry.endpoint == endpoint && !compositions.contains(&entry.upstream_name.as_str())
        }) {
            if !listed
                .iter()
                .any(|tool| tool.get("name").is(&entry.upstream_name))
            {
                return Err(Error::new("upstream_tool_inventory_mismatch"));
            }
        }
        Ok(())
    }
}
