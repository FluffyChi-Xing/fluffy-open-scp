//! Business-only MegaTower crown tourism, without the tower Unit or services.
//! Native 61EFC700:02E94AD8 -> ECFC5DF8 refreshes publication and souvenirs
//! every 30 simulation ticks; 75D22FD0..2 instead follow daily opening times.
use super::{behavior_nodes::ResolvedBehavior, behavior_templates::{array,contains,ENABLE,EXCLUDED},
    property_edit::encode};
use sc_properties::{PropertyFile,Value};
use sha2::{Digest,Sha256};
pub(super) const BUNDLE:u32=0x02E9_4AD8;
const SOURCE:&[u8]=include_bytes!("../../resources/sc5/tourism-crown/02e94ad8.bin");
const SHA:&str="025f3c54c1ccbb31f2638288f1aed59e873fb233f9292ba2fffc345b955ba836";
const ERROR:&str="BEHAVIOR_TOURISM_CROWN";
fn source()->Result<PropertyFile,String>{
    if format!("{:x}",Sha256::digest(SOURCE))!=SHA{return Err(format!("{ERROR}: native source hash"));}
    PropertyFile::parse(SOURCE).map_err(|e|format!("{ERROR}: {e}"))
}
pub(super) fn configure(program:&mut ResolvedBehavior)->Result<(),String>{
    // Hotel/retail compositions retain their proven original shared-pool route.
    if program.native_tourism.is_none() || program.ensure_bundles.contains(&super::behavior_hotel::INTAKE){return Ok(());}
    source()?;
    if !super::behavior_conflicts::catalog()?.has_bundle(BUNDLE){return Err(format!("{ERROR}: native bundle absent from catalog"));}
    for id in super::behavior_tourism::TOURIST_BUNDLES {
        program.ensure_bundles.retain(|v|*v!=id);
        program.remove_excluded_bundles.retain(|v|*v!=id);
        if !program.remove_bundles.contains(&id){program.remove_bundles.push(id);}
    }
    program.remove_bundles.retain(|v|*v!=BUNDLE);
    if !program.ensure_bundles.contains(&BUNDLE){program.ensure_bundles.push(BUNDLE);}
    if !program.remove_excluded_bundles.contains(&BUNDLE){program.remove_excluded_bundles.push(BUNDLE);}
    program.copan_crown_tourism=true;
    program.notes.push("native-crown-tourism: 02E94AD8/ECFC5DF8; no floor context or utility consumer".into());
    Ok(())
}
pub(super) fn verify(unit:&PropertyFile,program:&ResolvedBehavior)->Result<(),String>{
    source()?;
    if !contains(unit,&ENABLE,BUNDLE) || contains(unit,&[EXCLUDED],BUNDLE){return Err(format!("{ERROR}: missing native continuous publisher"));}
    for id in super::behavior_tourism::TOURIST_BUNDLES {
        if contains(unit,&ENABLE,id){return Err(format!("{ERROR}: daily publisher/schedule remains {id:08X}"));}
    }
    if !super::vehicle_paths::has_supported_arrivals(&encode(unit)?)? {
        return Err(format!("{ERROR}: native tourist vehicle receivers not registered"));
    }
    // Authored attraction, wealth quotas, shared shopper pool and souvenir
    // capacity remain the same inputs as the existing tourism profile.
    for field in [0x0E54_2F39,0x0D8C_29F6,0x0D8C_29FE,0x0D8C_2A01,0x0AD6_A725,0x0E26_60C6] {
        let expected=program.set_float_fields.get(&field).ok_or_else(||format!("{ERROR}: missing authored input {field:08X}"))?;
        if unit.get(field).and_then(|p|p.scalar())!=Some(&Value::Float(*expected as f32)){
            return Err(format!("{ERROR}: changed authored input {field:08X}"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn native_business_bundle_has_no_tower_finance_or_external_service_ownership(){
        let s=source().unwrap();assert_eq!(encode(&s).unwrap(),SOURCE);
        assert_eq!(array(&s,0x07E0_AD23).unwrap().len(),21);
        assert!(matches!(array(&s,0x0808_0D58).unwrap(),[Value::Key(k)] if k.instance==0xECFC_5DF8));
        assert_eq!(array(&s,0x0A27_25CE).unwrap().len(),9);
        assert_eq!(array(&s,0x0C61_4160).unwrap().len(),5);
        for forbidden in [0x8E8B_4E10,0x8839_8207,0x9E3C_3DFA,0xCE46_1A05,0xF6FC_5D0D]{
            assert!(!array(&s,0x07E0_AD23).unwrap().iter().any(|v|matches!(v,Value::Key(k) if k.instance==forbidden)));
        }
    }
    #[test]
    fn copan_tourism_uses_crown_publisher_without_changing_legacy_or_authored_inputs(){
        let nodes=[json!({"id":"visitors","type":"tourism","params":{"tourismProfile":"native-tourism-v1","attraction":700,"quotaW1":200,"quotaW2":150,"quotaW3":100,"shoppers":450}})];
        let legacy=super::super::behavior_nodes::compile(None,&nodes).unwrap();
        assert!(!legacy.copan_crown_tourism);
        let program=super::super::behavior_copan_base::compile_building(None,&nodes).unwrap();
        let leaf=&program.units[0].program;assert!(leaf.copan_crown_tourism);
        assert!(leaf.ensure_bundles.contains(&BUNDLE));
        for id in super::super::behavior_tourism::TOURIST_BUNDLES{assert!(!leaf.ensure_bundles.contains(&id));assert!(leaf.remove_bundles.contains(&id));}
        for h in [0x0E54_2F39,0x0D8C_29F6,0x0D8C_29FE,0x0D8C_2A01,0x0AD6_A725,0x0E26_60C6]{assert_eq!(leaf.set_float_fields.get(&h),legacy.set_float_fields.get(&h));}
    }
    #[test]
    fn gate_accepts_absent_legacy_rows_and_rejects_stale_publishers_or_changed_inputs(){
        use super::super::{behavior_templates::key,property_edit::{set_array_with_flags,set_scalar}};
        use sc_properties::PropType;
        let nodes=[json!({"id":"visitors","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}})];
        let program=super::super::behavior_copan_base::compile_building(None,&nodes).unwrap();
        let leaf=&program.units[0].program;let mut unit=PropertyFile::default();
        set_array_with_flags(&mut unit,ENABLE[3],PropType::Key,vec![key(BUNDLE)],0x809C);
        for (&h,&value) in &leaf.set_float_fields{set_scalar(&mut unit,h,PropType::Float,Value::Float(value as f32));}
        verify(&unit,leaf).unwrap();
        let mut stale=unit.clone();set_array_with_flags(&mut stale,ENABLE[3],PropType::Key,vec![key(BUNDLE),key(0xE153_69A2)],0x809C);
        assert!(verify(&stale,leaf).unwrap_err().contains("daily publisher"));
        set_scalar(&mut unit,0x0E54_2F39,PropType::Float,Value::Float(1.0));
        assert!(verify(&unit,leaf).unwrap_err().contains("changed authored input"));
    }
}
