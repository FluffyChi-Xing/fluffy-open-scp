use crate::asset_build_m0::schema::{AssetSchemaV2, ParsedSchema, parse_tgi};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const IM_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tgi {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntermediateModel {
    pub version: u32,
    pub schema_id: String,
    pub schema_hash: String,
    pub kind: String,
    pub references: ReferenceIr,
    pub scene: SceneIr,
    pub semantics: SemanticsIr,
    pub output: OutputIr,
    pub slots: Vec<ProcessorSlot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceIr {
    pub package: Option<String>,
    pub menu: Option<Tgi>,
    pub unit: Option<Tgi>,
    pub lot: Option<Tgi>,
    pub unit_template: Option<Tgi>,
    pub lot_template: Option<Tgi>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneIr {
    pub document: String,
    pub model: Option<String>,
    pub slots: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsIr {
    pub asset_class: Option<String>,
    pub unit_overrides: BTreeMap<String, Value>,
    pub lot_bindings: BTreeMap<String, Value>,
    pub menu: BTreeMap<String, Value>,
    pub properties: Vec<Value>,
    pub allow_shared_template: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputIr {
    pub package: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessorKind {
    Property,
    Raster,
    Rw4,
    Menu,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessorSlot {
    pub id: String,
    pub kind: ProcessorKind,
    pub input: Option<String>,
    pub resource: Option<Tgi>,
}

pub fn lower_schema(parsed: &ParsedSchema) -> Result<IntermediateModel, String> {
    let schema = &parsed.value;
    Ok(IntermediateModel {
        version: IM_VERSION,
        schema_id: schema.id.clone(),
        schema_hash: parsed.hash.clone(),
        kind: schema.kind.clone(),
        references: lower_references(schema)?,
        scene: SceneIr {
            document: schema.scene.document.clone(),
            model: schema.scene.model.clone(),
            slots: schema.scene.slots.clone(),
        },
        semantics: SemanticsIr {
            asset_class: schema.semantics.asset_class.clone(),
            unit_overrides: schema.semantics.unit_overrides.clone(),
            lot_bindings: schema.semantics.lot_bindings.clone(),
            menu: schema.semantics.menu.clone(),
            properties: schema.semantics.properties.clone(),
            allow_shared_template: schema.semantics.allow_shared_template,
        },
        output: OutputIr {
            package: schema.outputs.package.clone(),
        },
        slots: processor_slots(schema),
    })
}

fn lower_references(schema: &AssetSchemaV2) -> Result<ReferenceIr, String> {
    Ok(ReferenceIr {
        package: schema.reference.package.clone(),
        menu: parse_optional(schema.reference.menu.as_deref())?,
        unit: parse_optional(schema.reference.unit.as_deref())?,
        lot: parse_optional(schema.reference.lot.as_deref())?,
        unit_template: parse_optional(schema.reference.unit_template.as_deref())?,
        lot_template: parse_optional(schema.reference.lot_template.as_deref())?,
    })
}

fn parse_optional(value: Option<&str>) -> Result<Option<Tgi>, String> {
    value.map(parse_tgi).transpose().map(|value| {
        value.map(|resource| Tgi {
            type_id: resource.type_id,
            group: resource.group,
            instance: resource.instance,
        })
    })
}

fn processor_slots(schema: &AssetSchemaV2) -> Vec<ProcessorSlot> {
    let mut slots = Vec::new();
    if schema.reference.unit.is_some()
        || schema.reference.lot.is_some()
        || !schema.semantics.properties.is_empty()
    {
        slots.push(ProcessorSlot {
            id: "property".into(),
            kind: ProcessorKind::Property,
            input: Some(schema.scene.document.clone()),
            resource: schema
                .reference
                .unit
                .as_deref()
                .or(schema.reference.lot.as_deref())
                .and_then(|value| parse_tgi(value).ok())
                .map(|resource| Tgi {
                    type_id: resource.type_id,
                    group: resource.group,
                    instance: resource.instance,
                }),
        });
    }
    for (slot, input) in &schema.scene.slots {
        slots.push(ProcessorSlot {
            id: format!("raster:{slot}"),
            kind: ProcessorKind::Raster,
            input: Some(input.clone()),
            resource: None,
        });
    }
    if schema.scene.model.is_some() {
        slots.push(ProcessorSlot {
            id: "rw4:model".into(),
            kind: ProcessorKind::Rw4,
            input: schema.scene.model.clone(),
            resource: None,
        });
    }
    if schema.reference.menu.is_some() || !schema.semantics.menu.is_empty() {
        slots.push(ProcessorSlot {
            id: "menu".into(),
            kind: ProcessorKind::Menu,
            input: None,
            resource: schema
                .reference
                .menu
                .as_deref()
                .and_then(|value| parse_tgi(value).ok())
                .map(|resource| Tgi {
                    type_id: resource.type_id,
                    group: resource.group,
                    instance: resource.instance,
                }),
        });
    }
    slots.push(ProcessorSlot {
        id: "package".into(),
        kind: ProcessorKind::Package,
        input: None,
        resource: None,
    });
    slots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m0::schema::parse_schema;

    #[test]
    fn lowers_willis_references_and_expands_slots() {
        let raw = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "willis", "kind": "building-asset",
            "reference": {"unit": "00B1B104:40E0C000:8D007745", "lot": "00B1B104:40E1C000:8D007745", "menu": "00B1B104:09878A01:74168A83"},
            "scene": {"document": "asset.lot.json", "slots": {"slot0": "slot0.json", "slot1": "slot1.png"}},
            "outputs": {"package": "willis.package"}
        });
        let parsed = parse_schema(&raw.to_string()).unwrap();
        let im = lower_schema(&parsed).unwrap();
        assert_eq!(im.references.unit.unwrap().instance, 0x8D007745);
        assert_eq!(
            im.slots
                .iter()
                .filter(|slot| slot.kind == ProcessorKind::Raster)
                .count(),
            2
        );
        assert_eq!(im.slots.last().unwrap().kind, ProcessorKind::Package);
    }
}
