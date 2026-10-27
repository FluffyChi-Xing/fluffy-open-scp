//! Build a non-destructive Cape Trinity bootstrap candidate. Unit transforms
//! are read from a known empty-region EGB for calibration; path geometry itself
//! comes from static package Unit properties. This is deliberately a probe
//! until template placement is recovered from static region data.
use sc_properties::{PropertyFile, region_state::{RegionState, StateMap, StateUnit, curves_from_static_unit, write_state_gzip}};
use serde::Deserialize;
#[derive(Deserialize)] struct Input { version:u32, environment:u32, maps:Vec<StateMapIn>, units:Vec<StateUnit>, curves:Vec<serde_json::Value> }
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct StateMapIn { id:u32, size:usize, values:Vec<i32> }
fn main(){
 let root=std::path::PathBuf::from(std::env::args().nth(1).expect("SimCity root")); let input_path=std::path::PathBuf::from(std::env::args().nth(2).expect("state json")); let out=std::path::PathBuf::from(std::env::args().nth(3).unwrap_or_else(||"tmp/cape-trinity-bootstrap".into())); std::fs::create_dir_all(&out).unwrap();
 let input:Input=serde_json::from_slice(&std::fs::read(&input_path).unwrap()).unwrap();
 fn collect(d:&std::path::Path,o:&mut Vec<std::path::PathBuf>){if let Ok(es)=std::fs::read_dir(d){for e in es.flatten(){let p=e.path();if p.is_dir(){collect(&p,o)}else if p.extension().is_some_and(|x|x=="package"){o.push(p)}}}}
 let mut paths=vec![]; collect(&root,&mut paths);
 let mut props=std::collections::HashMap::<u32,PropertyFile>::new();
 for path in paths { let Ok(pkg)=dbpf::Package::open(&path) else{continue}; for e in pkg.entries().iter().filter(|e|e.id.type_id==0x00B1_B104&&e.id.group==0x40E1_C000){ if let Ok(b)=pkg.read(e){if let Ok(p)=PropertyFile::parse(&b){props.entry(e.id.instance).or_insert(p);}} } }
 let mut curves=Vec::new(); let mut matched=0usize;
 for u in &input.units { if let Some(p)=props.get(&u.id){ let cs=curves_from_static_unit(p,u); if !cs.is_empty(){matched+=1;curves.extend(cs);} } }
 let state=RegionState{version:20,environment:input.environment,maps:input.maps.into_iter().map(|m|StateMap{id:m.id,size:m.size,values:m.values}).collect(),curves,units:input.units};
 let bytes=write_state_gzip(&state).expect("write bootstrap"); std::fs::write(out.join("state_file_bootstrap.egb"),bytes).unwrap(); std::fs::write(out.join("static-road-report.json"),serde_json::to_vec_pretty(&state).unwrap()).unwrap(); println!("wrote {} units with static paths; curves={} -> {}",matched,state.curves.len(),out.display());
}
