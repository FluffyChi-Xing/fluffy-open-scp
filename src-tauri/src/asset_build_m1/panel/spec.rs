use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub native: String,
    pub group: String,
    #[serde(default)]
    pub feedback: bool,
    #[serde(default)]
    pub counter: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<Metric>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub current: String,
    pub previous: String,
}
pub fn components() -> Vec<Component> {
    serde_json::from_str(include_str!("../../../../src/lib/panel-components.json"))
        .expect("panel component contract")
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelSchema {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "schema_version")]
    pub version: u32,
    /// Exact local-game layout TGI, discovered from the donor's inherited UI property.
    pub shell: String,
    pub hover: Vec<String>,
    pub click: Vec<String>,
    pub power: bool,
    pub missing_feedback: MissingFeedback,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MissingFeedback {
    Hide,
    Native,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dependency {
    pub schema: String,
    pub sources: Vec<Source>,
    pub runtime: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct PanelPlan {
    pub path: String,
    pub config: PanelSchema,
    pub dependency: Dependency,
    pub fingerprint: String,
}

pub fn parse(raw: &str) -> Result<PanelSchema, String> {
    let schema: PanelSchema =
        serde_json::from_str(raw).map_err(|e| format!("PANEL_SCHEMA: {e}"))?;
    if schema.schema != "openscp.panel" || schema.version != 1 {
        return Err("PANEL_SCHEMA_VERSION".into());
    }
    let shell = crate::asset_build_m1::patch::parse_tgi(&schema.shell)?;
    if shell.type_id != 0x6777_1F5C {
        return Err("PANEL_SHELL_TYPE".into());
    }
    let known = components();
    for (name, items) in [("hover", &schema.hover), ("click", &schema.click)] {
        if items.len() > 48 {
            return Err(format!("PANEL_SLOT_LIMIT:{name}"));
        }
        let mut seen = BTreeSet::new();
        for id in items {
            if !known.iter().any(|c| &c.id == id) {
                return Err(format!("PANEL_COMPONENT_UNKNOWN:{id}"));
            }
            if !seen.insert(id) {
                return Err(format!("PANEL_COMPONENT_DUPLICATE:{name}:{id}"));
            }
        }
    }
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc() -> serde_json::Value {
        serde_json::json!({"$schema":"openscp.panel","schema_version":1,"shell":"67771F5C:0B07405A:2BDCBFDD","hover":[],"click":["residents","park-visitors"],"power":true,"missingFeedback":"hide"})
    }
    #[test]
    fn empty_is_explicit_and_unknown_components_fail() {
        let mut d = doc();
        assert!(parse(&d.to_string()).unwrap().hover.is_empty());
        d["click"] = serde_json::json!(["unknown"]);
        assert!(parse(&d.to_string()).unwrap_err().contains("UNKNOWN"));
        d["click"] = serde_json::json!(["residents", "residents"]);
        assert!(parse(&d.to_string()).unwrap_err().contains("DUPLICATE"));
    }
}
