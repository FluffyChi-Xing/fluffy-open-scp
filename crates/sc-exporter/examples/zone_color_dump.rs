//! 精确提取 EP1 的 3 个 0x00b1b104 属性资源（instance 低 16 位命中 zone id）
//! 的 0xdca011d/0xdca012d 颜色属性值。
use dbpf::Package;

const PROP_LO: u32 = 0x0dca011d;
const PROP_HI: u32 = 0x0dca012d;

fn find_props(body: &[u8], key: u32) -> Vec<Vec<u8>> {
    // 简易线性扫描：key 出现处之后 4B 长度 + 数据（属性表 value 常见布局）
    let mut out = Vec::new();
    let kb = key.to_le_bytes();
    let mut i = 0;
    while i + 8 <= body.len() {
        if &body[i..i + 4] == kb.as_slice() {
            // 尝试两种布局：[key][len u32][data] 或 [key][data...]
            let l1 = u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap()) as usize;
            if l1 > 0 && l1 <= 64 && i + 8 + l1 <= body.len() {
                out.push(body[i + 8..i + 8 + l1].to_vec());
                i += 8 + l1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn main() -> dbpf::Result<()> {
    let pkg = Package::open(r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package")?;
    let want: Vec<u32> = vec![0x03d690930, 0x03d690937, 0x0410308c0];
    for e in pkg.entries() {
        if want.contains(&e.id.instance) {
            let body = pkg.read(e)?;
            let lo = find_props(&body, PROP_LO);
            let hi = find_props(&body, PROP_HI);
            println!("== inst={:08x} grp={:08x} size={} ==", e.id.instance, e.id.group, body.len());
            for (tag, vals) in [("0xdca011d(颜色A)", &lo), ("0xdca012d(颜色B)", &hi)] {
                for v in vals {
                    println!("  {tag}: {:02x?}", v);
                    if v.len() >= 16 {
                        let f: Vec<f32> = v.chunks_exact(4).take(4)
                            .map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
                        println!("      as f32×4: {:?}", f);
                    }
                }
                if vals.is_empty() { println!("  {tag}: 无"); }
            }
        }
    }
    Ok(())
}
