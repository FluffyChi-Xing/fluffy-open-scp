//! Native park visits, without a park facility or a MegaTower floor identity.
//! 29F0E345 supplies visitor capacities; 59292C11 uses the same visit providers.
//! Public services and the player-facing lifecycle belong to the host.
use super::{behavior_nodes::ResolvedBehavior, behavior_templates::{array,contains,key,ENABLE,EXCLUDED},
    connector::Resources, menu, property_edit::{encode,set_array_with_flags}};
use dbpf::ResourceId;
use sc_properties::{Key,PropType,PropertyFile,Value};
use sha2::{Digest,Sha256};

const ERROR:&str="BEHAVIOR_PARK_CORE";
const INPUTS:&[u8]=include_bytes!("../../resources/sc5/park-core/native-capacities.bin");
const SHA:&str="319826bcd6e863897a1c94ed9fc621dee78d19527776e1a169a14ffa3d44d0c2";
pub(super) const BUNDLES:[u32;7]=[0x6EA8_E701,0x265F_B8DD,0x265F_B8DE,0x265F_B8DF,
    0xEA02_54BF,0x7187_A49D,0x5FFA_86B9];

fn inputs()->Result<PropertyFile,String>{
    if format!("{:x}",Sha256::digest(INPUTS))!=SHA{return Err(format!("{ERROR}: native capacity source hash"));}
    PropertyFile::parse(INPUTS).map_err(|e|format!("{ERROR}: {e}"))
}

pub(super) fn compile(params:&serde_json::Value)->Result<ResolvedBehavior,String>{
    // Do not run the legacy park donor/land-value family: it owns an entire
    // facility and incidentally relied on school-like fields for children.
    let mut program=super::behavior_nodes::compile_with_profile(None,&[],false)?;
    for property in inputs()?.values{
        let Some(Value::Float(value))=property.scalar() else{return Err(format!("{ERROR}: native input type"));};
        program.set_float_fields.insert(property.hash,*value as f64);
        program.declared_unit_columns.push(property.hash);
        program.remove_fields.retain(|h|*h!=property.hash);
    }
    for (name,fields) in [("residentCapacity",&[0x0AD6_A725][..]),
        ("childrenCapacity",&[0x0AD7_FAC9,0x0B98_805A,0x0B98_805D][..]),
        ("visitBudget",&[0x0DA3_EE15][..])]{
        if let Some(value)=params.get(name).filter(|v|!v.is_null()){
            let value=value.as_u64().filter(|v|(1..=100_000).contains(v))
                .ok_or_else(||format!("{ERROR}: invalid {name}"))?;
            for &field in fields{program.set_float_fields.insert(field,value as f64);}
        }
    }
    for id in BUNDLES{
        if !super::behavior_conflicts::catalog()?.has_bundle(id){return Err(format!("{ERROR}: missing native bundle {id:08X}"));}
        program.ensure_bundles.push(id);
        program.remove_excluded_bundles.push(id);
        program.remove_bundles.retain(|v|*v!=id);
    }
    program.copan_park_core=true;
    program.ensure_pedestrian_path=true;
    program.notes.push("native park core: adult/kid/homeless visits and return happiness; land value is a separate leaf".into());
    Ok(program)
}

pub(super) fn opening_id(child:u32)->ResourceId{
    ResourceId{type_id:menu::PROPERTY_TYPE,group:0x40E0_C700,
        instance:super::behavior_unit_build::instance(child,"native-park-opening-mode-v1")}
}
fn full(id:ResourceId)->Value{Value::Key(Key{type_id:id.type_id,group:id.group,instance:id.instance})}
fn opening()->Result<Vec<u8>,String>{
    let mut file=PropertyFile::default();
    // Exact 5D1EC901 row: OpeningHours cap=3/init=1 (6AM-6PM park mode).
    // Keep its native input for 6EA8E701's opening/closing continuations,
    // without importing the facility workforce or its utility controller.
    for (hash,ty,values) in [
        (0x07E0_AD23,PropType::Key,vec![key(0x4B16_C93D)]),
        (0x07E0_AD24,PropType::Int32,vec![Value::Int32(3)]),
        (0x0863_5E12,PropType::Int32,vec![Value::Int32(1)])]{
        set_array_with_flags(&mut file,hash,ty,values,0x809C);
    }
    encode(&file)
}
pub(super) fn apply(unit:&mut PropertyFile,child:u32)->Result<Resources,String>{
    let id=opening_id(child);let mut enabled=array(unit,ENABLE[3])?.to_vec();
    if enabled.contains(&full(id)){return Err(format!("{ERROR}: duplicate opening provider"));}
    enabled.push(full(id));set_array_with_flags(unit,ENABLE[3],PropType::Key,enabled,0x809C);
    let mut excluded=array(unit,EXCLUDED)?.to_vec();
    for id in [0x5D1E_C901,0x8BA6_D35D]{if !excluded.contains(&key(id)){excluded.push(key(id));}}
    set_array_with_flags(unit,EXCLUDED,PropType::Key,excluded,0x809C);
    Ok([(id,opening()?)].into_iter().collect())
}
pub(super) fn verify(resources:&Resources,unit:&PropertyFile,child:u32,program:&ResolvedBehavior)->Result<(),String>{
    for id in BUNDLES{
        if !contains(unit,&ENABLE,id)||contains(unit,&[EXCLUDED],id){return Err(format!("{ERROR}: missing visitor provider {id:08X}"));}
    }
    for property in inputs()?.values{
        let expected=program.set_float_fields.get(&property.hash).map(|v|Value::Float(*v as f32));
        if expected.is_none()||unit.get(property.hash).and_then(|p|p.scalar())!=expected.as_ref(){
            return Err(format!("{ERROR}: missing or changed native input {:08X}",property.hash));
        }
    }
    let id=opening_id(child);
    if resources.get(&id)!=Some(&opening()?) || !array(unit,ENABLE[3])?.contains(&full(id)){
        return Err(format!("{ERROR}: missing native opening mode"));
    }
    for id in [0x5D1E_C901,0xE153_69A2,0x75D2_2FD0,0x75D2_2FD1,0x75D2_2FD2,0x8BA6_D35D]{
        if contains(unit,&ENABLE,id)&&!contains(unit,&[EXCLUDED],id){return Err(format!("{ERROR}: unrelated facility/business {id:08X}"));}
    }
    if !super::vehicle_paths::has_supported_arrivals(&encode(unit)?)?{return Err(format!("{ERROR}: park vehicle arrivals missing"));}
    Ok(())
}
