//! Additional service ownership boundary for the new park/mall/office route.
//! The already accepted residence/tourism route remains byte-for-byte frozen.
use super::{behavior_nodes::ResolvedBehavior,behavior_templates::{array,contains,key,ENABLE,EXCLUDED},
    property_edit::set_array_with_flags};
use sc_properties::{PropType,PropertyFile};
const BUNDLES:[u32;3]=[0xD7C6_2954,0x54AB_8B16,0x1B94_6073];
const INPUTS:[u32;2]=[0x0C21_6562,0x0AFB_9886];
pub(super) fn selected(program:&ResolvedBehavior)->bool{
    program.copan_park_core||program.commercial.is_some()||program.office.is_some()
}
pub(super) fn manages_components(program:&ResolvedBehavior)->bool{
    // A business composition also contains decorative/land-value components.
    // Those share the same host and must not retain public service producers.
    // The accepted residence+tourism-only composition never enters this path.
    program.units.iter().any(|unit|selected(&unit.program))
}
pub(super) fn apply(unit:&mut PropertyFile)->Result<(),String>{
    let mut excluded=array(unit,EXCLUDED)?.to_vec();
    for id in BUNDLES{if !excluded.contains(&key(id)){excluded.push(key(id));}}
    set_array_with_flags(unit,EXCLUDED,PropType::Key,excluded,0x809C);
    unit.values.retain(|p|!INPUTS.contains(&p.hash));
    verify(unit)
}
pub(super) fn verify(unit:&PropertyFile)->Result<(),String>{
    for id in BUNDLES{
        if !contains(unit,&[EXCLUDED],id)||(contains(unit,&ENABLE,id)&&!contains(unit,&[EXCLUDED],id)){
            return Err(format!("BEHAVIOR_BUSINESS_SERVICES: child owns public service {id:08X}"));
        }
    }
    if unit.values.iter().any(|p|INPUTS.contains(&p.hash)){
        return Err("BEHAVIOR_BUSINESS_SERVICES: child owns garbage/sewage inputs".into());
    }
    Ok(())
}

pub(super) fn finalize_mall_capacity(unit:&mut PropertyFile,child:u32)->Result<(),String>{
    use sc_properties::{Key,Value};
    let binding=Value::Key(Key{type_id:super::menu::PROPERTY_TYPE,group:0x40E0_C700,instance:child});
    let mut enabled=array(unit,ENABLE[3])?.to_vec();
    if enabled.iter().filter(|v|**v==binding).count()!=1{
        return Err("COMMERCIAL_BUNDLE: mixed-capacity finalization requires exactly one owned binding".into());
    }
    // The legacy mall verifier requires its native capacity override last.
    // Host activation and private income adapters are assembled afterwards;
    // finalize this new plop path without weakening that existing contract.
    enabled.retain(|v|*v!=binding);enabled.push(binding);
    set_array_with_flags(unit,ENABLE[3],PropType::Key,enabled,0x809C);
    Ok(())
}
