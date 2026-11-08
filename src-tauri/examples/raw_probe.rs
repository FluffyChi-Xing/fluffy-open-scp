use dbpf::{Package, ResourceId};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let want = u32::from_str_radix(args[3].trim_start_matches("0x"), 16).unwrap();
    let id = ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    };
    let bytes = package.read(package.entry(id).expect("e")).expect("r");
    let hb = want.to_be_bytes();
    let mut hits = Vec::new();
    for i in 0..bytes.len().saturating_sub(4) {
        if bytes[i] == hb[0]
            && bytes[i + 1] == hb[1]
            && bytes[i + 2] == hb[2]
            && bytes[i + 3] == hb[3]
        {
            hits.push(i);
        }
    }
    println!("hash byte occurrences: {hits:?}");
    for &h in &hits {
        if h + 20 > bytes.len() {
            continue;
        }
        let ftype = u16::from_be_bytes([bytes[h + 4], bytes[h + 5]]);
        let flags = u16::from_be_bytes([bytes[h + 6], bytes[h + 7]]);
        if ftype != 56 {
            continue;
        }
        let count = u32::from_be_bytes([bytes[h + 8], bytes[h + 9], bytes[h + 10], bytes[h + 11]]);
        let stride =
            i32::from_be_bytes([bytes[h + 12], bytes[h + 13], bytes[h + 14], bytes[h + 15]]);
        println!("at {h}: ftype=56 flags={flags:#06x} count={count} stride={stride}");
        let start = h + 16;
        let end = (start + count as usize * stride.unsigned_abs() as usize).min(bytes.len());
        let mut starts = Vec::new();
        let mut i = start;
        while i + 1 < end {
            if bytes[i] == 0 && bytes[i + 1] == 0x0E {
                starts.push(i - start);
            }
            i += 1;
        }
        println!(
            "flags14 markers at payload offsets: {:?} (payload {}B)",
            &starts[..starts.len().min(15)],
            end - start
        );
        return;
    }
}
