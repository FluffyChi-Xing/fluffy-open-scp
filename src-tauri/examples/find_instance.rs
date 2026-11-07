//! Find every entry with a given instance id across a package (any type/group).
//! Usage: cargo run -q -p fluffy-open-scp --example find_instance -- <package> <hex-instance>
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let want = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open package");
    for entry in package.entries() {
        if entry.id.instance == want {
            println!(
                "{:08X}:{:08X}:{:08X}  {} B",
                entry.id.type_id, entry.id.group, entry.id.instance,
                package.read(entry).map(|b| b.len()).unwrap_or(0)
            );
        }
    }
}
