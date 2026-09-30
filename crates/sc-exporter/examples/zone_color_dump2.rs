//! dump 三个 EP1 属性资源全文（hex+可读），人工判读属性布局。
use dbpf::Package;

fn main() -> dbpf::Result<()> {
    let pkg = Package::open(r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package")?;
    let want: Vec<u32> = vec![0x03d690930, 0x03d690937, 0x0410308c0];
    for e in pkg.entries() {
        if want.contains(&e.id.instance) && e.id.type_id == 0x00b1b104 {
            let body = pkg.read(e)?;
            println!("== inst={:08x} grp={:08x} size={} ==", e.id.instance, e.id.group, body.len());
            for (i, ch) in body.chunks(16).take(50).enumerate() {
                let hexs: Vec<String> = ch.iter().map(|b| format!("{b:02x}")).collect();
                let asc: String = ch.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
                println!("  {:04x}  {:<47}  {}", i * 16, hexs.join(" "), asc);
            }
        }
    }
    Ok(())
}
