//! One stock Game.js plus deterministically composed, named ES5 patches.
//! Versioned receipts permit a newer compatible shared runtime to serve older
//! buildings. A differing opaque script is never merged or silently accepted.
use super::{components, sources, runtime_registry::{Patch,Registry}};
use serde::{Deserialize,Serialize};
use std::collections::{BTreeMap,BTreeSet};

pub const COMPILER_VERSION:&str="openscp.panel-bundle/1";
const MARKER:&str="\n/* OpenSCPPanelBundleReceipt:";
const END:&str=" */\n";
const LEGACY:&[&str]=&[
    // Actual installed payloads, independently pinned on 2026-11-25.
    "edb782c6422f55e76bd780ea0bb99c7d6bc29536ccb3a9c3bdfa63a2e2409065",
    "4183ca597731ddbb5662d319794f7deb571a86cfca63bf75d038d31d1ae3faff",
];
// The 34 native bindings used by these legacy panels have identical ES5 ASTs
// across these two bases. No other game version inherits this migration.
const LEGACY_BASES:&[&str]=&[
    "64663354fabd086c7dd8e02cc2f9ad317f0a1de5332ec0fddd173bf283771f37",
    "683c66fe0d0146b9b630bbe59e1df89848fc05d907f3028147c01ff4f407431b",
];
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PatchReceipt {
    id:String, owner:String, generation:u64, view:String, slot:String,
    start:usize, end:usize, sha256:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Receipt {
    schema:String, abi:String, base_len:usize, base_sha256:String,
    body_sha256:String, capabilities:BTreeMap<String,u32>,
    bindings:BTreeSet<u32>, patches:Vec<PatchReceipt>,
}
fn capabilities(stock:&[u8])->BTreeMap<String,u32>{
    let mut result:BTreeMap<_,_>=["bindings","tower-operations","container-tourists","container-shoppers","component-queries","park-component-queries"]
        .into_iter().map(|name|(format!("openscp.panel.{name}/1"),1)).collect();
    if LEGACY_BASES.contains(&sources::digest(stock).as_str()) {
        result.insert("openscp.panel.legacy-20261124/1".into(),1);
    }
    result
}
fn bindings()->BTreeSet<u32>{
    let mut names=vec!["OpenSCPPanel_headerNone".to_string(),"OpenSCPPanel_empty".into(),
        "OpenSCPPanel_containerTourists".into(),"OpenSCPPanel_containerShoppers".into()];
    for c in components() {
        if c.feedback {for suffix in ["_hide","_native"]{names.push(format!("OpenSCPPanel_{}{suffix}",c.id));}}
        else {names.push(format!("OpenSCPPanel_{}",c.id));}
    }
    for id in ["residence-feedback_hide","residence-feedback_native","residents","land-value-residential",
        "residential-happiness-reasons","residential-sadness-reasons","tourism-revenue",
        "park-feedback_hide","park-feedback_native","park-hours","park-visitors-title","park-visitors",
        "hotel-occupancy"] {
        names.push(format!("OpenSCPPanel_component_{id}"));
    }
    names.iter().map(|n|super::hash(n)).collect()
}
fn patches()->Result<Registry,String>{
    let mut registry=Registry::default();
    let catalog=format!("\n;window.OpenSCPPanelComponents={};window.OpenSCPPanelMessages={};\n",
        serde_json::to_string(&components()).map_err(|e|e.to_string())?,super::MESSAGES);
    for (index,(id,source,requires)) in [
        ("catalog",catalog,vec![]),
        ("tower-operations",format!("{}\n",super::tower_operations::RUNTIME),vec!["catalog"]),
        ("bindings",super::RUNTIME.into(),vec!["catalog","tower-operations"]),
        ("container-tourists",format!("\n;{}",super::container_contract::RUNTIME),vec!["bindings"]),
        ("container-shoppers",format!("\n;{}",super::container_shoppers::RUNTIME),vec!["bindings"]),
        ("component-queries",format!("\n;{}",super::component_queries::RUNTIME),vec!["bindings"]),
    ].into_iter().enumerate(){
        registry.register(Patch{id:format!("openscp.{id}"),owner:"openscp".into(),generation:1,
            view:"ingame.building-details".into(),slot:id.into(),priority:index as i32,
            requires:requires.iter().map(|s|format!("openscp.{s}")).collect(),source})?;
    }
    Ok(registry)
}
pub fn read_package(path:&std::path::Path)->Result<Vec<u8>,String>{
    let package=dbpf::Package::open(path).map_err(|e|e.to_string())?;
    let entry=package.entry(sources::GAME_JS).ok_or_else(||format!("PANEL_RUNTIME_MISSING:{}",path.display()))?;
    package.read(entry).map_err(|e|e.to_string())
}
pub fn required_source_matches(stock:&[u8],required:&[u8])->bool{
    required.len()>stock.len() && (required.starts_with(stock)
        || (LEGACY.contains(&sources::digest(required).as_str())
            && LEGACY_BASES.contains(&sources::digest(stock).as_str())))
}

pub(crate) fn verify_source(stock: &[u8], runtime: &[u8]) -> Result<Receipt, String> {
    let receipt = inspect(runtime)?;
    if !runtime.starts_with(stock) || receipt.base_len != stock.len()
        || receipt.base_sha256 != sources::digest(stock)
        || receipt.abi != super::manifest::RUNTIME_VERSION
    {
        return Err("PANEL_RUNTIME_SOURCE".into());
    }
    Ok(receipt)
}

pub(crate) fn verify_bindings(stock: &[u8], runtime: &[u8], required: &BTreeSet<u32>) -> Result<(), String> {
    let receipt = verify_source(stock, runtime)?;
    if !required.is_subset(&receipt.bindings) {
        return Err("PANEL_RUNTIME_CAPABILITY".into());
    }
    Ok(())
}
pub fn compose(stock:&[u8])->Result<Vec<u8>,String>{
    let mut body=stock.to_vec();let mut records=vec![];
    let registry=patches()?;
    for patch in registry.compose()? {
        let start=body.len();body.extend_from_slice(patch.source.as_bytes());
        records.push(PatchReceipt{id:patch.id.clone(),owner:patch.owner.clone(),generation:patch.generation,
            view:patch.view.clone(),slot:patch.slot.clone(),start,end:body.len(),sha256:sources::digest(patch.source.as_bytes())});
    }
    let receipt=Receipt{schema:COMPILER_VERSION.into(),abi:super::manifest::RUNTIME_VERSION.into(),
        base_len:stock.len(),base_sha256:sources::digest(stock),body_sha256:sources::digest(&body),
        capabilities:capabilities(stock),bindings:bindings(),patches:records};
    body.extend_from_slice(MARKER.as_bytes());
    body.extend_from_slice(serde_json::to_string(&receipt).map_err(|e|e.to_string())?.as_bytes());
    body.extend_from_slice(END.as_bytes());
    inspect(&body)?;
    Ok(body)
}
pub fn inspect(runtime:&[u8])->Result<Receipt,String>{
    let invalid=||"PANEL_RUNTIME_CONTRACT".to_string();
    let text=std::str::from_utf8(runtime).map_err(|_|invalid())?;
    let (body,json)=text.rsplit_once(MARKER).ok_or_else(invalid)?;
    let receipt:Receipt=serde_json::from_str(json.strip_suffix(END).ok_or_else(invalid)?)
        .map_err(|_|invalid())?;
    if receipt.schema!=COMPILER_VERSION || receipt.base_len>body.len()
        || sources::digest(&body.as_bytes()[..receipt.base_len])!=receipt.base_sha256
        || sources::digest(body.as_bytes())!=receipt.body_sha256 {return Err(invalid());}
    let mut cursor=receipt.base_len;let mut ids=BTreeSet::new();let mut slots=BTreeSet::new();
    for p in &receipt.patches {
        if p.start!=cursor || p.end<=p.start || p.end>body.len() || p.generation==0
            || !p.id.starts_with(&format!("{}.",p.owner)) || !ids.insert(&p.id)
            || !slots.insert((&p.view,&p.slot))
            || sources::digest(&body.as_bytes()[p.start..p.end])!=p.sha256 {return Err(invalid());}
        cursor=p.end;
    }
    if cursor!=body.len() {return Err(invalid());}
    Ok(receipt)
}
/// The installed runtime must satisfy every required API, not merely resemble
/// its JavaScript text. Legacy matching is only allowed as a required input:
/// an opaque old runtime can never stand in for a newer required bundle.
pub fn compatible(stock:&[u8],required:&[u8],installed:&[u8])->Result<(),String>{
    let legacy=LEGACY.contains(&sources::digest(required).as_str());
    if !required_source_matches(stock,required) || installed.len()<=stock.len() || !installed.starts_with(stock) {
        return Err("PANEL_RUNTIME_SOURCE".into());
    }
    if required==installed {
        if std::str::from_utf8(required).is_ok_and(|s|s.contains(MARKER)){inspect(required)?;}
        return Ok(());
    }
    let actual=inspect(installed)?;
    if actual.base_len!=stock.len() || actual.base_sha256!=sources::digest(stock)
        || actual.abi!=super::manifest::RUNTIME_VERSION {return Err("PANEL_RUNTIME_CONTRACT".into());}
    let needed=match inspect(required) {
        Ok(needed)=>needed,
        Err(_) if legacy=>{
            if actual.capabilities.get("openscp.panel.legacy-20261124/1")!=Some(&1) {
                return Err("PANEL_RUNTIME_CAPABILITY: legacy-20261124".into());
            }
            return Ok(());
        },
        Err(e)=>return Err(e),
    };
    if needed.abi!=actual.abi || needed.base_sha256!=actual.base_sha256
        || !needed.bindings.is_subset(&actual.bindings)
        || needed.capabilities.iter().any(|(k,v)|actual.capabilities.get(k)!=Some(v)) {
        return Err("PANEL_RUNTIME_CAPABILITY".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_source_preserving_bundle_rejects_corruption_and_unrelated_overlays(){
        let stock=b"stock";let runtime=compose(stock).unwrap();
        assert_eq!(runtime,compose(stock).unwrap());
        let r=inspect(&runtime).unwrap();assert_eq!(r.patches.len(),6);
        assert!(!r.capabilities.contains_key("openscp.panel.legacy-20261124/1"));
        assert!(runtime.starts_with(stock));compatible(stock,&runtime,&runtime).unwrap();
        let mut corrupt=runtime.clone();corrupt[stock.len()+20]^=1;
        assert!(inspect(&corrupt).is_err());
        assert!(compatible(stock,&corrupt,&corrupt).is_err());
        assert!(compatible(stock,&runtime,b"stock other script").is_err());
        assert!(compatible(b"other",&runtime,&runtime).is_err());
    }
    #[test]
    fn compatible_superset_serves_older_contract_but_missing_binding_and_abi_fail(){
        let stock=b"stock";let runtime=compose(stock).unwrap();
        let (body,_)=std::str::from_utf8(&runtime).unwrap().rsplit_once(MARKER).unwrap();
        let mut older=inspect(&runtime).unwrap();older.capabilities.remove("openscp.panel.component-queries/1");
        older.bindings.pop_last();
        let encode=|r:&Receipt|format!("{body}{MARKER}{}{END}",serde_json::to_string(r).unwrap()).into_bytes();
        compatible(stock,&encode(&older),&runtime).unwrap();
        assert!(compatible(stock,&runtime,&encode(&older)).is_err());
        older.abi="unknown/2".into();assert!(compatible(stock,&encode(&older),&runtime).is_err());
    }
}
