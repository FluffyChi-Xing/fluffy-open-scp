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
use rw4::Rw4File;
use sc_properties::{Kind, ParseLimits, PropertyFile, Value};
use serde::Serialize;

use crate::package_service::{PackageError, PackageManager, TgiDto};

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
/// 引擎类别绑定 Key（prop 资源自带；7CB23A4E=Garbage、8E8B4E10=Power 等）。
const KEY_CATEGORY_BINDING: u32 = 0x08A1_6042;
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
        let Some(data_dir) = path.parent() else {
            continue;
        };
        for root in [
            data_dir.parent().map(|p| p.to_path_buf()),
            data_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf()),
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
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE && e.id.instance == instance)
        {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            hits.push((*id, file, entry.id.group));
        }
    }
    // 资源组形态的记录排前（同 instance 多 group 复制时取语义正确者）
    hits.sort_by_key(|(_, _, group)| (RESOURCE_GROUP_LOW.contains(&(*group as u16)), *group));
    hits
}

/// 单个 resourceID 的解析（两分支 + 载具定义 + 树 descriptor）。
fn resolve_one(packages: &[(u64, Arc<Package>)], resource_id: u32) -> Option<ResolvedPropModel> {
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

    // 包装记录：0x0D897169 直出 / LOD1 / 树标记
    for (w_inst, _w_type, _w_group) in &wrapper_keys {
        for (_, file, _) in find_records(packages, *w_inst) {
            // 常规杂件（如屋顶太阳能板）把 RW4 模型直接放在包装记录的
            // LOD1 字段，而资源记录本身只保存 0x0D8C29C3 包装 key。
            // 这条链与资源记录自带的 lod1_direct 不同，必须在解包后再查一次。
            let lod1 = key_list(&file, KEY_LOD1);
            if !lod1.is_empty() {
                if let Some(done) = finish(packages, resource_id, lod1, "lod1_wrapper") {
                    return Some(done);
                }
            }
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
                    TREE_PART_MODELS
                        .iter()
                        .map(|i| (*i, RW4_MODEL_TYPE, 0))
                        .collect(),
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
        let Some(sec) = file
            .sections_of_type(rw4::SectionType::TEXTURE)
            .next()
            .map(|s| s.number)
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
            image::Rgba([
                cropped[at],
                cropped[at + 1],
                cropped[at + 2],
                cropped[at + 3],
            ])
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
static REGISTERED_ECOGAME: LazyLock<Mutex<Vec<u64>>> = LazyLock::new(|| Mutex::new(Vec::new()));

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

/// 打开 property 前预注册 EcoGame 包：decal 的 Lot Textures / 树图集在
/// `read_lot_editor_session` 阶段就要从已打开包里取，不能等到 prop 解析。
pub fn ensure_for_session(manager: &PackageManager) -> Result<(), PackageError> {
    let base = manager.all_packages_with_ids()?;
    let dirs = ecogame_dirs(&base);
    let eco = ensure_ecogame_registered(manager, &dirs)?;
    let eco_ids: Vec<u64> = eco.iter().map(|(id, _)| *id).collect();
    if let Ok(mut reg) = REGISTERED_ECOGAME.lock() {
        reg.extend(eco_ids);
        reg.sort_unstable();
        reg.dedup();
    }
    Ok(())
}

/// 命令入口：解析一批 prop resourceID。
pub fn resolve_all(
    manager: &PackageManager,
    resource_ids: &[u32],
) -> Result<Vec<ResolvedPropModel>, PackageError> {
    // Auto-registered EcoGame packages are only needed for the current batch;
    // keeping them open across properties exhausts MAX_OPEN_PACKAGES and makes
    // every later property resolve zero props. Release the previous batch when
    // the pool is near the cap (frontend already holds the LOTM bytes it needs).
    // Do it only near the cap so normal prop reads are not racing a close.
    let mut base = manager.all_packages_with_ids()?;
    if base.len() + 2 >= crate::package_service::MAX_OPEN_PACKAGES {
        release_registered(manager)?;
        base = manager.all_packages_with_ids()?;
    }
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
    Ok(resource_ids
        .iter()
        .filter_map(|id| {
            resolve_one(&packages, *id).or_else(|| resolve_direct_model(&packages, *id))
        })
        .collect())
}

/// 工作台「直挂模型」prop 兜底：构建器把 modelTgi.instance 写进 PROP_ID 列，
/// 该 id 是 RW4 模型实例、不在脚本资源表内（resolve_one 必失败）。脚本表
/// 解析失败时按 0x2F4E681B:0:<id> 跨已打开包直查模型（2026-10-30 用户
/// 对拍：构建包 prop 全部只剩锥标记的根因）。
fn resolve_direct_model(
    packages: &[(u64, Arc<Package>)],
    resource_id: u32,
) -> Option<ResolvedPropModel> {
    packages.iter().find_map(|(id, package)| {
        let entry = package.entries().iter().find(|e| {
            e.id.type_id == RW4_MODEL_TYPE && e.id.group == 0 && e.id.instance == resource_id
        })?;
        // 拒绝非模型载荷（能按 RW4 解析且带 MESH 段才认）。
        let bytes = package.read(entry).ok()?;
        let file = Rw4File::parse(&bytes).ok()?;
        file.sections_of_type(rw4::SectionType::MESH).next()?;
        Some(ResolvedPropModel {
            resource_id,
            models: vec![TgiDto {
                type_id: RW4_MODEL_TYPE,
                group: 0,
                instance: resource_id,
            }],
            package_id: Some(*id),
            source: "direct_model".into(),
            tree_atlas_png: None,
        })
    })
}

// ---------------------------------------------------------------------------
// prop bin 语义目录（组件库「树/车辆/垃圾桶/代售牌子」全量筛选，2026-10-27；
// 2026-11-04 树类别改为普查权威清单 TREE_SPECIES——名称启发式不再判树）。
// 数据源 = 已打开包 + 自动注册 EcoGame 包的资源组记录（group 低 16 位 ∈
// RESOURCE_GROUP_LOW）；模型 TGI 复用 resolve_one 全链。
// ---------------------------------------------------------------------------

/// 树 prop 权威清单（2026-11-04 全盘普查，crates/sc-properties/examples/
/// tree_hunt.rs 实证；两套安装、12 个 EcoGame 历史版本一致）：可被 prop
/// 系统放置的树有且仅有 8 个 resourceID = 4 树种。树种 = 引擎 impostor
/// lod 序号 0-3 ↔ 公告板图集 2×2 象限；每树种双路 resourceID（R/C/I 建筑
/// 14984C68-6B / 公园-森林工具 41C2D443、C8B4254x）指向同一 wrapper，
/// 同种同外观。resource 记录 B1B104 G 40E0C100（EcoGame），wrapper
/// B1B104 G 40E02D00（SimCity_Game），descriptor C602CD31 G 40002D00
/// （SimCity_App，仅作引擎链一环）。海边棕榈/沙漠仙人掌不在 prop 系统
/// （地形装饰内容），故无对应条目。
const TREE_SPECIES: [(&[u32], u32, u32); 4] = [
    // (resourceID 双路, lod 序号, wrapper instance)
    (&[0x1498_4C6B, 0x41C2_D443], 0, 0xADA2_724C),
    (&[0x1498_4C6A, 0xC8B4_2548], 1, 0xADA2_724D),
    (&[0x1498_4C69, 0xC8B4_254B], 2, 0xADA2_724E),
    (&[0x1498_4C68, 0xC8B4_254A], 3, 0xADA2_724F),
];

/// resourceID → (树种序号 0-3，同种双路中的路序)。普查外 id 返回 None。
fn census_tree_species(resource_id: u32) -> Option<(usize, usize)> {
    TREE_SPECIES
        .iter()
        .enumerate()
        .find_map(|(species, (ids, _, _))| {
            ids.iter()
                .position(|id| *id == resource_id)
                .map(|dup| (species, dup))
        })
}

/// 普通建筑车辆权威清单（2026-11-04 普查，crates/sc-properties/examples/
/// car_census.rs 实证）：可被普通建筑（R/C/I/公园）停车场绑定的车辆原型
/// 有且仅有 CarW1/W2/W3 三档（Game+EP1 543/546/479 lots；Graphics R/C/I
/// 建筑记录 226 条财富×车辆列联对角线零反例：W1 建筑只绑 CarW1、W2→
/// CarW2、W3→CarW3，mix=公园/混合类绑多档）。特殊车辆（ConstructionTruck/
/// Ambulance/GarbageTruck/kResourceIDFreight 等，绑定量个位数十）由专用
/// 建筑刷出，名称启发式不再判车辆。bin 语义：ids=原型表、transforms=车位、
/// slots=填充序（原版恒 0..n-1）；运行时可见车数 = EcoResourceBin 的
/// mAmount（随入住/上班人数涨落），PercentFill 置位时 = ceil(Amount/
/// Capacity × 车位数)（engine cUnitBinDraw）。
const NORMAL_VEHICLES: [(u32, u32); 3] = [
    // (resourceID, 财富档位)
    (0x0AB4_DFE1, 1),
    (0x0AB4_DFE2, 2),
    (0x0AB4_DFE3, 3),
];

/// resourceID → 车辆清单序号。普查外 id 返回 None。
fn census_vehicle(resource_id: u32) -> Option<usize> {
    NORMAL_VEHICLES.iter().position(|(id, _)| *id == resource_id)
}

/// prop bin 语义类别（trees/vehicles/waste/signs = 组件库四个明确目标；
/// misc = 已解析出模型但名称不可判的杂件桶——6673 lot 普查证明全游戏
/// prop ≈ 车辆+树+杂件三族，无招牌大类，牌子类道具只能在此桶目视挑选）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PropBinCategory {
    Trees,
    Vehicles,
    Waste,
    Signs,
    Misc,
}

/// prop bin 目录条目：resourceID + 分类 + 可放置的首选 RW4 模型。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropBinCatalogEntry {
    pub resource_id: u32,
    pub name: String,
    pub category: PropBinCategory,
    /// 首选 RW4 模型（models[0]，最高细节）。
    pub model: TgiDto,
    /// 模型所在已打开包 id。
    pub model_package_id: u64,
    /// 解析路径（同 [`ResolvedPropModel::source`]）。
    pub source: String,
}

/// 噪声注册表名（UI 键/图标/教程气泡/告警等非 prop 记录）。空名**不算**
/// 噪声——多数 prop 资源/模型在注册表中无名，靠种子映射与解析签名分类。
fn is_noise_name(name: &str) -> bool {
    let n = name.trim();
    n.starts_with("kResourceUIKeys")
        || n.starts_with("icn_")
        || n.starts_with("datavis_")
        || n.starts_with("MSTutorial")
        || n.starts_with("speech_")
        || n.starts_with("ThoughtBalloon")
        || n.starts_with("Alert")
}

/// 多词任一词边界命中：`word` 在 `hay` 中出现且两侧非 ASCII 字母
/// （"bush" 不含词 "bus"、"design" 不含词 "sign"、"Bus_Stop" 含词 "bus"）。
fn contains_any_word(hay: &str, words: &[&str]) -> bool {
    let h = hay.as_bytes();
    words.iter().any(|word| {
        let w = word.as_bytes();
        if w.is_empty() || h.len() < w.len() {
            return false;
        }
        let mut start = 0;
        while start + w.len() <= h.len() {
            if let Some(pos) = hay[start..].find(word) {
                let at = start + pos;
                let left_ok = at == 0 || !h[at - 1].is_ascii_alphabetic();
                let end = at + w.len();
                let right_ok = end == h.len() || !h[end].is_ascii_alphabetic();
                if left_ok && right_ok {
                    return true;
                }
                start = at + 1;
            } else {
                break;
            }
        }
        false
    })
}


/// 已验证种子映射（人工指认，2026-10-27）。键同时匹配 resourceID 与首选
/// 模型 instance——同模型变体自动跟随。仅用于无任何自动信号的 prop。
fn seed_category(instance: u32) -> Option<PropBinCategory> {
    Some(match instance {
        // 出租告示牌 "HARSH REALTY / PROPERTY FOR LEASE"（用户指认，
        // C_Gen_W1D2_Store_02_01_N 屋顶；CAF1511E→LOD1→0D4DBEA7，
        // Graphics RW4 64KB，贴图即牌面文字。资源/模型均无注册表名；
        // 同族三变体 40731424/4E15EC0E 经模型实例自动跟随）
        0xCAF1_511E | 0x0D4D_BEA7 => PropBinCategory::Signs,
        _ => return None,
    })
}

/// 引擎类别绑定（prop 资源上的 `0x08A16042` Key，威利斯研究 §4.3 同族：
/// 模板类别绑定属性）→ prop bin 类别。这是引擎自带语义，优先级最高。
/// 已核实：46FF72C0/41FA6F69/D15E6C57/2F207D6B（垃圾桶/大型垃圾箱）→
/// 7CB23A4E kCategoryIDGarbage；solarpanel BDB7A53A → 8E8B4E10 kCategoryIDPower。
fn binding_category(category_id: u32) -> Option<PropBinCategory> {
    Some(match category_id {
        0x7CB2_3A4E => PropBinCategory::Waste, // kCategoryIDGarbage
        _ => return None,
    })
}

/// 分类顺序即优先级：引擎类别绑定 > 树普查权威清单（TREE_SPECIES）>
/// 车辆普查权威清单（NORMAL_VEHICLES）> 种子映射 > 垃圾/招牌注册表名 >
/// 杂件桶。名称启发式不再判树/车（"bush" 含 "bus"、"garbagetruck" 同时含
/// garbage+truck 的历史教训；特殊车辆归入垃圾桶/杂件桶目视挑选）。
fn classify_bin(
    name: &str,
    census_tree: bool,
    census_car: bool,
    seed: Option<PropBinCategory>,
    binding: Option<PropBinCategory>,
) -> Option<PropBinCategory> {
    if let Some(category) = binding {
        return Some(category);
    }
    if census_tree {
        return Some(PropBinCategory::Trees);
    }
    if census_car {
        return Some(PropBinCategory::Vehicles);
    }
    if let Some(category) = seed {
        return Some(category);
    }
    let n = name.to_lowercase();
    if contains_any_word(
        &n,
        &["garbage", "trash", "dumpster", "litter", "recycle", "bin"],
    ) {
        return Some(PropBinCategory::Waste);
    }
    if contains_any_word(&n, &["sign", "billboard", "banner", "flag"]) {
        return Some(PropBinCategory::Signs);
    }
    None
}

/// 全量 prop bin 语义目录（四个目标类别；模型解析失败者不计入）。
pub fn prop_bin_catalog(
    manager: &PackageManager,
    bundled_registry: Option<&Path>,
) -> Result<Vec<PropBinCatalogEntry>, PackageError> {
    let base = manager.all_packages_with_ids()?;
    let dirs = ecogame_dirs(&base);
    let eco = ensure_ecogame_registered(manager, &dirs)?;
    let eco_ids: Vec<u64> = eco.iter().map(|(id, _)| *id).collect();
    let mut packages = base;
    packages.extend(eco);
    if let Ok(mut reg) = REGISTERED_ECOGAME.lock() {
        reg.extend(eco_ids);
        reg.sort_unstable();
        reg.dedup();
    }
    let registry = bundled_registry.and_then(|path| sc_registry::Registry::open(path).ok());
    let instance_name = |instance: u32| -> Option<String> {
        registry
            .as_ref()
            .and_then(|r| r.instances().get(&instance))
            .map(|record| record.name.trim().to_string())
            .filter(|n| !n.is_empty())
    };

    // 1) 资源记录枚举（按 instance 去重；EcoGame 与安装包副本同 id 时
    //    resolve_one 自带资源组形态优先）
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut entries: Vec<PropBinCatalogEntry> = Vec::new();
    // 1.0) 普查树直入（封闭权威清单）：第二路 resourceID（41C2D443、
    //      C8B4254x）的记录 group 不落在 RESOURCE_GROUP_LOW，主循环枚举
    //      不到——树清单必须全量出现（tree_hunt 普查实证），故先行解析。
    for species in &TREE_SPECIES {
        let (ids, lod, _wrapper) = *species;
        for (dup, resource_id) in ids.iter().enumerate() {
            if !seen.insert(*resource_id) {
                continue;
            }
            let Some(resolved) = resolve_one(&packages, *resource_id) else {
                continue;
            };
            let Some(model) = resolved.models.first() else {
                continue;
            };
            let Some(model_package_id) = resolved.package_id else {
                continue;
            };
            entries.push(PropBinCatalogEntry {
                resource_id: *resource_id,
                name: format!(
                    "Tree {}{} · 0x{resource_id:08X}",
                    (b'A' + lod as u8) as char,
                    if dup == 0 { "" } else { " alt" }
                ),
                category: PropBinCategory::Trees,
                model: model.clone(),
                model_package_id,
                source: resolved.source,
            });
        }
    }
    // 1.1) 普通建筑车辆直入（封闭权威清单 NORMAL_VEHICLES；EcoGame G
    //      40E0C100 主循环可枚举，但封闭集直入保证缺一即知——与树同构）。
    for (resource_id, wealth) in &NORMAL_VEHICLES {
        if !seen.insert(*resource_id) {
            continue;
        }
        let Some(resolved) = resolve_one(&packages, *resource_id) else {
            continue;
        };
        let Some(model) = resolved.models.first() else {
            continue;
        };
        let Some(model_package_id) = resolved.package_id else {
            continue;
        };
        entries.push(PropBinCatalogEntry {
            resource_id: *resource_id,
            name: format!("Car W{wealth} · 0x{resource_id:08X}"),
            category: PropBinCategory::Vehicles,
            model: model.clone(),
            model_package_id,
            source: resolved.source,
        });
    }
    for (_, pkg) in &packages {
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_RESOURCE_TYPE
                && RESOURCE_GROUP_LOW.contains(&(e.id.group as u16))
        }) {
            if !seen.insert(entry.id.instance) {
                continue;
            }
            let resource_id = entry.id.instance;
            let raw_name = instance_name(resource_id).unwrap_or_default();
            if is_noise_name(&raw_name) {
                continue;
            }
            // 1.5) 引擎类别绑定（0x08A16042 Key，垃圾桶/垃圾箱等自带）
            let binding = parse_record(pkg, entry)
                .and_then(|file| first_key(&file, KEY_CATEGORY_BINDING))
                .and_then(|(instance, _, _)| binding_category(instance));
            // 2) 先解析模型（复用全链；无模型即不可放置，直接排除）
            let Some(resolved) = resolve_one(&packages, resource_id) else {
                continue;
            };
            // 3) 分类：引擎类别绑定 > 树普查权威清单（TREE_SPECIES）> 种子
            //    映射 > 注册表名（车辆/垃圾/招牌）> 兜底杂件桶。普查树注册表
            //    无名——显示树种字母 + 本体 id（同种双路同外观，见清单注释）。
            let census = census_tree_species(resource_id);
            let car = census_vehicle(resource_id);
            let model_instance = resolved
                .models
                .first()
                .map(|m| m.instance)
                .unwrap_or(resource_id);
            let display_name = if let Some((species, _)) = census {
                format!(
                    "Tree {} · 0x{resource_id:08X}",
                    (b'A' + species as u8) as char
                )
            } else if let Some(index) = car {
                format!(
                    "Car W{} · 0x{resource_id:08X}",
                    NORMAL_VEHICLES[index].1
                )
            } else {
                let name = instance_name(model_instance).unwrap_or_else(|| raw_name.clone());
                if name.is_empty() {
                    format!("0x{resource_id:08X}")
                } else {
                    name
                }
            };
            let seed = seed_category(resource_id).or_else(|| seed_category(model_instance));
            let category = classify_bin(
                &display_name,
                census.is_some(),
                car.is_some(),
                seed,
                binding,
            )
            .unwrap_or(PropBinCategory::Misc);
            let Some(model) = resolved.models.first() else {
                continue;
            };
            let Some(model_package_id) = resolved.package_id else {
                continue;
            };
            entries.push(PropBinCatalogEntry {
                resource_id,
                name: display_name,
                category,
                model: model.clone(),
                model_package_id,
                source: resolved.source,
            });
        }
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

// ---------------------------------------------------------------------------
// 路径条目目录（path entry 选择器，2026-10-27 路径规划研究落地）。
// 枚举已打开包（含自动注册 EcoGame）G 低 16 位 0xC400 的 B1B104 记录，
// 解析允许通行类型 allowlist（0x0D6498E8 → 注册表名，如 "Pedestrians"）
// 与父模板链。威利斯实证：门口短线 entry 418A18B1 allowlist=Pedestrians。
// ---------------------------------------------------------------------------

/// 路径条目摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathEntrySummary {
    pub instance: u32,
    pub group: u32,
    pub package_id: u64,
    /// 允许通行类型解析名（注册表有名者；无名条目以 hex 补充）。
    pub transports: Vec<String>,
    /// allowlist 原始实例（注册表无名时前端显示 hex）。
    pub transport_ids: Vec<u32>,
    /// 父模板 instance（0x00B2CCCB 首环，诊断用）。
    pub parent: Option<u32>,
    /// 道路几何参数族是否在场（0x0BD81A2D）。
    pub has_geometry: bool,
}

const PATH_ENTRY_GROUP_LOW: u16 = 0xC400;
const KEY_TRANSPORT_ALLOWLIST: u32 = 0x0D64_98E8;

/// 全量路径条目目录（entry 选择器数据源）。
/// 带路径的 property 摘要（工作台"目标建筑选择"数据源，2026-10-28）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSourceSummary {
    pub package_id: u64,
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
    /// 注册表名（无名时为空，前端回退 hex）。
    pub name: String,
    /// 路径条数（pairs 对数）。
    pub path_count: usize,
    /// 采样点总数。
    pub point_count: usize,
    /// 该 property 各路径引用的 path entry（去重）。
    pub entry_ids: Vec<u32>,
}

const KEY_PATH_POINTS: u32 = 0x0CAA_680D;
const KEY_PATH_PAIRS: u32 = 0x0CAA_6841;
const KEY_PATH_ENTRY: u32 = 0x0CC8_FE7A;

/// 枚举一个包内带路径点列的 property（"目标建筑"候选列表）。
/// 判定 = B1B104 记录含非空 0x0CAA680D 点数组；entry 取 0x0CC8FE7A 去重。
pub fn path_source_catalog(
    manager: &PackageManager,
    package_id: u64,
    bundled_registry: Option<&Path>,
) -> Result<Vec<PathSourceSummary>, PackageError> {
    let packages = manager.all_packages_with_ids()?;
    let Some((_, pkg)) = packages.iter().find(|(id, _)| *id == package_id) else {
        return Err(PackageError::StatePoisoned);
    };
    let registry = bundled_registry.and_then(|path| sc_registry::Registry::open(path).ok());
    let name_of = |instance: u32| -> String {
        registry
            .as_ref()
            .and_then(|r| r.instances().get(&instance))
            .map(|record| record.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_default()
    };
    let mut entries: Vec<PathSourceSummary> = Vec::new();
    for entry in pkg
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE)
    {
        let Some(file) = parse_record(pkg, entry) else {
            continue;
        };
        // 0x0CAA680D 是 vector3 数组（非 Key），必须按值提取——
        // key_list 只匹配 Key 变体，会把所有路径记录误判为无
        let points = values_of(&file, KEY_PATH_POINTS);
        if points.is_empty() {
            continue;
        }
        let pairs = values_of(&file, KEY_PATH_PAIRS).len();
        let mut entry_ids: Vec<u32> = Vec::new();
        for (instance, _, _) in key_list(&file, KEY_PATH_ENTRY) {
            if !entry_ids.contains(&instance) {
                entry_ids.push(instance);
            }
        }
        let instance = entry.id.instance;
        entries.push(PathSourceSummary {
            package_id,
            type_id: entry.id.type_id,
            group: entry.id.group,
            instance,
            name: name_of(instance),
            path_count: pairs / 2,
            point_count: points.len(),
            entry_ids,
        });
    }
    entries.sort_by(|a, b| b.path_count.cmp(&a.path_count));
    Ok(entries)
}

pub fn path_entry_catalog(
    manager: &PackageManager,
    bundled_registry: Option<&Path>,
) -> Result<Vec<PathEntrySummary>, PackageError> {
    let base = manager.all_packages_with_ids()?;
    let dirs = ecogame_dirs(&base);
    let eco = ensure_ecogame_registered(manager, &dirs)?;
    let eco_ids: Vec<u64> = eco.iter().map(|(id, _)| *id).collect();
    let mut packages = base;
    packages.extend(eco);
    if let Ok(mut reg) = REGISTERED_ECOGAME.lock() {
        reg.extend(eco_ids);
        reg.sort_unstable();
        reg.dedup();
    }
    let registry = bundled_registry.and_then(|path| sc_registry::Registry::open(path).ok());
    let instance_name = |instance: u32| -> Option<String> {
        registry
            .as_ref()
            .and_then(|r| r.instances().get(&instance))
            .map(|record| record.name.trim().to_string())
            .filter(|n| !n.is_empty())
    };

    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut entries: Vec<PathEntrySummary> = Vec::new();
    for (package_id, pkg) in &packages {
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_RESOURCE_TYPE && (e.id.group as u16) == PATH_ENTRY_GROUP_LOW
        }) {
            if !seen.insert(entry.id.instance) {
                continue;
            }
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            let mut transports = Vec::new();
            let mut transport_ids = Vec::new();
            for (instance, _, _) in key_list(&file, KEY_TRANSPORT_ALLOWLIST) {
                transport_ids.push(instance);
                transports
                    .push(instance_name(instance).unwrap_or_else(|| format!("0x{instance:08X}")));
            }
            entries.push(PathEntrySummary {
                instance: entry.id.instance,
                group: entry.id.group,
                package_id: *package_id,
                transports,
                transport_ids,
                parent: first_key(&file, KEY_PARENT).map(|(instance, _, _)| instance),
                has_geometry: has_prop(&file, 0x0BD8_1A2D),
            });
        }
    }
    entries.sort_by_key(|e| e.instance);
    Ok(entries)
}

/// path_source_catalog 冒烟（需本机游戏包，用户报告的漏检回归钉）：
/// cargo test -p fluffy-open-scp --release path_source_catalog -- --ignored --nocapture
#[test]
#[ignore]
fn path_source_catalog_finds_user_cited_records() {
    let manager = PackageManager::new();
    for path in [
        r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package",
        r"D:\ea-games\SimCity\SimCityData\SimCityDataEP1.package",
    ] {
        let package = Package::open(path).expect("open");
        manager.insert(package).expect("insert");
    }
    let registry = Path::new("resources/database_main.s3db");
    let graphics = path_source_catalog(&manager, 1, Some(registry)).expect("graphics");
    assert!(
        graphics
            .iter()
            .any(|e| e.instance == 0x10DA_F7BF && e.group == 0x40A2_A151),
        "Graphics 应命中用户指出的 40A2A151:10DAF7BF"
    );
    let ep1 = path_source_catalog(&manager, 2, Some(registry)).expect("ep1");
    assert!(
        ep1.iter()
            .any(|e| e.instance == 0x1A74_90A1 && e.group == 0x42E1_C000),
        "EP1 应命中用户指出的 42E1C000:1A7490A1"
    );
    for e in graphics.iter().take(5) {
        println!(
            "{:08X}:{:08X} {} pts={} paths={}",
            e.group, e.instance, e.name, e.point_count, e.path_count
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

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
            0x0AB4_DFE1,
            0x0AB4_DFE2,
            0x0AB4_DFE3,
            0xBF2C_1022,
            0x54CA_89F0,
            0x92BE_E95F,
            0x1498_4C6B,
            0x1498_4C6A,
            0x1498_4C69,
            0x1498_4C68,
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
            out.iter()
                .any(|r| matches!(r.source.as_str(), "explicit" | "vehicle_models")),
            "至少一个 prop 解析出 RW4 模型"
        );
        // 树链（乔木 14984C68-6B）必须走 tree_model（4 个 impostor 源 3D 模型）
        let tree = out
            .iter()
            .find(|r| r.source == "tree_model")
            .expect("树 prop 应解析出 tree_model");
        assert_eq!(tree.models.len(), 4, "树应有 4 个 impostor 源模型");
        assert!(tree.package_id.is_some(), "树模型应定位到所在包");
        assert!(
            tree.tree_atlas_png.is_some(),
            "树应随发公告板图集（回落用）"
        );
    }

    /// 引擎类别绑定（0x08A16042）全量普查：EcoGame 脚本 prop 资源上该 Key
    /// 的全部去重取值 + 计数 + 每值 prop 注册表名样本——「prop 到底有哪些
    /// 引擎自带语义」的证据底表。
    /// cargo test -p fluffy-open-scp --release prop_category_binding_census -- --ignored --nocapture
    #[test]
    #[ignore]
    fn prop_category_binding_census() {
        let manager = PackageManager::new();
        for path in [
            r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_App.package",
            r"D:\ea-games\SimCity\SimCityData\SimCityDataEP1.package",
        ] {
            if let Ok(package) = Package::open(path) {
                manager.insert(package).expect("insert");
            }
        }
        let registry = sc_registry::Registry::open(Path::new("resources/database_main.s3db")).ok();
        let instance_name = |instance: u32| -> String {
            registry
                .as_ref()
                .and_then(|r| r.instances().get(&instance))
                .map(|record| record.name.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("{instance:08X}"))
        };
        let base = manager.all_packages_with_ids().expect("packages");
        let dirs = ecogame_dirs(&base);
        let eco = ensure_ecogame_registered(&manager, &dirs).expect("eco");
        let mut packages = base;
        packages.extend(eco);

        let mut census: std::collections::BTreeMap<u32, (usize, Vec<String>)> =
            std::collections::BTreeMap::new();
        let mut no_binding = 0usize;
        let mut seen = std::collections::HashSet::new();
        for (_, pkg) in &packages {
            for entry in pkg.entries().iter().filter(|e| {
                e.id.type_id == PROPERTY_RESOURCE_TYPE
                    && RESOURCE_GROUP_LOW.contains(&(e.id.group as u16))
            }) {
                if !seen.insert(entry.id.instance) {
                    continue;
                }
                let Some(file) = parse_record(pkg, entry) else {
                    continue;
                };
                let Some((_, _, category)) = first_key(&file, KEY_CATEGORY_BINDING) else {
                    no_binding += 1;
                    continue;
                };
                let slot = census.entry(category).or_default();
                slot.0 += 1;
                if slot.1.len() < 5 {
                    let name = instance_name(entry.id.instance);
                    if !slot.1.contains(&name) {
                        slot.1.push(name);
                    }
                }
            }
        }
        println!(
            "引擎类别绑定: {} 个不同类别; 无绑定 prop: {}",
            census.len(),
            no_binding
        );
        for (id, (count, names)) in &census {
            println!("  {id:08X}: {count} props  样本 {names:?}");
        }
    }

    /// prop bin 语义目录冒烟（需本机游戏包 + EcoGame 目录 + 注册表资源）：
    /// cargo test -p fluffy-open-scp --release prop_bin_catalog -- --ignored --nocapture
    #[test]
    #[ignore]
    fn prop_bin_catalog_smoke() {
        let manager = PackageManager::new();
        for path in [
            r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package",
            r"D:\ea-games\SimCity\SimCityData\SimCity_App.package",
        ] {
            let package = Package::open(path).expect("open");
            manager.insert(package).expect("insert");
        }
        let registry = Path::new("resources/database_main.s3db");
        let entries = prop_bin_catalog(&manager, Some(registry)).expect("catalog");
        let count = |c: PropBinCategory| entries.iter().filter(|e| e.category == c).count();
        println!(
            "catalog: trees={} vehicles={} waste={} signs={} total={}",
            count(PropBinCategory::Trees),
            count(PropBinCategory::Vehicles),
            count(PropBinCategory::Waste),
            count(PropBinCategory::Signs),
            entries.len()
        );
        for e in entries.iter().take(12) {
            println!(
                "  {:?} {:08X} {} -> {} (source={})",
                e.category, e.resource_id, e.name, e.model.instance, e.source
            );
        }
        // 小类别全量打印（便于种子维护与目视核对）
        for e in entries.iter().filter(|e| {
            !matches!(
                e.category,
                PropBinCategory::Misc | PropBinCategory::Vehicles
            )
        }) {
            println!(
                "  [{:?}] {:08X} {} -> model {:08X}",
                e.category, e.resource_id, e.name, e.model.instance
            );
        }
        // 威利斯/消防局研究实证过的锚点：垃圾桶与车辆必须非空
        assert!(count(PropBinCategory::Vehicles) > 0, "车辆类不能为空");
        assert!(count(PropBinCategory::Waste) > 0, "垃圾桶类不能为空");
        // 树链（descriptor/impostor 源模型）至少一条
        assert!(count(PropBinCategory::Trees) > 0, "树木类不能为空");
    }
    /// 直挂模型兜底：PROP_ID 是 RW4 模型实例（脚本资源表查不到）时，按
    /// 0x2F4E681B:0:<id> 跨已打开包直查并验证载荷确为模型（含 MESH 段）。
    #[test]
    fn direct_model_fallback_resolves_rw4_instance() {
        let mut mesh_payload = Vec::new();
        for value in [40u32, 4, 0, 0, 1, 0, 0, 0, 0, 0] {
            mesh_payload.extend_from_slice(&value.to_le_bytes());
        }
        let specs = [rw4::writer_section(rw4::SectionType::MESH, mesh_payload)];
        let model_bytes = rw4::build_file(1, &specs, &[]);
        let entries = [dbpf::OverlayEntry::new(
            dbpf::ResourceId {
                type_id: RW4_MODEL_TYPE,
                group: 0,
                instance: 0x1234_5678,
            },
            model_bytes,
        )];
        let dir = std::env::temp_dir().join(format!(
            "prop-models-direct-{}-{}",
            std::process::id(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let package_path = dir.join("direct.package");
        std::fs::write(
            &package_path,
            dbpf::write_uncompressed_overlay(&entries).unwrap(),
        )
        .unwrap();
        let package = Arc::new(Package::open(&package_path).unwrap());
        let packages = vec![(7u64, package)];

        assert!(resolve_one(&packages, 0x1234_5678).is_none());
        let resolved = resolve_direct_model(&packages, 0x1234_5678).expect("direct model");
        assert_eq!(resolved.source, "direct_model");
        assert_eq!(resolved.models.len(), 1);
        assert_eq!(resolved.models[0].type_id, RW4_MODEL_TYPE);
        assert_eq!(resolved.models[0].instance, 0x1234_5678);
        assert_eq!(resolved.package_id, Some(7));
        assert!(resolve_direct_model(&packages, 0xDEAD_BEEF).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 普查权威树清单（纯函数）：8 个 resourceID 全部命中且 4 树种齐全；
    /// 普查外 id（车辆/垃圾桶/直挂模型/相邻未收录 id）不误判。
    /// 清单锚定：crates/sc-properties/examples/tree_hunt.rs 2026-11-04 全盘
    /// 普查输出（两套安装、12 个 EcoGame 历史版本一致）。
    #[test]
    fn census_tree_species_maps_eight_resources_to_four_species() {
        let all: [u32; 8] = [
            0x1498_4C6B, 0x1498_4C6A, 0x1498_4C69, 0x1498_4C68,
            0x41C2_D443, 0xC8B4_2548, 0xC8B4_254B, 0xC8B4_254A,
        ];
        let mut species_seen = std::collections::BTreeSet::new();
        for id in all {
            let (species, dup) =
                census_tree_species(id).unwrap_or_else(|| panic!("{id:08X} 应命中普查清单"));
            assert!(dup < 2, "每树种最多双路");
            species_seen.insert(species);
        }
        assert_eq!(species_seen, [0, 1, 2, 3].into_iter().collect(), "4 树种齐全");
        for id in [0x0AB4_DFE1, 0x54CA_89F0, 0x0D4D_BEA7, 0x1498_4C6C] {
            assert!(
                census_tree_species(id).is_none(),
                "{id:08X} 不在普查清单，不应判树"
            );
        }
    }

    /// 普通建筑车辆权威清单（纯函数）：CarW1/W2/W3 三档全命中且财富号
    /// 与序号一致；特殊车辆/垃圾桶/树不误判（2026-11-04 car_census 普查
    /// 锚定：财富对角线零反例）。
    #[test]
    fn census_vehicle_maps_three_car_tiers() {
        let tiers: [(u32, u32); 3] = [
            (0x0AB4_DFE1, 1),
            (0x0AB4_DFE2, 2),
            (0x0AB4_DFE3, 3),
        ];
        for (id, wealth) in tiers {
            let index = census_vehicle(id).unwrap_or_else(|| panic!("{id:08X} 应命中车辆清单"));
            assert_eq!(NORMAL_VEHICLES[index].1, wealth, "财富档位对齐");
        }
        for id in [0x87AA_975D, 0xE310_1F25, 0xBDC7_1042, 0x1498_4C69] {
            assert!(
                census_vehicle(id).is_none(),
                "{id:08X} 特殊车辆/树不在普通车辆清单"
            );
        }
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
            (0x22A4_45FE, "tmp/simhead.lotm"),
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
