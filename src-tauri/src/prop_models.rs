//! Lot prop 模型解析（P2 精细替换后端）：
//! lot prop 列 resourceID（0x0C12EF20 族）→ RW4 模型 TGI + 所在包 id。
//!
//! 解析链（docs/design/pe-component-replacement.md §3，消防局 0x4DE9912B
//! 全部 10 个 prop id 实证，docs/re/dev-dump-lot-placement-and-script-resources.md）：
//! 1. resource 记录 = B1B104 属性表，主战场在 `SimCityUserData/EcoGame/*.package`
//!    （服务器下发脚本包，G 40E0C100 族），安装目录 40E1Cxxx 族互补；
//! 2. 模型两分支（引擎 cGraphicsResource::FillFromProps 逐字）：
//!    ① 显式 key 0x0D8C29C3 → 模型包装记录 → 0x0D897169（Vehicle Models，
//!       直接 RW4 key 数组，type 0x2F4E681B，多 LOD）或 0x0C36D30D →
//!       树种 descriptor（0x0E0B99FD 变体模型 key 数组）；
//!    ② self-key 标记（0x00F9EFBB/0x0D897169/0x0C36D30D 在记录内）→
//!       模型从记录自身/Parent（0x00B2CCCB）链的 Vehicle Models 取。
//!
//! EcoGame 包按需自注册进 PackageManager（前端随即可用 read_lot_model_meshes
//! 加载其中的模型）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use dbpf::{IndexEntry, Package};
use sc_properties::{Kind, PropertyFile, ParseLimits, Value};
use serde::Serialize;

use crate::package_service::{PackageManager, PackageError, TgiDto};

const PROPERTY_RESOURCE_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
/// 显式模型 key（cGraphicsResource 分支 1）。
const KEY_MODEL_EXPLICIT: u32 = 0x0D8C_29C3;
/// Vehicle Models → RW4 模型 key 数组（直接可用）。
const KEY_VEHICLE_MODELS: u32 = 0x0D89_7169;
/// self-key 标记之一（引擎：含任一标记 → 用记录自身 key 当模型 key）。
const KEY_LOD1: u32 = 0x00F9_EFBB;
/// 树 LOD 序号（0x0D8C29CF，uint32 0..3）——与 KEY_TREE_STATE 共存 =
/// cGraphicsInstancedImpostor 触发签名（引擎 FillFromProps 尾段逐字）。
const KEY_LOD_INDEX: u32 = 0x0D8C_29CF;
const KEY_TREE_STATE: u32 = 0x0C36_D30D;
const KEY_PARENT: u32 = 0x00B2_CCCB;
/// 树种 descriptor 的变体模型 key 数组（App 包 G 40002D00 记录）。
const KEY_TREE_VARIANTS: u32 = 0x0E0B_99FD;
/// 树 impostor 源 3D 模型（descriptor C602CD31 → Parent 5F804D7E 配置表
/// 0BD62577 列，App 包 G 40002D00；模型实体在 Graphics 包 T 2F4E681B）。
/// 引擎 RenderOffscreenForModel 把它们离屏渲染成 128×128 公告板——本地
/// 直出 3D 即超越引擎原生精度（docs/re/props-effects-spawners-paths-engine-flow.md §1.6）。
const TREE_PART_MODELS: [u32; 4] = [0x4DF4_3690, 0xC2FB_D178, 0x1113_B131, 0x89D6_58DF];
/// 资源记录的典型 group 低 16 位（EcoGame resources / 安装包 resources）。
const RESOURCE_GROUP_LOW: [u16; 2] = [0xC100, 0xC600];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPropModel {
    pub resource_id: u32,
    /// 候选模型（变体/LOD，按解析序）；前端按 prop 序号确定性挑选。
    pub models: Vec<TgiDto>,
    /// models[0] 所在的已打开包 id（EcoGame 包已自注册）。
    pub package_id: Option<u64>,
    /// 命中路径（explicit / vehicle_models / lod1_direct / tree_model / tree）。
    pub source: String,
    /// 树公告板图集 PNG（base64）：Graphics 包树图集 RW4（0x835D64F3）上半
    /// 256×256 = 2×2 四棵树公告板。仅树 prop 下发（source=tree_model/tree），
    /// 前端 3D 模型加载失败时回落公告板渲染。
    pub tree_atlas_png: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropModelsRequest {
    pub resource_ids: Vec<u32>,
}

fn values_of<'a>(file: &'a PropertyFile, hash: u32) -> Vec<&'a Value> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(vs) => vs.iter().collect(),
            Kind::Empty => vec![],
        })
        .collect()
}

fn first_key(file: &PropertyFile, hash: u32) -> Option<(u32, u32, u32)> {
    values_of(file, hash).into_iter().find_map(|v| match v {
        Value::Key(k) => Some((k.instance, k.type_id, k.group)),
        _ => None,
    })
}

fn key_list(file: &PropertyFile, hash: u32) -> Vec<(u32, u32, u32)> {
    values_of(file, hash)
        .into_iter()
        .filter_map(|v| match v {
            Value::Key(k) => Some((k.instance, k.type_id, k.group)),
            _ => None,
        })
        .collect()
}

fn parse_record(package: &Package, entry: &IndexEntry) -> Option<PropertyFile> {
    let data = package.read(entry).ok()?;
    PropertyFile::parse_with_limits(&data, ParseLimits::default()).ok()
}

/// 从已打开包的路径推导 EcoGame 脚本包目录（`<root>/SimCityUserData/EcoGame`）。
fn ecogame_dirs(packages: &[(u64, Arc<Package>)]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for (_, pkg) in packages {
        let path = pkg.path();
        let Some(data_dir) = path.parent() else { continue };
        for root in [
            data_dir.parent().map(|p| p.to_path_buf()),
            data_dir.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()),
        ]
        .into_iter()
        .flatten()
        {
            let candidate = root.join("SimCityUserData").join("EcoGame");
            if candidate.is_dir() && !dirs.contains(&candidate) {
                dirs.push(candidate);
            }
        }
    }
    dirs
}

/// EcoGame 目录中的 .package 按家族分组取**最新版本**：
/// 文件名形如 `SimCity-Scripts_287520926.package`（家族_版本号，服务器
/// 多次下发的版本化副本）——同一实例 id 在多版本中内容不同，全部入池会
/// 令跨包“首个命中”查找随机命中旧版（lot 渲染概率异常的根因），且撑爆
/// PackageManager 的 32 包上限（too many open packages）。
fn ecogame_packages(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut latest: std::collections::BTreeMap<String, (u64, PathBuf)> = Default::default();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("package") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|e| e.to_str()) else {
            continue;
        };
        // 家族 = 最后一个 '_' 前缀；尾段 = 版本号（解析失败按 0）
        let Some((family, version)) = stem.rsplit_once('_') else {
            continue;
        };
        let version = version.parse::<u64>().unwrap_or(0);
        let slot = latest
            .entry(family.to_string())
            .or_insert_with(|| (version, path.clone()));
        if version > slot.0 {
            slot.0 = version;
            slot.1 = path.clone();
        }
    }
    latest.into_values().map(|(_v, path)| path).collect()
}

/// 把 EcoGame 包注册进 manager（跳过已按路径打开的；尊重 32 包上限——
/// 预留 2 个槽位给用户手动开包，余量不足时跳过注册）。
fn ensure_ecogame_registered(
    manager: &PackageManager,
    dirs: &[PathBuf],
) -> Result<Vec<(u64, Arc<Package>)>, PackageError> {
    let mut registered = Vec::new();
    let open_count = manager.all_packages_with_ids()?.len();
    let budget = crate::package_service::MAX_OPEN_PACKAGES.saturating_sub(open_count + 2);
    for dir in dirs {
        for path in ecogame_packages(dir) {
            if registered.len() >= budget {
                return Ok(registered);
            }
            if manager.is_path_open(&path) {
                continue;
            }
            let Ok(canonical) = std::fs::canonicalize(&path) else {
                continue;
            };
            if manager.is_path_open(&canonical) {
                continue;
            }
            if let Ok(package) = Package::open(&canonical) {
                if let Ok((id, package)) = manager.insert(package) {
                    registered.push((id, package));
                }
            }
        }
    }
    Ok(registered)
}

/// 跨包找 B1B104 记录（返回解析后的属性表 + 所在包 id）。
/// EcoGame 资源组优先（group 低 16 位 ∈ RESOURCE_GROUP_LOW），其次任意。
fn find_records<'a>(
    packages: &'a [(u64, Arc<Package>)],
    instance: u32,
) -> Vec<(u64, PropertyFile, u32)> {
    let mut hits: Vec<(u64, PropertyFile, u32)> = Vec::new();
    for (id, pkg) in packages {
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_RESOURCE_TYPE && e.id.instance == instance
        }) {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            hits.push((*id, file, entry.id.group));
        }
    }
    // 资源组形态的记录排前（同 instance 多 group 复制时取语义正确者）
    hits.sort_by_key(|(_, _, group)| {
        (RESOURCE_GROUP_LOW.contains(&(*group as u16)), *group)
    });
    hits
}

/// 单个 resourceID 的解析（两分支 + 载具定义 + 树 descriptor）。
fn resolve_one(
    packages: &[(u64, Arc<Package>)],
    resource_id: u32,
) -> Option<ResolvedPropModel> {
    let records = find_records(packages, resource_id);
    if records.is_empty() {
        return None;
    }
    let mut source = "explicit";
    let mut wrapper_keys: Vec<(u32, u32, u32)> = Vec::new();

    for (_, file, group) in &records {
        // 分支 0（最短路径）：LOD1 key 直引 RW4 模型（0x00F9EFBB，type
        // 0x2F4E681B）——普查中 22+8 个 prop（长椅/垃圾桶等杂件）走这条
        let lod1 = key_list(file, KEY_LOD1);
        if !lod1.is_empty() {
            if let Some(done) = finish(packages, resource_id, lod1, "lod1_direct") {
                return Some(done);
            }
        }
        // 分支 2（先查更具体者）：记录自带 Vehicle Models = 载具定义直出
        let vehicle = key_list(file, KEY_VEHICLE_MODELS);
        if !vehicle.is_empty() {
            return finish(packages, resource_id, vehicle, "vehicle_models");
        }
        // 分支 1：显式模型 key
        if let Some(k) = first_key(file, KEY_MODEL_EXPLICIT) {
            wrapper_keys.push(k);
            let _ = group;
            continue;
        }
        // self-key 标记：模型挂在 Parent 链的 Vehicle Models
        let has_marker = [KEY_LOD1, KEY_VEHICLE_MODELS, KEY_TREE_STATE]
            .iter()
            .any(|h| !values_of(file, *h).is_empty());
        if has_marker {
            if let Some((p_inst, _, _)) = first_key(file, KEY_PARENT) {
                for (_, parent_file, _) in find_records(packages, p_inst) {
                    let models = key_list(&parent_file, KEY_VEHICLE_MODELS);
                    if !models.is_empty() {
                        return finish(packages, resource_id, models, "selfkey_parent");
                    }
                }
            }
            source = "selfkey";
        }
    }

    // 包装记录：0x0D897169 直出 / 树标记
    for (w_inst, _w_type, _w_group) in &wrapper_keys {
        for (_, file, _) in find_records(packages, *w_inst) {
            let models = key_list(&file, KEY_VEHICLE_MODELS);
            if !models.is_empty() {
                return finish(packages, resource_id, models, source);
            }
            // 树签名（引擎 cGraphicsInstancedImpostor 触发条件逐字：
            // 0x0C36D30D descriptor + 0x0D8C29CF LOD 序号共存）。
            // 模型树路线（§1.6）：descriptor→Parent 5F804D7E 配置表的 4 个
            // impostor 源 3D 模型本地存在（Graphics 包）→ source="tree_model"
            // 下发模型 key，前端真 3D 渲染；公告板图集仍随载荷下发供回落。
            if has_prop(&file, KEY_TREE_STATE) && has_prop(&file, KEY_LOD_INDEX) {
                let tree_atlas_png = decode_tree_atlas_base64(packages);
                if let Some(mut done) = finish(
                    packages,
                    resource_id,
                    TREE_PART_MODELS.iter().map(|i| (*i, RW4_MODEL_TYPE, 0)).collect(),
                    "tree_model",
                ) {
                    done.tree_atlas_png = tree_atlas_png;
                    return Some(done);
                }
                return Some(ResolvedPropModel {
                    resource_id,
                    models: Vec::new(),
                    package_id: None,
                    source: "tree".into(),
                    tree_atlas_png,
                });
            }
            if let Some((d_inst, _, _)) = first_key(&file, KEY_TREE_STATE) {
                for (_, desc_file, _) in find_records(packages, d_inst) {
                    let variants = key_list(&desc_file, KEY_TREE_VARIANTS);
                    if !variants.is_empty() {
                        return finish(packages, resource_id, variants, "descriptor");
                    }
                }
            }
        }
    }
    None
}

fn has_prop(file: &PropertyFile, hash: u32) -> bool {
    !values_of(file, hash).is_empty()
}

/// 树公告板图集：Graphics 包 RW4 0x835D64F3（256×512）上半 256×256 =
/// 2×2 四棵树公告板（128×256/格）。解顶层 mip → 裁上半 → PNG → base64。
/// 引擎同图集：impostor 图集为运行时离屏渲染产物，此 RW4 为其静态源
/// （绿占比扫描 95% 命中，四树公告板目视确认）。
fn decode_tree_atlas_base64(packages: &[(u64, Arc<Package>)]) -> Option<String> {
    const ATLAS_INSTANCE: u32 = 0x835D_64F3;
    const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
    for (_, pkg) in packages {
        let Some(entry) = pkg
            .entries()
            .iter()
            .find(|e| e.id.instance == ATLAS_INSTANCE && e.id.type_id == RW4_MODEL_TYPE)
        else {
            continue;
        };
        let Ok(data) = pkg.read(entry) else {
            continue;
        };
        let Ok(file) = rw4::Rw4File::parse(&data) else {
            continue;
        };
        let Some(sec) = file.sections_of_type(rw4::SectionType::TEXTURE).next().map(|s| s.number)
        else {
            continue;
        };
        let Ok(tex) = file.decode_texture(&data, sec) else {
            continue;
        };
        let Ok(rgba) = tex.decode_top_mip_rgba() else {
            continue;
        };
        let (w, h) = (tex.width as usize, tex.height as usize);
        // 裁上半（树公告板 2×2；下半为地面纹理，非树）
        let top_h = h / 2;
        let mut cropped = Vec::with_capacity(w * top_h * 4);
        for y in 0..top_h {
            let src = (y * w) * 4;
            cropped.extend_from_slice(&rgba[src..src + w * 4]);
        }
        let img = image::RgbaImage::from_fn(w as u32, top_h as u32, |x, y| {
            let at = (x as usize + y as usize * w) * 4;
            image::Rgba([cropped[at], cropped[at + 1], cropped[at + 2], cropped[at + 3]])
        });
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut png, image::ImageFormat::Png)
            .ok()?;
        use base64::Engine as _;
        return Some(base64::engine::general_purpose::STANDARD.encode(png.into_inner()));
    }
    None
}

/// 候选 key → 过滤出在已打开包中真实存在的 RW4 模型，补 package id。
fn finish(
    packages: &[(u64, Arc<Package>)],
    resource_id: u32,
    keys: Vec<(u32, u32, u32)>,
    source: &str,
) -> Option<ResolvedPropModel> {
    let mut models = Vec::new();
    let mut package_id = None;
    for (inst, ty, group) in keys {
        let type_id = if ty == 0 { RW4_MODEL_TYPE } else { ty };
        let hit = packages.iter().find_map(|(pid, pkg)| {
            pkg.entries()
                .iter()
                .find(|e| {
                    e.id.instance == inst
                        && e.id.type_id == type_id
                        && (group == 0 || e.id.group == group)
                })
                .map(|_| *pid)
        });
        if let Some(pid) = hit {
            if package_id.is_none() {
                package_id = Some(pid);
            }
            models.push(TgiDto {
                type_id,
                group,
                instance: inst,
            });
        }
    }
    if models.is_empty() {
        None
    } else {
        // LOD/变体数组顺序不保证（消防局实测绘 41K/21.8K/3.6K/9.1K 乱序）——
        // 按 RW4 体积降序，models[0] 恒为最高细节（前端取首项）。
        let size_of = |tgi: &TgiDto| -> u32 {
            packages
                .iter()
                .filter_map(|(_, pkg)| {
                    pkg.entries()
                        .iter()
                        .find(|e| {
                            e.id.instance == tgi.instance
                                && e.id.type_id == tgi.type_id
                                && (tgi.group == 0 || e.id.group == tgi.group)
                        })
                        .map(|e| e.decompressed_size)
                })
                .next()
                .unwrap_or(0)
        };
        models.sort_by(|a, b| size_of(b).cmp(&size_of(a)));
        Some(ResolvedPropModel {
            resource_id,
            models,
            package_id,
            source: source.to_string(),
            tree_atlas_png: None,
        })
    }
}

/// 本进程内由 prop 解析自动注册的 EcoGame 包 id（PE 关闭时统一卸载）。
static REGISTERED_ECOGAME: LazyLock<Mutex<Vec<u64>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// 卸载全部自动注册的 EcoGame 包，返回卸载数。
pub fn release_registered(manager: &PackageManager) -> Result<usize, PackageError> {
    let ids: Vec<u64> = REGISTERED_ECOGAME
        .lock()
        .map_err(|_| PackageError::StatePoisoned)?
        .drain(..)
        .collect();
    let mut released = 0usize;
    for id in &ids {
        // 用户手动关闭/包池重排时不报错——目标已是"池里没有它们"
        if manager.close(*id).is_ok() {
            released += 1;
        }
    }
    Ok(released)
}

/// 命令入口：解析一批 prop resourceID。
pub fn resolve_all(
    manager: &PackageManager,
    resource_ids: &[u32],
) -> Result<Vec<ResolvedPropModel>, PackageError> {
    let base = manager.all_packages_with_ids()?;
    let dirs = ecogame_dirs(&base);
    let eco = ensure_ecogame_registered(manager, &dirs)?;
    // 登记进会话注册表（PE 关闭时 release_prop_model_packages 统一卸载）
    let eco_ids: Vec<u64> = eco.iter().map(|(id, _)| *id).collect();
    let mut packages = base;
    packages.extend(eco);
    if let Ok(mut reg) = REGISTERED_ECOGAME.lock() {
        reg.extend(eco_ids);
        reg.sort_unstable();
        reg.dedup();
    }
    Ok(resource_ids.iter().filter_map(|id| resolve_one(&packages, *id)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手动集成测试（需本机游戏包 + EcoGame 目录）：
    /// cargo test -p fluffy-open-scp --release prop_models -- --ignored
    #[test]
    #[ignore]
    fn resolves_fire_station_prop_models() {
        let manager = PackageManager::new();
        for path in [
            r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package",
        ] {
            let package = Package::open(path).expect("open");
            manager.insert(package).expect("insert");
        }
        let ids: [u32; 10] = [
            0x0AB4_DFE1, 0x0AB4_DFE2, 0x0AB4_DFE3, 0xBF2C_1022, 0x54CA_89F0,
            0x92BE_E95F, 0x1498_4C6B, 0x1498_4C6A, 0x1498_4C69, 0x1498_4C68,
        ];
        let out = resolve_all(&manager, &ids).expect("resolve");
        for r in &out {
            println!(
                "I {:08X} -> {} models (pkg {:?}) source={}",
                r.resource_id,
                r.models.len(),
                r.package_id,
                r.source
            );
        }
        // 车辆链（显式 key → 0x0D897169）必须命中
        assert!(
            out.iter().any(|r| matches!(r.source.as_str(), "explicit" | "vehicle_models")),
            "至少一个 prop 解析出 RW4 模型"
        );
        // 树链（乔木 14984C68-6B）必须走 tree_model（4 个 impostor 源 3D 模型）
        let tree = out
            .iter()
            .find(|r| r.source == "tree_model")
            .expect("树 prop 应解析出 tree_model");
        assert_eq!(tree.models.len(), 4, "树应有 4 个 impostor 源模型");
        assert!(tree.package_id.is_some(), "树模型应定位到所在包");
        assert!(tree.tree_atlas_png.is_some(), "树应随发公告板图集（回落用）");
    }
}

#[cfg(test)]
mod lotm_dump_tests {
    // 手动取证：把车辆模型的 LOTM 材质 PNG 落盘（看 baseColor/palette 实况）。
    // cargo test -p fluffy-open-scp --release lotm_dump -- --ignored --nocapture
    use crate::package_service::PackageManager;
    use dbpf::Package;

    #[test]
    #[ignore]
    fn dump_vehicle_lotm_materials() {
        let manager = PackageManager::new();
        for path in [
            r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package",
        ] {
            let package = Package::open(path).expect("open");
            manager.insert(package).expect("insert");
        }
        for (inst, out) in [
            (0xCA26_5D8Bu32, "tmp/vehicle.lotm"),
            (0x903A_704C, "tmp/trashcan.lotm"),
            (0x4DF4_3690, "tmp/tree.lotm"),
        ] {
            let pkg = manager.get(2).expect("graphics pkg");
            let entry = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == 0x2F4E_681B)
                .expect("model");
            let data = pkg.read(entry).expect("read");
            let file = rw4::Rw4File::parse(&data).expect("parse");
            let payload = crate::package_service::build_lot_model_payload_for_test(
                &file, &data, &pkg, &manager, inst,
            );
            std::fs::write(out, &payload).expect("write lotm");
            println!("{out}: {} 字节", payload.len());
        }
    }
}
