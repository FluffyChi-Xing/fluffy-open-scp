//! Qualitative native controls read one actual business Unit, never summed flags.
use super::{PanelSchema,hash};
use crate::asset_build_m1::{behavior_nodes::ResolvedBehavior,behavior_units::UnitBehavior,
    behavior_unit_build::instance,connector::Resources,property_edit::set_scalar};
use sc_properties::{PropertyFile,PropType,Value};
pub(super) const RUNTIME:&str=include_str!("../../../../src/pages/studio/components/asset/panel-editor/native/component-queries.js");
const RESIDENCE:u32=0xA00F_E310;
const TOURISM:u32=0xA00F_E311;
const PARK:u32=0xA00F_E312;
fn role(item:&str)->Option<u32>{
    match item {
        "residence-feedback"|"residents"|"land-value-residential"|"residential-happiness-reasons"|"residential-sadness-reasons"=>Some(RESIDENCE),
        "tourism-revenue"=>Some(TOURISM),
        "park-feedback"|"park-hours"|"park-visitors-title"|"park-visitors"=>Some(PARK),_=>None,
    }
}
fn target(program:&ResolvedBehavior,role:u32)->Option<&UnitBehavior>{
    if !program.copan_plop_base{return None;}
    let mut matches=program.units.iter().filter(|u|match role {
        RESIDENCE=>u.program.templates.residence(),TOURISM=>u.program.native_tourism.is_some(),
        PARK=>u.program.copan_park_core,_=>false,
    });
    let unit=matches.next()?;
    (matches.next().is_none()&&unit.shares_building_site).then_some(unit)
}
pub(super) fn supported(item:&str,program:&ResolvedBehavior)->bool{
    role(item).and_then(|r|target(program,r)).is_some()
}
pub(super) fn binding(item:&str,hide:bool,program:&ResolvedBehavior)->Option<Value>{
    if !supported(item,program){return None;}
    let suffix=if matches!(item,"residence-feedback"|"park-feedback") {if hide {"_hide"}else{"_native"}}else{""};
    Some(Value::UInt32(hash(&format!("OpenSCPPanel_component_{item}{suffix}"))))
}
fn selected(config:&PanelSchema)->std::collections::BTreeSet<u32>{
    config.hover.iter().chain(&config.click).filter_map(|s|role(s)).collect()
}
pub(super) fn write(asset:&mut PropertyFile,config:&PanelSchema,program:&ResolvedBehavior,host:u32)->Result<(),String>{
    for field in selected(config){
        let unit=target(program,field).ok_or("PANEL_CONTAINER_BINDING: ambiguous component query")?;
        if asset.get(field).is_some(){return Err("PANEL_CONTAINER_BINDING: component selector collision".into());}
        set_scalar(asset,field,PropType::UInt32,Value::UInt32(instance(host,&unit.id)));
    }
    Ok(())
}
pub(super) fn verify(resources:&Resources,asset:&PropertyFile,config:&PanelSchema,program:&ResolvedBehavior,host:u32)->Result<(),String>{
    for field in selected(config){
        let unit=target(program,field).ok_or("PANEL_CONTAINER_BINDING: ambiguous component query")?;
        let child=instance(host,&unit.id);
        if asset.get(field).and_then(|p|p.scalar())!=Some(&Value::UInt32(child))
            || !resources.contains_key(&dbpf::ResourceId{type_id:0x00B1_B104,group:0x40E0_C000,instance:child}){
            return Err("PANEL_CONTAINER_BINDING: missing or stale component selector".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn qualitative_residential_rows_require_one_identifiable_residence(){
        let mut p=crate::asset_build_m1::behavior_copan_base::compile_building(None,&[json!({"id":"home","type":"residence","params":{}})]).unwrap();
        assert!(supported("residence-feedback",&p));assert!(!supported("tourism-revenue",&p));
        p.units.push(p.units[0].clone());assert!(!supported("residence-feedback",&p));
    }
}
