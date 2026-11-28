//! A business component follows its connected host's native ActiveBuilding.
//! Native rule 4D20B4A5 (472644E9 -> 0ECCE71E) already tests connected
//! ActiveBuilding and runs the original activation/deactivation continuations.
//! The host alone evaluates utility failures. No new binary rule library,
//! transport, consumer, inventory, demand, or global counter is installed.
use super::{connector::Resources, menu, property_edit::{encode,set_array_with_flags},
    behavior_templates::{array,ENABLE,EXCLUDED,key},behavior_unit_build::instance};
use dbpf::ResourceId;
use sc_properties::{Key,PropType,PropertyFile,Value};
const ERROR:&str="BEHAVIOR_UTILITY_STATE";
const RULE:u32=0x4D20_B4A5;
const ACTIVE:u32=0x7A19_A243;
const FACILITY:u32=0x3064_5625;
pub(super) fn resources(child:u32)->[ResourceId;1]{
    [ResourceId{type_id:menu::PROPERTY_TYPE,group:0x40E0_C700,instance:instance(child,"connected-host-activation-v1")}]
}
fn full(id:ResourceId)->Value{Value::Key(Key{type_id:id.type_id,group:id.group,instance:id.instance})}
fn bundle()->Result<Vec<u8>,String>{
    let mut file=PropertyFile::default();
    // Exact ActiveBuilding row from native 30645625: capacity 1, initial 0.
    // Its facility controller stays on the host. The component uses the
    // native connected-host controller instead of independently self-activating.
    for (h,ty,values) in [(0x07E0_AD23,PropType::Key,vec![key(ACTIVE)]),
        (0x07E0_AD24,PropType::Int32,vec![Value::Int32(1)]),
        (0x0863_5E12,PropType::Int32,vec![Value::Int32(0)]),
        (0x0808_0D58,PropType::Key,vec![key(RULE)])] {
        set_array_with_flags(&mut file,h,ty,values,0x809C);
    }
    encode(&file)
}
pub(super) fn apply(unit:&mut PropertyFile,child:u32)->Result<Resources,String>{
    let id=resources(child)[0];let mut enabled=array(unit,ENABLE[3])?.to_vec();
    if enabled.contains(&full(id)){return Err(format!("{ERROR}: duplicate adapter"));}
    enabled.push(full(id));set_array_with_flags(unit,ENABLE[3],PropType::Key,enabled,0x809C);
    let mut excluded=array(unit,EXCLUDED)?.to_vec();
    if !excluded.contains(&key(FACILITY)){excluded.push(key(FACILITY));}
    set_array_with_flags(unit,EXCLUDED,PropType::Key,excluded,0x809C);
    Ok([(id,bundle()?)].into_iter().collect())
}
pub(super) fn verify(all:&Resources,unit:&PropertyFile,child:u32)->Result<(),String>{
    let id=resources(child)[0];
    if ENABLE.into_iter().try_fold(0,|n,h|array(unit,h).map(|a|n+a.iter().filter(|v|**v==full(id)).count()))?!=1
        || all.get(&id)!=Some(&bundle()?) || !array(unit,EXCLUDED)?.contains(&key(FACILITY)) {
        return Err(format!("{ERROR}: missing, duplicated or changed native connected-host controller"));
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn connected_controller_contains_only_native_activation_and_one_real_state_bin(){
        let f=PropertyFile::parse(&bundle().unwrap()).unwrap();
        assert_eq!(f.values.len(),4);
        assert_eq!(array(&f,0x0808_0D58).unwrap(),&[key(0x4D20_B4A5)]);
        assert_eq!(array(&f,0x07E0_AD23).unwrap(),&[key(0x7A19_A243)]);
        assert_eq!(array(&f,0x07E0_AD24).unwrap(),&[Value::Int32(1)]);
        assert_eq!(array(&f,0x0863_5E12).unwrap(),&[Value::Int32(0)]);
    }
}
