//! Run the declared-stride gate against a package's property files.
//! Usage: cargo run -q -p fluffy-open-scp --example gate_check -- <package>
use dbpf::Package;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let mut files = 0;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let bytes = package.read(entry).expect("read");
        let Ok(file) = sc_properties::PropertyFile::parse(&bytes) else { continue };
        files += 1;
        match file.validate_declared_strides() {
            Ok(()) => println!("PASS {:08X}:{:08X}:{:08X}", entry.id.type_id, entry.id.group, entry.id.instance),
            Err(e) => println!("FAIL {:08X}:{:08X}:{:08X}  {e}", entry.id.type_id, entry.id.group, entry.id.instance),
        }
    }
    println!("# {files} property files checked");
}
