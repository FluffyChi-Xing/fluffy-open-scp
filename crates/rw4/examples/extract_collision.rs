use dbpf::Package;
use rw4::{Rw4File, SectionType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
 let p=Package::open(std::env::args().nth(1).unwrap())?;
 let e=p.entries().iter().find(|e| e.id.type_id==0x2F4E681B && e.id.instance==0x06C5470C).unwrap();
 let b=p.read(e)?; let f=Rw4File::parse(&b)?; let s=f.sections_of_type(SectionType::COLLISION_MESH).next().unwrap(); let x=f.payload(&b,s.number)?;
 std::fs::write("tmp/nh_collision.bin",x)?; println!("len={}",x.len()); Ok(())
}
