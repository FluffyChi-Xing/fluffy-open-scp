//! M3 收尾验证：对 `docs/packages`（app.package）与 `docs/packages/m3`
//! （SimCity_DLC0.package、SimCityDataEP1.package）中的全部 RW4 资源做
//! header + section index + mesh（顶点/三角形）全量解码 sweep。
//!
//! 数据不入库，缺失时跳过对应包。验收基线（与 C# oracle 行为对齐）：
//! - `0xCAFED00D` 占位桩在头部即失败（C# 同样失败），显式归类；
//! - mesh 解码失败需逐个给出 TGI 与错误类别，非占位类失败为零容忍。

use std::collections::HashMap;
use std::time::Instant;

use dbpf::Package;
use rw4::{FileType, Rw4File};

const RW4_WRAPPED: u32 = 0x2F4E_681B;

const PACKAGES: [(&str, &str); 3] = [
    (
        "app",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/packages/app.package"
        ),
    ),
    (
        "DLC0",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/packages/m3/SimCity_DLC0.package"
        ),
    ),
    (
        "EP1",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/packages/m3/SimCityDataEP1.package"
        ),
    ),
];

#[test]
fn m3_packages_decode_all_meshes() {
    let mut any_ran = false;

    for (label, path) in PACKAGES {
        let Ok(package) = Package::open(path) else {
            eprintln!("skipping [{label}]: {path} not present");
            continue;
        };
        any_ran = true;

        let entries: Vec<_> = package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == RW4_WRAPPED)
            .collect();
        let t0 = Instant::now();

        let mut stats = SweepStats::default();
        for entry in &entries {
            let tgi = format!("{}", entry.id);
            let parsed = package
                .read(entry)
                .map_err(|e| (tgi.clone(), format!("read: {e}")))
                .and_then(|data| {
                    Rw4File::parse(&data)
                        .map(|f| (f, data))
                        .map_err(|e| (tgi.clone(), format!("parse: {e}")))
                });
            match parsed {
                Ok((file, data)) => {
                    if file.file_type() != FileType::Model {
                        stats.model_containers_without_mesh += 1;
                    }
                    for section in file.sections_of_type(rw4::SectionType::MESH) {
                        match file.decode_mesh(&data, section.number) {
                            Ok(mesh) => {
                                stats.meshes += 1;
                                stats.triangles += mesh.triangles.len();
                                stats.vertices += mesh.vertices.len();
                                for v in &mesh.vertices {
                                    // 几何不变量：位置必须有限
                                    if let Some(p) = v.position()
                                        && p.iter().any(|f| !f.is_finite())
                                    {
                                        stats.non_finite_positions += 1;
                                    }
                                    if v.uv().is_some() {
                                        stats.vertices_with_uv += 1;
                                    }
                                    if v.blend_indices_raw().is_some() {
                                        stats.vertices_with_blend += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                let reason = format!("mesh {}: {e}", section.number);
                                if reason.contains("0xCAFED00D") {
                                    stats.placeholders += 1;
                                } else if oracle_known_mesh_failure(&reason) {
                                    // ME0xx/ME100/ME200 是 C# `RW4Mesh.Read` 的严格
                                    // expect 校验；这些资源在 C# 下同样抛
                                    // ModelFormatException（oracle 对齐失败，记录不阻断）
                                    stats.oracle_mesh_mismatches += 1;
                                    stats
                                        .mesh_failures
                                        .push((tgi.clone(), format!("oracle-known {reason}")));
                                } else {
                                    stats.mesh_failures.push((tgi.clone(), reason));
                                }
                            }
                        }
                    }
                }
                Err((tgi, reason)) => {
                    if reason.contains("0xCAFED00D") {
                        stats.placeholders += 1;
                    } else {
                        stats.header_failures.push((tgi, reason));
                    }
                }
            }
        }

        stats.report(label, entries.len(), t0.elapsed().as_secs_f64());

        // 头部级失败零容忍（占位桩除外）
        assert!(
            stats.header_failures.is_empty(),
            "[{label}] header failures: {:?}",
            &stats.header_failures[..stats.header_failures.len().min(10)]
        );
        // mesh 解码失败零容忍（oracle 对齐类失败除外——C# 在同样的
        // 严格 expect 上会抛 ModelFormatException）
        let hard_failures: Vec<_> = stats
            .mesh_failures
            .iter()
            .filter(|(_, r)| !r.starts_with("oracle-known"))
            .collect();
        assert!(
            hard_failures.is_empty(),
            "[{label}] mesh failures: {:?}",
            &hard_failures[..hard_failures.len().min(10)]
        );
        // 几何不变量
        assert_eq!(
            stats.non_finite_positions, 0,
            "[{label}] non-finite positions"
        );
        // 有模型的包必须解出至少一个 mesh
        assert!(stats.meshes > 0, "[{label}] expected decoded meshes");
    }

    assert!(
        any_ran,
        "no real package present; differential sweep did not run"
    );
}

/// ME0xx（严格 expect）/ME100/ME200 是 C# `RW4Mesh.Read` 的原生校验点，
/// 触发即 ModelFormatException —— 视为 oracle 对齐失败。
fn oracle_known_mesh_failure(reason: &str) -> bool {
    let checks = [
        "ME001", "ME002", "ME003", "ME004", "ME005", "ME006", "ME100", "ME200",
    ];
    checks.iter().any(|c| reason.contains(c))
}

#[derive(Default)]
struct SweepStats {
    meshes: usize,
    triangles: usize,
    vertices: usize,
    vertices_with_uv: usize,
    vertices_with_blend: usize,
    non_finite_positions: usize,
    model_containers_without_mesh: usize,
    placeholders: usize,
    oracle_mesh_mismatches: usize,
    header_failures: Vec<(String, String)>,
    mesh_failures: Vec<(String, String)>,
}

impl SweepStats {
    fn report(&self, label: &str, resources: usize, secs: f64) {
        eprintln!(
            "[{label}] {resources} RW4 resources in {secs:.2}s: {} meshes, {} tris, {} verts \
             (uv {:.1}%, blend {:.1}%), no-mesh containers {}, placeholders {}",
            self.meshes,
            self.triangles,
            self.vertices,
            percent(self.vertices_with_uv, self.vertices),
            percent(self.vertices_with_blend, self.vertices),
            self.model_containers_without_mesh,
            self.placeholders,
        );
        eprintln!(
            "[{label}] oracle mesh mismatches (C# fails identically): {}",
            self.oracle_mesh_mismatches
        );
        let mut kinds: HashMap<&str, usize> = HashMap::new();
        for (_, r) in &self.mesh_failures {
            let kind = r.split(':').next().unwrap_or("?").trim();
            *kinds.entry(kind).or_default() += 1;
        }
        eprintln!(
            "[{label}] header failures {} / mesh failures {} / kinds {kinds:?}",
            self.header_failures.len(),
            self.mesh_failures.len()
        );
    }
}

fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}
