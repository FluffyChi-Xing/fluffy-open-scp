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

// ---------------------------------------------------------------------------
// prop bin 语义目录（组件库「树/车辆/垃圾桶/代售牌子」全量筛选，2026-10-27）。
// 数据源 = 已打开包 + 自动注册 EcoGame 包的资源组记录（group 低 16 位 ∈
// RESOURCE_GROUP_LOW）；树以树种 descriptor（G 40002D00，0x0E0B99FD 变体
// 数组）为权威信号，其余按注册表名分类；模型 TGI 复用 resolve_one 全链。
// ---------------------------------------------------------------------------

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

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
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

/// 分类顺序即优先级：引擎类别绑定 > 权威树(descriptor/变体) > 种子映射 >
/// 解析签名 > 树名（"bush" 含 "bus" 前缀，必须先判）> 车辆（"garbagetruck"
/// 同时含 garbage+truck，归车辆）> 垃圾 > 招牌。
fn classify_bin(
    source: &str,
    name: &str,
    authoritative_tree: bool,
    seed: Option<PropBinCategory>,
    binding: Option<PropBinCategory>,
) -> Option<PropBinCategory> {
    if let Some(category) = binding {
        return Some(category);
    }
    if authoritative_tree || matches!(source, "tree" | "tree_model" | "descriptor") {
        return Some(PropBinCategory::Trees);
    }
    if let Some(category) = seed {
        return Some(category);
    }
    let n = name.to_lowercase();
    if contains_any_word(
        &n,
        &[
            "tree", "palm", "oak", "pine", "bush", "hedge", "shrub", "foliage", "cypress",
            "willow", "maple", "birch", "spruce", "plant", "vegetation", "flower",
        ],
    ) {
        return Some(PropBinCategory::Trees);
    }
    if contains_any_word(
        &n,
        &[
            "truck", "train", "tram", "trolley", "vehicle", "taxi", "boat", "ferry", "ship",
            "moped", "scooter", "motorcycle", "ambulance", "limo", "helicopter", "car", "bus",
            "van",
        ],
    ) {
        return Some(PropBinCategory::Vehicles);
    }
    if contains_any_word(&n, &["garbage", "trash", "dumpster", "litter", "recycle", "bin"]) {
        return Some(PropBinCategory::Waste);
    }
    if contains_any_word(&n, &["sign", "billboard", "banner", "flag"]) {
        return Some(PropBinCategory::Signs);
    }
    None
}

/// 树种 descriptor 权威集：G 40002D00 记录（0x0E0B99FD 变体数组）→
/// （descriptor instance 集，变体模型 instance 集）。
fn tree_authority(packages: &[(u64, Arc<Package>)]) -> (std::collections::HashSet<u32>, std::collections::HashSet<u32>) {
    let mut descriptors = std::collections::HashSet::new();
    let mut variants = std::collections::HashSet::new();
    for (_, pkg) in packages {
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_RESOURCE_TYPE && e.id.group == 0x4000_2D00
        }) {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            if !has_prop(&file, KEY_TREE_VARIANTS) {
                continue;
            }
            descriptors.insert(entry.id.instance);
            for (inst, _, _) in key_list(&file, KEY_TREE_VARIANTS) {
                variants.insert(inst);
            }
        }
    }
    (descriptors, variants)
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
    let (tree_desc, tree_variants) = tree_authority(&packages);
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut entries: Vec<PropBinCatalogEntry> = Vec::new();
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
            // 3) 分类：引擎类别绑定 > descriptor 权威集 > 种子映射 > 解析签名 >
            //    注册表名 > 兜底杂件桶（模型可放置即可入选，名字后续目视定）
            let model_instance = resolved
                .models
                .first()
                .map(|m| m.instance)
                .unwrap_or(resource_id);
            let name = instance_name(model_instance).unwrap_or_else(|| raw_name.clone());
            let display_name = if name.is_empty() {
                format!("0x{resource_id:08X}")
            } else {
                name.clone()
            };
            let authoritative_tree = tree_desc.contains(&resource_id)
                || tree_variants.contains(&model_instance);
            let seed = seed_category(resource_id).or_else(|| seed_category(model_instance));
            let category =
                classify_bin(&resolved.source, &name, authoritative_tree, seed, binding)
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
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
    });
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
            e.id.type_id == PROPERTY_RESOURCE_TYPE
                && (e.id.group as u16) == PATH_ENTRY_GROUP_LOW
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
                transports.push(
                    instance_name(instance).unwrap_or_else(|| format!("0x{instance:08X}")),
                );
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
        let count = |c: PropBinCategory| {
            entries
                .iter()
                .filter(|e| e.category == c)
                .count()
        };
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
        for e in entries
            .iter()
            .filter(|e| !matches!(e.category, PropBinCategory::Misc | PropBinCategory::Vehicles))
        {
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
