//! Dump raw material payloads and decoded texture refs for one model resource.
use dbpf::{Package, ResourceId};
use rw4::{Rw4File, SectionType};

fn main() {
    let mut args = std::env::args().skip(1);
    let package = args.next().expect("package");
    let instance = u32::from_str_radix(args.next().expect("instance").trim_start_matches("0x"), 16)
        .expect("instance hex");
    let package = Package::open(std::path::Path::new(&package)).expect("open package");
    let id = ResourceId { type_id: 0x2F4E_681B, group: 0, instance };
    let entry = package.entry(id).expect("model not found");
    let bytes = package.read(entry).expect("read model");
    let file = Rw4File::parse(&bytes).expect("parse model");
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let payload = file.payload(&bytes, section.number).expect("payload");
        let refs = file.decode_material(&bytes, section.number).ok().map(|m| match m {
            rw4::MaterialSection::Decoded(m) => m.texture_refs.iter().map(|r| (r.slot_byte(), r.texture_instance)).collect::<Vec<_>>(),
            rw4::MaterialSection::Raw(_) => Vec::new(),
        }).unwrap_or_default();
        let end = payload.len().min(240);
        let hex = payload[..end].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("");
        println!("material={} payload={} refs={refs:?} head={hex}", section.number, payload.len());
    }
}
