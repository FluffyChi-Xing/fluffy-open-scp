//! Ground household life + native apartment vacancy/migration adapters.
//! Copies rule references, not floor Units. Source/partition evidence:
//! tmp/residence-minimal-loop-20261125/{balance-source-extraction,
//! apartment-balance-boundary}.json. Player rule 0C614160 is part of the loop.
use super::{behavior_nodes::ResolvedBehavior, behavior_residence_wealth::GroundResidence,
    behavior_templates::{ENABLE,EXCLUDED,array,contains,key}, connector::Resources,
    menu, property_edit::{encode,set_array_with_flags}};
use dbpf::ResourceId;
use sc_properties::{Kind,PropType,PropertyFile,Value};
use sha2::{Digest,Sha256};

const ERROR:&str="BEHAVIOR_COPAN_RESIDENCE";
const INPUT:u32=0x0E81_6C38;
const INPUT_BYTES:&[u8]=include_bytes!("../../resources/sc5/residence-balance/cleanup-input.bin");
const LEGACY:&[u32]=&[0x999F_8484,0x999F_8485,0xA797_4BCE,0x8D9A_F497,
    0x78A7_30CD,0x78A7_30CE,0x78A7_30CF,0xEBFC_1753,0xEBFC_1750,0xEBFC_1751];
const TUTORIAL:u32=0xE238_1694;
struct Tier {
    intake_id:u32,
    intake:&'static [u8], intake_sha:&'static str,
    balance:&'static [u8], balance_sha:&'static str,
    omitted:[u32;6], counter:u32,
}
fn tier(source:&GroundResidence)->Result<Tier,String>{
    let kind=source.fields().into_iter().find_map(|(h,_,v)|
        if h==0x0D02_D846 {if let Value::Key(k)=v {Some(k.instance)}else{None}}else{None});
    Ok(match kind {
        Some(0x31D3_307B)=>Tier{
            intake_id:0x78A7_30CD,
            intake:include_bytes!("../../resources/sc5/residence-balance/78a730cd.bin"),
            intake_sha:"b394bc9a56d8561d145edf3bc22cfaa76aa03ed857acd919937b495b71801445",
            balance:include_bytes!("../../resources/sc5/residence-balance/ebfc1753.bin"),
            balance_sha:"32d1aa9fc2b28427fc08f0183abe3a0272b26cc77a7764c7aa0df5c219f3b347",
            omitted:[0x1E2B_CEE8,0xBA15_47D0,0x8E52_7F31,0x7379_91EA,0xD500_B08A,0x4E19_7A34],counter:0xF42A_61C8},
        Some(0x02CA_56B2)=>Tier{
            intake_id:0x78A7_30CE,
            intake:include_bytes!("../../resources/sc5/residence-balance/78a730ce.bin"),
            intake_sha:"5d43c5d1f534f202f1f6553907a53837afcb43ed1282dd624136f76187af26a4",
            balance:include_bytes!("../../resources/sc5/residence-balance/ebfc1750.bin"),
            balance_sha:"cf9a3fa4e4c0c81dfd7207e00d6bc89a8fa88524aa0b6b598d8122518af6fd21",
            omitted:[0xC1CF_708D,0xBA15_47D0,0x8E52_7F31,0x7379_91E9,0xFFFB_DFE3,0x92AE_0775],counter:0xFA0A_D4F5},
        Some(0x10BA_89C9)=>Tier{
            intake_id:0x78A7_30CF,
            intake:include_bytes!("../../resources/sc5/residence-balance/78a730cf.bin"),
            intake_sha:"3465417eb99eeff8a766c223590802abae9e7185d6bb65c8d1d9316c7ce4888d",
            balance:include_bytes!("../../resources/sc5/residence-balance/ebfc1751.bin"),
            balance_sha:"1232396241e19841b4ab8383e8c5ff8e1c8f1281130de4c9ec95980866760f1e",
            omitted:[0x4319_F62E,0xBA15_47D0,0x8E52_7F31,0x7379_91E8,0x6509_14DC,0x9A4D_FB96],counter:0x1796_D426},
        _=>return Err(format!("{ERROR}: unsupported household wealth")),
    })
}
fn parse(bytes:&[u8],sha:&str)->Result<PropertyFile,String>{
    if format!("{:x}",Sha256::digest(bytes))!=sha {return Err(format!("{ERROR}: source hash"));}
    PropertyFile::parse(bytes).map_err(|e|format!("{ERROR}: {e}"))
}
fn input()->Result<PropertyFile,String>{
    parse(INPUT_BYTES,"15fbec324c14eb4fd6940f9aaa69b56ac0e0bbd50c6b3af347a5de0dc6cbc513")
}
fn filtered(t:&Tier)->Result<Vec<u8>,String>{
    let mut f=parse(t.balance,t.balance_sha)?;
    // Explicitly unrelated edges: floor income, floor cost publication,
    // tower park bonus, floor registration and its matching cleanup.
    for p in &mut f.values {
        if matches!(p.hash,0x0808_0D58..=0x0808_0D5A | 0x0832_7D48..=0x0832_7D5C)
            && let Kind::Array(values)=&mut p.kind {
            values.retain(|v|!matches!(v,Value::Key(k) if t.omitted.contains(&k.instance)));
        }
    }
    let ids=array(&f,0x07E0_AD23)?;
    let count=ids.len();
    let keep:Vec<_>=ids.iter().enumerate().filter_map(|(i,v)|
        (!matches!(v,Value::Key(k) if [t.counter,0xF6FC_5D0D].contains(&k.instance))).then_some(i)).collect();
    for field in [0x07E0_AD23,0x07E0_AD24,0x0863_5E12] {
        let original=array(&f,field)?.to_vec();
        if original.len()!=count{return Err(format!("{ERROR}: source bin row count"));}
        let p=f.values.iter_mut().find(|p|p.hash==field).ok_or_else(||format!("{ERROR}: source bin column"))?;
        p.kind=Kind::Array(keep.iter().map(|&i|original[i].clone()).collect());
    }
    // The three native player callbacks remain byte-for-byte source rows.
    if f.get(0x0C61_4160)!=parse(t.balance,t.balance_sha)?.get(0x0C61_4160) {
        return Err(format!("{ERROR}: native whole-building toggle callbacks changed"));
    }
    encode(&f)
}
pub(super) fn normalize(params:&mut serde_json::Value){
    let profile=params["residenceProfile"].as_str();
    if !matches!(profile,Some("copan-residence-v1"|"megatower-residence-v1")){return;}
    let population=params["population"].as_u64().unwrap_or(1100);
    if params["population"].is_null(){params["population"]=1100.into();}
    if params["residentWorkers"].is_null()&&params["residentShoppers"].is_null(){
        let workers=(800.0*population as f64/1100.0).round() as u64;
        params["residentWorkers"]=workers.into();params["residentShoppers"]=(population-workers).into();
    }
    if params["residentChildren"].is_null(){params["residentChildren"]=((100.0*population as f64/1100.0).round() as u64).into();}
    params["residenceProfile"]=super::behavior_residence_d3::PROFILE.into();
}
pub(super) fn configure(program:&mut ResolvedBehavior)->Result<(),String>{
    if program.templates.ground_residence().is_none(){return Err(format!("{ERROR}: missing ground household parameters"));}
    let native=input()?;
    let Some(Value::Float(value))=native.get(INPUT).and_then(|p|p.scalar())else{return Err(format!("{ERROR}: native cleanup input"));};
    program.record_float(INPUT,*value as f64,"native-household-cleanup")?;
    program.remove_fields.retain(|h|*h!=INPUT);
    program.copan_residence_balance=true;
    Ok(())
}
pub(super) fn bundle_ids(child:u32)->[ResourceId;1]{
    ["native-household-balance-v1"].map(|scope|ResourceId{
        type_id:menu::PROPERTY_TYPE,group:0x40E0_C700,instance:super::behavior_unit_build::instance(child,scope)})
}
fn unit_id(child:u32)->ResourceId{ResourceId{type_id:menu::PROPERTY_TYPE,group:menu::UNIT_GROUP,instance:child}}
fn active(unit:&PropertyFile,id:u32)->bool{contains(unit,&ENABLE,id)&&!contains(unit,&[EXCLUDED],id)}
pub(super) fn apply(resources:&Resources,child:u32,program:&ResolvedBehavior)->Result<Resources,String>{
    let source=program.templates.ground_residence().ok_or_else(||format!("{ERROR}: source household"))?;
    let t=tier(source)?;parse(t.intake,t.intake_sha)?;
    let ids=bundle_ids(child);let catalog=super::behavior_conflicts::catalog()?;
    if !catalog.has_bundle(t.intake_id)||ids.iter().any(|id|id.instance==0||id.instance==child||catalog.has_bundle(id.instance)||resources.contains_key(id)) {
        return Err(format!("{ERROR}: private recipe identity collision"));
    }
    let mut unit=PropertyFile::parse(resources.get(&unit_id(child)).ok_or_else(||format!("{ERROR}: Unit"))?).map_err(|e|e.to_string())?;
    for row in ENABLE {
        let mut values=array(&unit,row)?.to_vec();
        values.retain(|v|!matches!(v,Value::Key(k) if LEGACY.contains(&k.instance)||k.instance==TUTORIAL));
        if row==ENABLE[3]{
            values.push(key(t.intake_id));
            // Owned resources require their full TGI, unlike stock script IDs.
            // This is also the edge used by identity-closure before writing.
            values.extend(ids.map(|id|Value::Key(sc_properties::Key{
                type_id:id.type_id,group:id.group,instance:id.instance})));
        }
        // Native property layout represents an absent enable tier by omitting
        // the column, never by serializing a count=0 array after partitioning.
        if values.is_empty(){unit.values.retain(|p|p.hash!=row);}
        else {set_array_with_flags(&mut unit,row,PropType::Key,values,0x809C);}
    }
    let mut excluded=array(&unit,EXCLUDED)?.to_vec();
    excluded.retain(|v|!matches!(v,Value::Key(k) if k.instance==t.intake_id));
    for id in LEGACY.iter().copied().chain([TUTORIAL]).filter(|id|*id!=t.intake_id) {
        if !excluded.contains(&key(id)){excluded.push(key(id));}
    }
    set_array_with_flags(&mut unit,EXCLUDED,PropType::Key,excluded,0x809C);
    let additions:Resources=[(ids[0],filtered(&t)?),(unit_id(child),encode(&unit)?)].into_iter().collect();
    Ok(additions)
}
pub(super) fn verify(resources:&Resources,child:u32,program:&ResolvedBehavior)->Result<(),String>{
    let source=program.templates.ground_residence().ok_or_else(||format!("{ERROR}: source household"))?;
    if program.templates.land_value(){return Err(format!("{ERROR}: household template must have its own scope"));}
    let unit=PropertyFile::parse(resources.get(&unit_id(child)).ok_or_else(||format!("{ERROR}: Unit"))?).map_err(|e|e.to_string())?;
    if unit.values.iter().any(|p|matches!(&p.kind,Kind::Array(values) if values.is_empty())) {
        return Err(format!("{ERROR}: empty native property array"));
    }
    let t=tier(source)?;let ids=bundle_ids(child);
    for (id,expected) in ids.into_iter().zip([filtered(&t)?]) {
        if !active(&unit,id.instance)||resources.get(&id)!=Some(&expected){return Err(format!("{ERROR}: missing or changed native recipe"));}
    }
    let catalog=super::behavior_conflicts::catalog()?;
    for id in source.bundles().filter(|id|!LEGACY.contains(id)&&*id!=TUTORIAL).chain([t.intake_id]) {
        let count=ENABLE.into_iter().try_fold(0,|n,h|Ok::<_,String>(n+array(&unit,h)?.iter().filter(|v|matches!(v,Value::Key(k) if k.instance==id)).count()))?;
        if !catalog.has_bundle(id)||!active(&unit,id)||count!=1{return Err(format!("{ERROR}: missing household core {id:08X}"));}
    }
    for id in source.excluded().chain(LEGACY.iter().copied()).chain([TUTORIAL]).filter(|id|*id!=t.intake_id) {
        if contains(&unit,&ENABLE,id)||!contains(&unit,&[EXCLUDED],id){return Err(format!("{ERROR}: incompatible intake/lifecycle {id:08X}"));}
    }
    for (hash,kind,value) in source.fields() {
        if unit.get(hash).is_none_or(|p|p.prop_type!=kind||p.kind!=Kind::Scalar(value.clone())) {
            return Err(format!("{ERROR}: household parameter {hash:08X}"));
        }
    }
    if unit.get(INPUT)!=input()?.get(INPUT)||unit.get(0x0FC6_E602).is_some()
        ||unit.get(0x07E0_AD09).is_some(){return Err(format!("{ERROR}: cleanup input or unrelated floor finance/replacement"));}
    let flags=array(&unit,0x0A88_D179)?;
    if !flags.contains(&Value::Int32(22))||flags.iter().any(|v|matches!(v,Value::Int32(11|13|14|15))) {
        return Err(format!("{ERROR}: missing native vacancy or inherited construction/failure state"));
    }
    super::behavior_copan_base::verify_component(&unit)?;
    super::behavior_copan_base::verify_residence_component(&unit)?;
    Ok(())
}

// vehicle_paths::bind already installs this path for the native ground
// return sinks. The native apartment intake uses the same per-wealth mover
// sink as the ground stock record; include that actual, now-private source.
fn intake_sink(program:&ResolvedBehavior)->Result<Value,String>{
    let t=tier(program.templates.ground_residence().ok_or_else(||format!("{ERROR}: source household"))?)?;
    let f=parse(t.intake,t.intake_sha)?;
    let sinks=array(&f,0x0A27_25CE)?;
    if sinks.len()!=1{return Err(format!("{ERROR}: native intake sink row count"));}
    Ok(sinks[0].clone())
}
fn instance(value:&Value)->Option<u32>{if let Value::Key(k)=value{Some(k.instance)}else{None}}
pub(super) fn bind_vehicle_arrival(unit:&[u8],lot:&[u8],program:&ResolvedBehavior)->Result<(Vec<u8>,Vec<u8>),String>{
    let original=PropertyFile::parse(unit).map_err(|e|e.to_string())?;
    let t=tier(program.templates.ground_residence().ok_or_else(||format!("{ERROR}: source household"))?)?;
    if !active(&original,t.intake_id){return Err(format!("{ERROR}: vehicle binding requires the actual native intake"));}
    let (unit,lot)=super::vehicle_paths::bind(unit,lot)?;
    let mut lot=PropertyFile::parse(&lot).map_err(|e|e.to_string())?;
    let sink=intake_sink(program)?;
    let mut sinks=array(&lot,0x0CFF_E03D)?.to_vec();
    let mut paths=array(&lot,0x0CFE_C186)?.to_vec();
    let mut directions=array(&lot,0x0D1A_7636)?.to_vec();
    let row=sinks.iter().position(|v|instance(v)==instance(&sink)).unwrap_or_else(||{
        sinks.push(sink);paths.push(key(0x0310_6C2E));
        if !directions.is_empty(){directions.push(key(0));}
        sinks.len()-1
    });
    paths[row]=key(0x0310_6C2E);
    if !directions.is_empty(){directions[row]=key(0);}
    set_array_with_flags(&mut lot,0x0CFF_E03D,PropType::Key,sinks,0x809C);
    set_array_with_flags(&mut lot,0x0CFE_C186,PropType::Key,paths,0x809C);
    if !directions.is_empty(){set_array_with_flags(&mut lot,0x0D1A_7636,PropType::Key,directions,0x809C);}
    Ok((unit,encode(&lot)?))
}
pub(super) fn verify_vehicle_arrival(lot:&[u8],program:&ResolvedBehavior)->Result<(),String>{
    let lot=PropertyFile::parse(lot).map_err(|e|e.to_string())?;
    let sink=intake_sink(program)?;
    let sinks=array(&lot,0x0CFF_E03D)?;let paths=array(&lot,0x0CFE_C186)?;let dirs=array(&lot,0x0D1A_7636)?;
    if sinks.len()!=paths.len()||(!dirs.is_empty()&&dirs.len()!=sinks.len())
        ||!sinks.iter().zip(paths).enumerate().any(|(row,(id,path))|instance(id)==instance(&sink)&&instance(path)==Some(0x0310_6C2E)&&dirs.get(row).and_then(instance)!=Some(26822469)) {
        return Err(format!("{ERROR}: missing native move-in vehicle arrival"));
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn three_wealths_preserve_native_intake_and_player_callbacks_without_floor_context(){
        for profile in ["ground-residence-w1-v1","ground-residence-w2-v1","ground-residence-w3-v1"] {
            let source=GroundResidence::select(&serde_json::json!({"residenceProfile":profile,"population":600}),"home").unwrap().unwrap();
            let t=tier(&source).unwrap();let raw=parse(t.balance,t.balance_sha).unwrap();
            let clean=PropertyFile::parse(&filtered(&t).unwrap()).unwrap();
            assert_eq!(array(&clean,0x07E0_AD23).unwrap().len(),9);
            assert_eq!(clean.get(0x0C61_4160),raw.get(0x0C61_4160));
            for p in &clean.values {if matches!(p.hash,0x0808_0D58..=0x0808_0D5A|0x0832_7D48..=0x0832_7D5C) {
                assert!(!array(&clean,p.hash).unwrap().iter().any(|v|matches!(v,Value::Key(k) if t.omitted.contains(&k.instance))));
            }}
        }
    }
    #[test]
    fn legacy_aliases_keep_authored_capacity_and_role_counts(){
        let mut p=serde_json::json!({"residenceProfile":"copan-residence-v1","population":2200});normalize(&mut p);
        assert_eq!(p["residentWorkers"],1600);assert_eq!(p["residentShoppers"],600);assert_eq!(p["residentChildren"],200);
        let mut p=serde_json::json!({"residenceProfile":"megatower-residence-v1","population":600,"residentWorkers":500,"residentShoppers":100});normalize(&mut p);
        assert_eq!(p["residentWorkers"],500);assert_eq!(p["residentShoppers"],100);
    }
}
