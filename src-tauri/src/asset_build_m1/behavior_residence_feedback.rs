//! UI metadata is part of the residential closure, not a school Lot parent.
//! Copan Graphics 40E1C000:4DC31F9B, payload SHA-256 864206b0afa7b64bfd9f3ffa304547672b1b355e131e7a51066f4c493cdb533f.
//! The same column is inherited by all three native ground residential donors.
use sc_properties::PropertyFile;
use sha2::{Digest,Sha256};
const SOURCE:&[u8]=include_bytes!("../../resources/sc5/residential-feedback-ui.bin");
const SHA:&str="f82e5393c61ae98fac2da77eb36e5b22668e95d25839e9078f2db96eb5e68a50";
const FIELD:u32=0x0D5A_1674;
fn source()->Result<PropertyFile,String>{
    if format!("{:x}",Sha256::digest(SOURCE))!=SHA {return Err("BEHAVIOR_COPAN_BASE: residential feedback source hash".into());}
    PropertyFile::parse(SOURCE).map_err(|e|e.to_string())
}
pub(super) fn patch(bytes:&[u8])->Result<Vec<u8>,String>{
    let mut lot=PropertyFile::parse(bytes).map_err(|e|e.to_string())?;
    lot.values.retain(|p|p.hash!=FIELD);
    lot.values.extend(source()?.values);
    super::property_edit::encode(&lot)
}
pub(super) fn verify(lot:&PropertyFile)->Result<(),String>{
    if lot.get(FIELD)!=source()?.get(FIELD) {return Err("BEHAVIOR_COPAN_BASE: residential feedback metadata missing or changed".into());}
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn adds_only_the_native_feedback_column_and_rejects_its_loss(){
        let empty=super::super::property_edit::encode(&PropertyFile::default()).unwrap();
        let mut lot=PropertyFile::parse(&patch(&empty).unwrap()).unwrap();
        verify(&lot).unwrap();assert_eq!(lot.values.len(),1);
        assert_eq!(lot.get(FIELD).unwrap().scalar(),Some(&sc_properties::Value::String8("TextFeedbackResidentialW1Rollover.json".into())));
        lot.values.clear();assert!(verify(&lot).is_err());
    }
}
