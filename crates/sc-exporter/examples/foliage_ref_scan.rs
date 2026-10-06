//! 冒烟工具：全包扫 2F4E681B 模型的材质纹理引用，找引用 foliage 图集族的模型
//! （树 impostor 离屏渲染源若存在，必经材质引用树叶图集）。
//! 用法：cargo run -p sc-exporter --release --example foliage_ref_scan -- <pkg> [...]
use dbpf::Package;
use std::collections::BTreeSet;

const RW4_MODEL: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // foliage 图集族实例（find_foliage_textures 落盘的 23 个 + 已知 835D64F3）
    let foliage: BTreeSet<u32> = [
        "0E63E58D", "11B872D6", "1CE63F46", "23585701", "2A1CB524", "471DF5D0", "4DF43690",
        "521E5A3E", "5E54DA5D", "700B073C", "7B500736", "8185B1E8", "835D64F3", "8C741B99",
        "925B45E9", "9469EFD2", "B393F9BB", "B393F9BC", "B393F9BE", "BFC8B47E", "C812A88C",
        "F68D9682", "FCC00CE7",
    ]
    .iter()
    .map(|s| u32::from_str_radix(s, 16).unwrap())
    .collect();
    for path in &args {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        let mut scanned = 0usize;
        for e in package.entries().iter().filter(|e| e.id.type_id == RW4_MODEL) {
            let Ok(data) = package.read(e) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
            let mut tex_refs: BTreeSet<u32> = BTreeSet::new();
            for mat in file.sections_of_type(rw4::SectionType::MATERIAL) {
                if let Ok(rw4::MaterialSection::Decoded(m)) =
                    file.decode_material(&data, mat.number)
                {
                    for r in &m.texture_refs {
                        tex_refs.insert(r.texture_instance);
                    }
                }
            }
            scanned += 1;
            let hits: Vec<u32> = tex_refs.intersection(&foliage).cloned().collect();
            if !hits.is_empty() {
                println!(
                    "{path}: I {:08X} {}B 命中 {:?}",
                    e.id.instance,
                    data.len(),
                    hits.iter().map(|h| format!("{h:08X}")).collect::<Vec<_>>()
                );
            }
        }
        eprintln!("{path}: 已扫 {scanned} 模型");
    }
}
