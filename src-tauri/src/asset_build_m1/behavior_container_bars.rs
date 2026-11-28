//! Retail's native modular bar grouping, not world-space component offsets.
//! Installed EXE SHA256 2b0aff52…: 7A7D90 reads nonSim Bool 0FB2DB9A;
//! 6ACFE0 / 6AD8C0 then aggregate component bars via GetBaseUnit (7A8A00)
//! and 6A9F90. The native tower Asset parent 61F0C000:0229A393 enables it.
use super::{behavior_nodes::ResolvedBehavior, connector::Resources, menu,
    package::BuildResources, property_edit::{encode, set_scalar}};
use dbpf::ResourceId;
use sc_properties::{PropType, PropertyFile, Value};

const GROUP_BARS: u32 = 0x0FB2_DB9A;
fn assets(host: u32, program: &ResolvedBehavior) -> Vec<ResourceId> {
    std::iter::once(host)
        .chain(program.units.iter().map(|u| super::behavior_unit_build::instance(host, &u.id)))
        .map(|instance| ResourceId {type_id: menu::PROPERTY_TYPE, group: menu::lot_group(), instance})
        .collect()
}
pub(super) fn apply(resources: &mut BuildResources, host: u32,
    program: &ResolvedBehavior) -> Result<(), String> {
    for id in assets(host, program) {
        let bytes = resources.get(&id).ok_or_else(|| format!("CONTAINER_BARS: missing Asset {id}"))?;
        let mut asset = PropertyFile::parse(bytes).map_err(|e|e.to_string())?;
        set_scalar(&mut asset, GROUP_BARS, PropType::Bool, Value::Bool(true));
        resources.insert(id, encode(&asset)?);
    }
    verify(resources.all(), host, program)
}
pub(super) fn verify(resources: &Resources, host: u32,
    program: &ResolvedBehavior) -> Result<(), String> {
    for id in assets(host, program) {
        let asset = PropertyFile::parse(resources.get(&id)
            .ok_or_else(||format!("CONTAINER_BARS: missing Asset {id}"))?).map_err(|e|e.to_string())?;
        if asset.get(GROUP_BARS).and_then(|p|p.scalar()) != Some(&Value::Bool(true)) {
            return Err(format!("CONTAINER_BARS: {id} requires native Bool 0FB2DB9A=true"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_grouping_on_host_and_every_component_without_touching_paths() {
        let program = super::super::behavior_nodes::compile(None,&[
            serde_json::json!({"id":"work","type":"office","unit":"work","params":{}}),
            serde_json::json!({"id":"tourism","type":"tourism","unit":"visits",
                "params":{"tourismProfile":"native-tourism-v1"}}),
        ]).unwrap();
        let host = 42;
        let mut source = PropertyFile::default();
        set_scalar(&mut source, 0x0AD9_0138, PropType::Bool, Value::Bool(false));
        let bytes = encode(&source).unwrap();
        let ids = assets(host,&program);
        let mut resources = BuildResources::from_source(ids.iter().map(|id|(*id,bytes.clone())).collect());
        assert!(verify(resources.all(),host,&program).is_err());
        apply(&mut resources,host,&program).unwrap();
        for id in &ids {
            let mut file = PropertyFile::parse(resources.get(id).unwrap()).unwrap();
            assert_eq!(file.get(0x0AD9_0138).and_then(|p|p.scalar()),Some(&Value::Bool(false)));
            file.values.retain(|p|p.hash!=GROUP_BARS);
            assert_eq!(encode(&file).unwrap(),bytes);
            let mut broken = resources.all().clone();
            broken.insert(*id,encode(&file).unwrap());
            assert!(verify(&broken,host,&program).is_err());
        }
    }
}
