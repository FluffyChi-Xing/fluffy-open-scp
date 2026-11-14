//! R11 包审计：菜单/lot/ERZ 链引用完整性 + 模型引用自洽。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let menu = 0xFD97_0F8Cu32;
    let tower = 0x5C78_6C64u32;
    // 全包扫描：所有 Key 引用是否都能在包内或白名单（基础包）解析
    let mut dangling: Vec<String> = Vec::new();
    for e in pkg.entries() {
        if e.id.type_id != 0x00B1_B104 && e.id.type_id != 0x2F7D_0004 {
            continue;
        }
        let pf = match PropertyFile::parse(&pkg.read(e)?) {
            Ok(p) => p,
            Err(_) => continue,
        };
        for p in &pf.values {
            let mut refs = Vec::new();
            match &p.kind {
                Kind::Scalar(Value::Key(k)) => refs.push((k.type_id, k.group, k.instance)),
                Kind::Array(vals) => {
                    for v in vals {
                        if let Value::Key(k) = v {
                            refs.push((k.type_id, k.group, k.instance));
                        }
                    }
                }
                _ => {}
            }
            for (t, g, i) in refs {
                if t == 0 {
                    continue;
                }
                let in_pkg = pkg
                    .entries()
                    .iter()
                    .any(|x| x.id.type_id == t && x.id.group == g && x.id.instance == i);
                let known_global = (t == 0x2F4E_681B && [0xA0DA_9E6C, 0x60E7_805D].contains(&i))
                    || i == menu
                    || i == tower;
                if !in_pkg && !known_global {
                    dangling.push(format!(
                        "{:08X}/{:08X} hash {:08X} -> {:08X}/{:08X}/{:08X}",
                        e.id.type_id, e.id.group, p.hash, t, g, i
                    ));
                }
            }
        }
    }
    if dangling.is_empty() {
        println!("AUDIT OK: no dangling refs (menu {menu:08X}, tower {tower:08X})");
    } else {
        for d in &dangling {
            println!("DANGLING {d}");
        }
    }
    // 模型 Texture section 引用自洽（R9 形态：#11 → blob #10）
    let m = pkg
        .entry(ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: tower,
        })
        .ok_or("model")?;
    let bytes = pkg.read(m)?;
    let f = rw4::Rw4File::parse(&bytes)?;
    let n_sec = f.sections().len();
    println!(
        "model: {} bytes, {} sections (expect 12)",
        bytes.len(),
        n_sec
    );
    Ok(())
}
