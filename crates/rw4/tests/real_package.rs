//! 真实包 RW4 资源全量 sweep：app.package 中全部 0x2F4E681B（RW4 包裹）
//! 资源的 header + section index 解析验证。数据不入库，缺失时跳过。
//!
//! 验收基线：header 级校验与 C# `RW4Header.Read` 同等严格（H001–H302 全检），
//! 真实游戏资源应 100% 通过；失败资源会列出 TGI 与错误类别。

use std::collections::HashMap;
use std::time::Instant;

use dbpf::Package;
use rw4::{FileType, Rw4File, SectionType};

const REAL_PACKAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/app.package"
);

const RW4_WRAPPED: u32 = 0x2F4E_681B;
const RASTER: u32 = 0x2F4E_681C;

#[test]
fn real_package_parses_all_rw4_containers() {
    let Ok(package) = Package::open(REAL_PACKAGE) else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == RW4_WRAPPED)
        .collect();
    eprintln!(
        "RW4-wrapped resources (0x{RW4_WRAPPED:08X}): {}",
        entries.len()
    );
    assert!(!entries.is_empty(), "expected RW4 resources in app.package");

    let t0 = Instant::now();
    let mut file_types: HashMap<&'static str, usize> = HashMap::new();
    let mut section_histogram: HashMap<String, usize> = HashMap::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut placeholders: Vec<String> = Vec::new();
    let mut meshes = 0usize;
    let mut textures = 0usize;
    let mut skeletons = 0usize;
    let mut anims = 0usize;

    for entry in &entries {
        let name = format!("{}", entry.id);
        let parsed = package
            .read(entry)
            .map_err(|e| (name.clone(), format!("read: {e}")))
            .and_then(|data| {
                Rw4File::parse(&data)
                    .map(|f| (f, data))
                    .map_err(|e| (name.clone(), format!("parse: {e}")))
            });
        match parsed {
            Ok((file, data)) => {
                let kind = match file.file_type() {
                    FileType::Model => "Model",
                    FileType::Texture => "Texture",
                };
                *file_types.entry(kind).or_default() += 1;
                for section in file.sections() {
                    let key = section
                        .type_name()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("0x{:08X}", section.type_code));
                    match section.type_code {
                        SectionType::MESH => meshes += 1,
                        SectionType::TEXTURE => textures += 1,
                        SectionType::RW4_SKELETON => skeletons += 1,
                        SectionType::ANIM => anims += 1,
                        _ => {}
                    }
                    // payload 切片必须全部可访问（pos/size 与文件一致）
                    if let Err(e) = file.payload(&data, section.number) {
                        failures.push((name.clone(), format!("payload: {e}")));
                    }
                    *section_histogram.entry(key).or_default() += 1;
                }
            }
            Err((name, reason)) => {
                // 0xCAFED00D 是游戏数据中的占位桩（同组 instance 0..3），
                // 不是 RW4 容器；C# `RW4Header.Read` 在此同样抛
                // "Unknown2 file type"。归类为预期跳过，其余错误零容忍。
                if reason.contains("0xCAFED00D") {
                    placeholders.push(name);
                } else {
                    failures.push((name, reason));
                }
            }
        }
    }
    let secs = t0.elapsed().as_secs_f64();

    eprintln!(
        "parsed {} RW4 resources in {secs:.3}s; file types {file_types:?}; \
         sections: {meshes} mesh / {textures} texture / {skeletons} skeleton / {anims} anim",
        entries.len()
    );
    let mut hist: Vec<_> = section_histogram.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (name, n) in &hist {
        eprintln!("  {name:24} {n}");
    }

    // 头部级校验与 C# 同严格：真实资源应全部通过（0xCAFED00D 占位桩除外，
    // 共 {} 个，C# oracle 同样无法解析）
    let raster_count = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == RASTER)
        .count();
    eprintln!(
        "(raster 0x{RASTER:08X}: {raster_count}; 0xCAFED00D placeholder stubs: {} -> {placeholders:?})",
        placeholders.len()
    );
    assert!(
        failures.is_empty(),
        "RW4 header sweep had {} failures, first few: {:?}",
        failures.len(),
        &failures[..failures.len().min(10)]
    );
    assert!(meshes > 0, "expected mesh sections across resources");
    assert!(textures > 0, "expected texture sections across resources");
}

/// MeshMaterialAssignment（0x2001A）绑定表：EP1 多 mesh 模型金样本
/// 0x41B1BAC0——2 mesh + 2 material + 2 assignment，字节实测
/// `{mesh:11, unk:1, material:12}` / `{mesh:10, unk:1, material:14}`。
#[test]
fn mesh_material_bindings_match_golden_sample() {
    const EP1: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/packages/m3/SimCityDataEP1.package"
    );
    const MODEL: u32 = 0x41B1_BAC0;
    let Ok(package) = Package::open(EP1) else {
        eprintln!("skipping: {EP1} not present");
        return;
    };
    let Some(entry) = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_WRAPPED && e.id.instance == MODEL)
        .cloned()
    else {
        panic!("golden model {MODEL:08X} missing from EP1");
    };
    let data = package.read(&entry).unwrap();
    let file = Rw4File::parse(&data).unwrap();
    assert_eq!(file.sections_of_type(SectionType::MESH).count(), 2);
    assert_eq!(file.sections_of_type(SectionType::MATERIAL).count(), 2);

    let bindings = file.decode_mesh_material_bindings(&data);
    assert_eq!(bindings.len(), 2, "2 assignments, one per material");
    assert!(
        bindings
            .iter()
            .any(|b| (b.mesh_section, b.unknown2, b.material_section) == (11, 1, 12)),
        "expected mesh#11 <-> material#12, got {bindings:?}"
    );
    assert!(
        bindings
            .iter()
            .any(|b| (b.mesh_section, b.unknown2, b.material_section) == (10, 1, 14)),
        "expected mesh#10 <-> material#14, got {bindings:?}"
    );
}
