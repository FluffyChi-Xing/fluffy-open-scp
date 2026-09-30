//! 全字典普查（N7）：decal 字典 → material → 0x2D shader-def 实例 + shaderdef 内容字符串。
//! 服务广告路由修复：判定各字典（sign/graffiti/DLC0 广告）最终落在哪个 shader 家族。
//! 用法：cargo run -p sc-exporter --release --example decal_dict_survey -- <pkg> [...]
use dbpf::Package;
use std::collections::BTreeMap;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_TYPE: u32 = 0x2F4E_681B;
const ATLAS_GROUP_TYPES: [u16; 3] = [0xb185, 0x1651, 0x1652];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let packages: Vec<(String, Package)> = paths
        .iter()
        .map(|p| {
            let pkg = Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}"));
            let name = p.rsplit(['/', '\\']).next().unwrap().to_string();
            (name, pkg)
        })
        .collect();

    // 1) 收集全部字典（按 (group, instance) 去重）
    let mut dicts: BTreeMap<(u32, u32), (String, usize, usize, Option<u32>)> = Default::default();
    for (name, pkg) in &packages {
        for e in pkg.entries() {
            if e.id.type_id != PROPERTY_TYPE || !ATLAS_GROUP_TYPES.contains(&(e.id.group as u16)) {
                continue;
            }
            let key = (e.id.group, e.id.instance);
            if dicts.contains_key(&key) {
                continue;
            }
            let Ok(data) = pkg.read(e) else { continue };
            let Ok(dict) = sc_properties::DecalDictionary::parse(&data) else { continue };
            let total = dict.entries.len();
            let colored = dict
                .entries
                .iter()
                .filter(|en| en.colors_rgba8().is_some())
                .count();
            let material = dict.material.as_ref().map(|k| k.instance);
            dicts.insert(key, (name.clone(), total, colored, material));
        }
    }

    // 2) 每个 material → 0x2D shader-def 实例
    let mut materials: Vec<u32> = dicts.values().filter_map(|(_, _, _, m)| *m).collect();
    materials.sort_unstable();
    materials.dedup();
    let mut shader_defs: BTreeMap<u32, Option<u32>> = Default::default();
    for material in &materials {
        shader_defs.insert(*material, resolve_shader_def(*material, &packages));
    }

    // 3) 输出
    println!(
        "{:<18} {:>9} {:>7} {:>12} {:>14}",
        "字典(group/inst)", "条目", "有色%", "material", "shader-def"
    );
    for ((group, instance), (name, total, colored, material)) in &dicts {
        let sd = material.and_then(|m| shader_defs.get(&m).cloned().flatten());
        println!(
            "{:06x}/{:08x} {:>9} {:>6}% {:>12} {:>14}",
            group & 0xffff,
            instance,
            total,
            if *total > 0 { colored * 100 / *total } else { 0 },
            material.map(|m| format!("{m:08X}")).unwrap_or_else(|| "-".into()),
            sd.map(|s| format!("{s:08X}")).unwrap_or_else(|| "-".into()),
        );
        let _ = name;
    }

    // 4) 每个 shader-def 资源：跨包找同 instance 条目，抽可打印字符串样本
    let sd_instances: Vec<u32> = shader_defs.values().flatten().copied().collect();
    for sd in sd_instances {
        println!("\n=== shader-def {sd:08X} 内容样本 ===");
        for (name, pkg) in &packages {
            for e in pkg.entries() {
                if e.id.instance != sd {
                    continue;
                }
                let Ok(data) = pkg.read(e) else { continue };
                let strings = printable_runs(&data, 6);
                if strings.is_empty() {
                    continue;
                }
                println!(
                    "  [{} T {:08X} G {:08X}] {}",
                    name,
                    e.id.type_id,
                    e.id.group,
                    strings
                        .iter()
                        .take(12)
                        .map(|s| format!("\"{s}\""))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
        }
    }
}

fn resolve_shader_def(material: u32, packages: &[(String, Package)]) -> Option<u32> {
    for (_, pkg) in packages {
        let entry = pkg
            .entries()
            .iter()
            .find(|e| e.id.type_id == RW4_TYPE && e.id.instance == material)
            .cloned()?;
        let data = pkg.read(&entry).ok()?;
        let file = rw4::Rw4File::parse(&data).ok()?;
        let section = file.sections_of_type(rw4::SectionType::MATERIAL).next()?.number;
        if let rw4::MaterialSection::Decoded(m) = file.decode_material(&data, section).ok()? {
            return m
                .texture_refs
                .iter()
                .find(|r| r.slot == rw4::SHADER_DEF_MARKER)
                .map(|r| r.texture_instance);
        }
    }
    None
}

fn printable_runs(data: &[u8], min: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for &b in data {
        if (0x20..0x7f).contains(&b) {
            run.push(b);
        } else {
            if run.len() >= min {
                out.push(String::from_utf8_lossy(&run).to_string());
            }
            run.clear();
        }
    }
    if run.len() >= min {
        out.push(String::from_utf8_lossy(&run).to_string());
    }
    out
}
