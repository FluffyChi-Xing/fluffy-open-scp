fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("package root"));
    fn collect(d:&std::path::Path, out:&mut Vec<std::path::PathBuf>) { if let Ok(es)=std::fs::read_dir(d) { for e in es.flatten(){ let p=e.path(); if p.is_dir(){collect(&p,out)} else if p.extension().is_some_and(|x|x=="package"){out.push(p)} } } }
    let mut ps=Vec::new(); collect(&root,&mut ps);
    for path in ps { let Ok(pkg)=dbpf::Package::open(&path) else {continue}; for e in pkg.entries().iter().filter(|e|e.id.type_id==0x00B1_B104 && e.id.group==0x40E1_C000 && e.id.instance==0x3036_0939) { let Ok(b)=pkg.read(e) else{continue}; let Ok(p)=sc_properties::PropertyFile::parse(&b) else{continue}; println!("{}",path.display()); for h in [0x0CAA_680D,0x0CAA_6841,0x0CB0_0ED8,0x0CC8_FE7A] { if let Some(v)=p.get(h){println!("{h:08X} {:?}",v.array().or_else(||v.scalar().map(std::slice::from_ref)));} } return; } }
}
