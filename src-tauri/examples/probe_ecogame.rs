//! Enumerate all entries of a package (group/type filter optional).
//! Usage: ... -- <package> [hex-group] [hex-type]
use dbpf::Package;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let group: Option<u32> = args
        .get(2)
        .map(|g| u32::from_str_radix(g.trim_start_matches("0x"), 16).unwrap());
    let want_type: Option<u32> = args
        .get(3)
        .map(|t| u32::from_str_radix(t.trim_start_matches("0x"), 16).unwrap());
    let mut count = 0usize;
    for entry in package.entries() {
        if let Some(g) = group {
            if entry.id.group != g {
                continue;
            }
        }
        if let Some(t) = want_type {
            if entry.id.type_id != t {
                continue;
            }
        }
        let size = package.read(entry).map(|b| b.len()).unwrap_or(0);
        println!(
            "{:08X}:{:08X}:{:08X}  {}B",
            entry.id.type_id, entry.id.group, entry.id.instance, size
        );
        count += 1;
    }
    println!("# {count} entries");
}
