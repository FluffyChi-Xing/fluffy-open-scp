//! Lower the legacy behavior selection to ordinary, single-Unit building recipes.
use super::behavior_nodes::{self, ResolvedBehavior};
use sc_properties::{Kind, PropertyFile, Value};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    floors: Vec<Behavior>,
    labor: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Behavior {
    kind: String,
    // Accepted for documents authored with the retired physical floor route.
    #[serde(rename = "offsetZ")]
    _offset_z: Option<f32>,
    params: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Selection {
    pub(super) fn parse(value: serde_json::Value) -> Result<Self, String> {
        let selection: Self =
            serde_json::from_value(value).map_err(|error| format!("MEGA_TOWER_CONFIG: {error}"))?;
        if selection.floors.is_empty() {
            return Err("MEGA_TOWER_CONFIG: select at least one behavior".into());
        }
        Ok(selection)
    }

    pub(super) fn compile(&self) -> Result<ResolvedBehavior, String> {
        self.compile_with_copan_base(false)
    }

    pub(super) fn compile_with_copan_base(&self, use_copan: bool) -> Result<ResolvedBehavior,String> {
        let nodes = self
            .floors
            .iter()
            .map(|behavior| {
                let mut params = behavior.params.clone().unwrap_or_default();
                let profile = match behavior.kind.as_str() {
                    "residence" => Some(("residenceProfile", "ground-residence-w1-v1")),
                    "tourism" => Some(("tourismProfile", "native-tourism-v1")),
                    "commercial" | "education" | "office" | "park" | "hotel" => None,
                    _ => {
                        return Err(format!(
                            "MEGA_TOWER_CONFIG: unknown behavior {}",
                            behavior.kind
                        ));
                    }
                };
                if let Some((key, value)) = profile {
                    params.entry(key.to_owned()).or_insert(json!(value));
                }
                Ok(json!({ "id": behavior.kind, "type": behavior.kind, "params": params }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let labor=behavior_nodes::parse_labor(self.labor.as_ref())?;
        if use_copan && super::behavior_copan_base::supports(&nodes) {
            super::behavior_copan_base::compile_building(labor,&nodes)
        } else { compile_building(labor,&nodes,false) }
    }
}

pub(super) fn compile_building(
    labor: Option<behavior_nodes::LaborSpec>,
    nodes: &[serde_json::Value],
    author_tourism: bool,
) -> Result<ResolvedBehavior, String> {
    if super::behavior_casino::requested(nodes) || super::behavior_controlnet::requested(nodes) || super::behavior_fusion::requested(nodes) {
        return behavior_nodes::compile_with_profile(labor, nodes, author_tourism);
    }
    if super::behavior_units::scoped(nodes) {
        return behavior_nodes::compile_with_profile(labor, nodes, author_tourism);
    }
    let mut nodes = nodes.to_vec();
    for node in &mut nodes {
        if let Some(params) = node["params"].as_object_mut() {
            for field in [
                "w1",
                "w2",
                "w3",
                "minWorkers",
                "jobsW1",
                "jobsW2",
                "jobsW3",
                "retailMinWorkers",
                "unitLabor",
            ] {
                params.remove(field);
            }
        }
    }
    if super::behavior_policy::needs_components(&nodes)? {
        return super::behavior_units::compile_workbench(labor, &nodes, author_tourism);
    }
    compile_host(labor, nodes, author_tourism)
}

pub(super) fn compile_host(
    labor: Option<behavior_nodes::LaborSpec>,
    mut nodes: Vec<serde_json::Value>,
    author_tourism: bool,
) -> Result<ResolvedBehavior, String> {
    let office = nodes.iter().position(|node| node["type"] == "office");
    let retail = nodes.iter().position(|node| node["type"] == "commercial");
    if let (Some(office), Some(retail)) = (office, retail)
        && nodes
            .iter()
            .filter(|node| node["type"] != "utilities-core")
            .count()
            == 2
    {
        // Keep the utilities node when using the combined recipe.
        nodes.remove(office);
        let retail = retail - usize::from(office < retail);
        return super::behavior_office_retail::compile(&nodes, retail, labor);
    }
    let economic = retail.is_some() || nodes.iter().any(|node| node["type"] == "hotel");
    let labor = if economic {
        Some(labor.unwrap_or(behavior_nodes::LaborSpec {
            w1: 0,
            w2: 0,
            w3: 0,
            min_workers: 0,
            schedule: Some([0; 8]),
            slots: None,
        }))
    } else {
        labor
    };
    if let Some(office) = office {
        let labor = labor.ok_or("OFFICE_PARAMETER: configure the building's workforce")?;
        let params = &mut nodes[office]["params"];
        for (field, value) in [
            ("w1", labor.w1),
            ("w2", labor.w2),
            ("w3", labor.w3),
            ("minWorkers", labor.min_workers),
        ] {
            params[field] = json!(value);
        }
        let mut program = behavior_nodes::compile_with_profile(None, &nodes, author_tourism)?;
        if let Some(schedule) = labor.schedule {
            for (index, workers) in schedule.into_iter().enumerate() {
                program
                    .set_int_fields
                    .insert(0x0D00_853A + index as u32, workers);
            }
        }
        return Ok(program);
    }
    behavior_nodes::compile_with_profile(labor, &nodes, author_tourism)
}

pub(super) fn verify(unit: &[u8], lot: &PropertyFile, expected_children: usize) -> Result<(), String> {
    let unit = PropertyFile::parse(unit).map_err(|error| error.to_string())?;
    let unexpected_children = [0x07E0_AD0A, 0x07E0_AD0B].into_iter().any(|hash| {
        match unit.get(hash).map(|property| &property.kind) {
            None => expected_children != 0,
            Some(Kind::Array(values)) => expected_children == 0 || values.len() != expected_children,
            _ => true,
        }
    });
    if unexpected_children || unit.get(0x0AFE_27CA).is_some()
        || matches!(unit.get(0x0DA6_9D6F).and_then(|property| property.scalar()),
            Some(Value::Key(key)) if key.instance == 0xB940_136F)
    {
        return Err(
            "MEGA_TOWER_CONFIG: an ordinary building cannot place tower floors or roads".into(),
        );
    }
    if matches!(lot.get(0x0CC8_FE7A).map(|p| &p.kind), Some(Kind::Array(entries))
        if entries.iter().any(|entry| matches!(entry, Value::Key(key)
            if [0xCDFC_0181, 0xC1C2_EF97, 0x2F22_9A6E, 0x863A_6E7D].contains(&key.instance))))
    {
        return Err("MEGA_TOWER_CONFIG: use ordinary authored paths".into());
    }
    let has = |hash, instance| {
        matches!(unit.get(hash).map(|p| &p.kind),
        Some(Kind::Array(values)) if values.iter().any(|value| matches!(value, Value::Key(key) if key.instance == instance)))
    };
    for instance in [0x4893_9D96, 0x825B_4CA4, 0xD7C6_2954, 0x54AB_8B16] {
        if !(0x0A02_09A2..=0x0A02_09A5).any(|hash| has(hash, instance))
            || has(0x0A02_09B2, instance)
        {
            return Err(format!(
                "MEGA_TOWER_CONFIG: ordinary utility bundle {instance:08X} is inactive"
            ));
        }
    }
    Ok(())
}
