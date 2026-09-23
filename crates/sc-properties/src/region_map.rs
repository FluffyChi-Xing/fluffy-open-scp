//! 区域地图渲染管线：341-tile 金字塔拼合 → 全局水位面着色 → PNG。
//! 供 OpenSCP 地图开发面板与探针共用。
//!
//! 机制结论（详见 docs/overview/glass-box/region-and-map.md §4/§5）：
//! - 每区域 341 张 256² u16 tile = 4096²×8m 区域高度图的 5 级 mip 金字塔，
//!   排布全游戏唯一（`SHARED_TILE_GRID`，RT0/RT1 槽位集合一致实证）；
//! - 水位面为全游戏统一常量 4928 raw（-870 m，引擎实证，§5）；
//! - ED（0x03E421ED，128² u32）为独立槽位的地面场金字塔：
//!   b0=植被密度+路网、b1=草量（草地着色）、b2=材质权重。
use std::collections::HashMap;

use image::{Rgb, RgbImage, Rgba};

/// 全游戏共享的 mip0 tile 排布（由 `tile_arrange` 像素级金字塔匹配复原，
/// BEAF0510 完美树：单根 0x7ADCBE88，节点 [1,4,16,64,256]）。
pub const SHARED_TILE_GRID: [[u32; 16]; 16] = [    [0x7ADCBE8C, 0x5D292B9B, 0xA7C1AE16, 0x1E279CE5, 0xA12CF3A0, 0x6854893F, 0xF92D745A, 0x5A72E609, 0xED4A03E4, 0x453A7813, 0x2103B675, 0x57D87926, 0x834E206B, 0x04BDD09C, 0x82FD4959, 0x9BE696EA],
    [0x4766E36B, 0xF9197D3C, 0x25152B55, 0x7BB03F26, 0x1C7CC92F, 0xC605CDF0, 0x410980D9, 0xABB3C34A, 0xE8112423, 0x4A192C94, 0xA3B03936, 0xFA4FD6E5, 0xB6C3FB8C, 0x52AF89FB, 0x3B213CDA, 0x315904A9],
    [0xAD4DAB7A, 0x435C67E9, 0xAB991ED8, 0xD2EF6037, 0x38CB831E, 0x4C37186D, 0xDA61A76C, 0x5E21B4BB, 0xADFF3472, 0x77AB6481, 0xA98C6847, 0xAF4678E8, 0x47307D79, 0xB98CBA8A, 0xD53C07CB, 0x9F0FEAFC],
    [0x0B494079, 0xADE8672A, 0xC3BEF927, 0xD31E3EE8, 0x51C249DD, 0x1AEB98AE, 0x93483F4B, 0xC82B5F5C, 0xE2133BD1, 0x466A5E62, 0x91668E78, 0xAF179A37, 0xE934E87A, 0x517B2449, 0x1C556FEC, 0x1C6DE15B],
    [0x710404B8, 0xC4AF6B77, 0xEB0FF21A, 0x6D637AA9, 0x73BBA1B4, 0xB7E1AB83, 0x89A1B256, 0x0B370845, 0x0C566190, 0x3AE0830F, 0x2BAD5D39, 0xB41946EA, 0xC78B8487, 0xB4BB7F88, 0x48E8F815, 0x3FA5C926],
    [0x8BA44787, 0xA9159928, 0x45DE5A99, 0xD7F10CEA, 0x3F70AFB3, 0xA3CC4344, 0x06F52F95, 0x551A8B06, 0xC4C4A55F, 0xBD164480, 0xE6FCEA3A, 0x498BB4A9, 0xACEB41B8, 0xE6734757, 0xCB957AD6, 0xE21D26E5],
    [0x59890776, 0x3C252625, 0xCEBE8E2C, 0x5878575B, 0x59ED4A82, 0x8275F331, 0x8AFD2718, 0xAC59AC17, 0x780E784E, 0x2A9A3E3D, 0x8117B52B, 0xB7429AFC, 0x1047C5B5, 0x8F342646, 0x00DDE7E7, 0x9713C8E8],
    [0xD46088B5, 0x6D6EB766, 0x6BDC750B, 0xDB1A60FC, 0x5BC213A1, 0x67AEA052, 0xBEEA1F67, 0xA6E0F548, 0x9287AC0D, 0x3E954BFE, 0xDE50A54C, 0x34A0915B, 0x95704476, 0x2E7CC485, 0xCCF0EF98, 0x96E4EA37],
    [0xA6040CC4, 0xA1677C33, 0x6E8AC84E, 0x9834631D, 0x471ECCA8, 0x896C7E07, 0x9FA6AAB2, 0xBEB168C1, 0xD8423E2C, 0xEADE327B, 0x9B107CAD, 0x58EB6D5E, 0xF600B203, 0xB1EA8414, 0xF9E8EF91, 0x9093FEC2],
    [0xBA197503, 0xD5B26E34, 0x8903FC0D, 0x7CC3335E, 0x62B89EF7, 0x6ECC3B38, 0xB7F52711, 0xB9AF73A2, 0x7560250B, 0x9CEC791C, 0x8098DBEE, 0x745C9D1D, 0xE1EB49C4, 0xAD0BCF93, 0xE19A7332, 0xABB57CE1],
    [0x02C34549, 0xB0401F16, 0x54AABA9B, 0x96187398, 0xBAAD9CED, 0xDD7B162A, 0x9D5F5BEF, 0xD28C5F4C, 0xBEBE1F21, 0x0409E9AE, 0x40CFF468, 0xEF8E39AB, 0xDB206CE6, 0x2521DD59, 0x62993C9C, 0x0D7354FF],
    [0x6AD4DB8A, 0x2D939C55, 0xF09B0C3C, 0xCA056BE7, 0x89621D2E, 0x72EF16E9, 0x1F96B060, 0x75536F2B, 0xBCE95602, 0x3555696D, 0x40A115B7, 0x4CC729CC, 0xA9D6DBA5, 0xDD45D0DA, 0xB08AF5FB, 0x5830AAB0],
    [0x03F7AAFB, 0xB4178FD8, 0x3ADDF6E9, 0x94BCFED6, 0xD6CB0DBF, 0x28AC2C3C, 0x8D19169D, 0xDB38A43A, 0xA26CBB33, 0xAF241EF0, 0xE961F4A6, 0x9A23E1B9, 0x16C74EA8, 0x77609BCB, 0x1768268A, 0xF154512D],
    [0xB605F19C, 0xCC3D6A27, 0xA569F62A, 0x12107C15, 0x347C5270, 0x8CBBDA9B, 0x71A7E6DE, 0x1FE91739, 0xD6B7AD34, 0x5172DA3F, 0x8BD95265, 0x55736EBA, 0x326120F7, 0xBE7A03EC, 0xAF569049, 0xD6DCB06E],
    [0xDFC4E585, 0xF38E631A, 0xBC30FA77, 0xE57CF3EC, 0xF0EC77B1, 0x9D016666, 0xBDD8D973, 0x85A24878, 0xB4BE555D, 0x50CCF152, 0x48CC167C, 0x1802ECC7, 0x1B9A1CAA, 0xEB0D8C15, 0x1296EB88, 0x2E456A43],
    [0x407C4746, 0x4E5CCB99, 0xA0972828, 0x9E638BCB, 0xD62524D2, 0x6BB7D525, 0xAC980574, 0x9DC82247, 0x9BC78E9E, 0x6B944431, 0xC62A0CDB, 0xFFDD12F8, 0xB10E1D69, 0x6DBA0ED6, 0x444EB357, 0x35F8B304],
];

/// 全游戏共享的 ED mip0 tile 排布（`ed_grid_cross` 实证：ED 槽位与 F0 完全独立
/// （交集 0），但跨 11 个区域组逐槽一致（0/256 差异）——区域无关，解一次即可。
/// 由 BEAF0510 金字塔匹配求得，BC357A2B/A0B60DDE/D01FA985 独立求解逐槽复核一致）。
pub const SHARED_ED_GRID: [[u32; 16]; 16] = [
    [0x10E6E1C4, 0x2B24E7F3, 0x59C694CE, 0xE1565E3D, 0xBCEB01F8, 0xB4A77CB7, 0xC687B712, 0x7EF138C1, 0x911DC76C, 0xC3323BFB, 0xFF77750D, 0x6AD8691E, 0xB6408203, 0x9D265094, 0x8FC7C111, 0xB7769EA2],
    [0x24FC4A03, 0x19E413F4, 0x743FC88D, 0xF5516BFE, 0xD510DBC7, 0xB4D65B68, 0xDED633F1, 0x79EF43A2, 0x4A045F4B, 0x7540829C, 0xE4FE414E, 0x83CF2FDD, 0xA22B19C4, 0x98479C13, 0x777944B2, 0xBC7893C1],
    [0xD4C93ED2, 0x49F0B4E1, 0xEE0E7E10, 0xF19CA30F, 0xCF34ACB6, 0xFFDFB965, 0x02A6ED04, 0x7644F453, 0xAFE9945A, 0xC1FDD649, 0x31B46E5F, 0xD339D9A0, 0x80D4C9B1, 0x99778262, 0xC204BA63, 0xB8A970D4],
    [0xEF9091B1, 0x2ECF36C2, 0xA67CC1DF, 0x73D26480, 0x4C8829F5, 0x5D685BA6, 0xFAF3A443, 0x7B2215D4, 0xF7C5A0D9, 0x2A0F6C8A, 0x79462A90, 0x4E89AF2F, 0x660D76D2, 0xCAB88881, 0xC9B67024, 0xB3CC4F53],
    [0x33206C70, 0x0AAB6A6F, 0x8FB750F2, 0x2E678481, 0x09D518CC, 0x3A3073DB, 0xA38AEA6E, 0x5BA3277D, 0x41B94718, 0x56F5F957, 0x4F0304D1, 0x8BFA3082, 0x66B35FBF, 0x10236880, 0x6C79AF4D, 0x679B5D3E],
    [0xD56F27BF, 0x8CE2BEE0, 0xC3CB5851, 0xFD267E62, 0xAC9C28AB, 0xBCD27D7C, 0xBE028B2D, 0x2A14023E, 0x75A63F67, 0x253E3188, 0x1AEEFD72, 0x8DCEF9A1, 0xC464A470, 0x8DEDA70F, 0x547AE48E, 0x992A827D],
    [0x8806372E, 0xFA65251D, 0xCF022064, 0xFBF69813, 0x12815DBA, 0x4F1B9729, 0x24DEE4B0, 0x265F394F, 0x405DD256, 0x9EFF7685, 0x5500ED23, 0xA5C887B4, 0x4A95EEED, 0x91A26FFE, 0x8BC5DF9F, 0xE896BEC0],
    [0xB951B6ED, 0xDEF3F55E, 0xC9C940A3, 0x00D54C94, 0x5731D0B9, 0xB9A9296A, 0xDA218EFF, 0xAB0F63C0, 0xBDB14F95, 0xFFB6D846, 0x5A39CCE4, 0x717D95B3, 0x194A6F2E, 0x7DA7623D, 0xD5D071D0, 0x63E6944F],
    [0xE5BF938C, 0xBAA9195B, 0x92FD7A96, 0x7D678D05, 0x2D7149B0, 0x8A32C4EF, 0x09EEF8BA, 0x1AB2B609, 0xA89FF624, 0x1D8E8193, 0x9B88A455, 0x3D84D0E6, 0x438DF06B, 0xEFF99D1C, 0x18DC1AD9, 0xA97AEE4A],
    [0xB249B86B, 0x3D4B22FC, 0x1050F7D5, 0xDE1EEEC6, 0xE2B3F3FF, 0x0C6A1960, 0x4E9F6BB9, 0x6BF3934A, 0xA0EE4063, 0x226D3614, 0x1E352716, 0x0C3B3FA5, 0x7703CB8C, 0x3DEB567B, 0xD1000E5A, 0x583A1109],
    [0xFBA7B201, 0xE9F4590E, 0x8424D7B3, 0x65E03EF0, 0x427FC1C5, 0xF90B1DE2, 0x7F5114A7, 0x94306884, 0x19044E29, 0x18C39876, 0x9252EA40, 0x58651603, 0xC8B3D51E, 0x87495011, 0xA5A4C194, 0x46D3EC77],
    [0xCA66ABE2, 0x01F323CD, 0xB86FC9B4, 0x082EFA3F, 0x8C634486, 0x2A4C2401, 0x672B3A58, 0x8C7D1FC3, 0x8391E06A, 0x939B19B5, 0x0DA2BFCF, 0x444FADC4, 0xE1AA9BDD, 0x6EFAD3B2, 0xA0C60D13, 0x2B3A1A28],
    [0xC936C593, 0x6B49E650, 0xA0763BA1, 0xBAC609AE, 0xE3A26597, 0xFCB9EC14, 0xE0A746D5, 0x58113292, 0x04192ADB, 0x303E95B8, 0x115788BE, 0x22F95DB1, 0x311545A0, 0xB9864963, 0xA1F5F362, 0xBE49A725],
    [0xCE157A14, 0x213F541F, 0x9EA17282, 0xEC11896D, 0xDE29AEC8, 0xF7DB3793, 0x6353C996, 0x705FAF71, 0x86BB347C, 0x4ADED887, 0x42E6ADFD, 0x08320AD2, 0xAC651B2F, 0xC137FF24, 0xD336F981, 0xEF933866],
    [0xAE968BBD, 0x0CF2B932, 0x6130F12F, 0x43A6B444, 0x91BB9F89, 0xF1360B7E, 0x276E908B, 0xB6686030, 0xC9C87065, 0x6C883C7A, 0x62659C54, 0x08D7F3BF, 0xE9D59C82, 0x63FB3E4D, 0x18A1D980, 0xDF4DAC9B],
    [0xC291997E, 0x25413591, 0xE5E11BA0, 0x57BC1C83, 0xE2FC7CCA, 0xDD3AFDBD, 0x8A50A9AC, 0x6BAB0A7F, 0x275112A6, 0xCA83D179, 0x5D887AD3, 0x66893870, 0xEBAA65A1, 0x4BFC738E, 0x966C180F, 0x7B3DFE3C],
];

/// 全游戏统一水位面（高度图 raw 值）。
///
/// 引擎证据链（2026-09-23，替代已撤回的 3336 猜测值）：
/// - 垂直映射（SimCity_App.package shader 源码容器 0x0469A3F7，terrain VS）：
///   HeightMap 为 UNORM u16，`height *= 2048; height -= 1024;`
///   即 **z(米) = raw/32 − 1024**，raw = (z+1024)×32；
/// - 水面世界高度 **-870 m**：exe 全二进制唯一浮点对 `-870.0, 1.4`
///   （SimCity.exe 0x9A36E4，位于 π/e/deg 常量页；1.4 = HLSL
///   `GetBeachTreeLine()`，`seaLevel = mWaterLevel.x + 1.4` 树线判定）；
/// - 区域 desc（0x51E7A18D）旁证：0x00FEAAB3=1024.0（高度半幅，同 shader
///   常量）、0x0E16BE1A=-870.0（水面），全部区域组值相同；
/// - 换算：(-870+1024)×32 = **4928**。旧值 3336（≈-919.75 m）偏低 49.75 m，
///   曾致地平线群岛海岸出现侵蚀状噪声出露。
pub const WATER_LEVEL: i32 = 4928;
/// 水面世界高度（米），shader/引擎侧常量。
pub const WATER_LEVEL_WORLD_Z: f32 = -870.0;
const HALF: f32 = 16384.0;
const CELL: f32 = 8.0;

/// 全部 RT0 区域 + 已知离线区域的显示名（组 id → 中文名 / 英文名）。
///
/// 名称链路（2026-09-24 破解）：JS 模板注册表（Game 包 67771F5C:40464200:7CCC548C）
/// 模板名→locale hash → locale JSON hash→中文名；模板↔组桥接 = 城市数 +
/// 官方图鉴地形目验（tmp/region_preview/source，2026-09-24 全部对上）：
/// 泰坦峽谷=狭长峡谷河流、三角洲=分叉水系、藍綠森林=排除法、荒涼=荒瘠山地核爆坑。
/// 例外：E0183D94/DB25018C 两张地形图几乎相同（教程地图复用），
/// 按官方 diorama 植被匹配分配，待游戏内存档 MetaData 终验。
const REGION_NAMES: &[(u32, &str, &str)] = &[
    (0xBEAF_0510, "白水谷", "Whitewater Valley"),   // Confluence；存档 MetaData 佐证
    (0x9F73_5B20, "绵延不毛之地", "Oasis"),         // 离线版包
    (0xD01F_A985, "地平线群岛", "Horizon Keys"),    // 11 城 + 3 伟工，官方一致
    (0x9E9B_1FF0, "大理石湖", "Caspian Lake"),      // 10 城，唯一
    (0xC2A9_C48F, "三一岬", "Cape Trinity"),        // 3 城，唯一
    (0xBC35_7A2B, "泰坦峡谷", "Titan Gorge"),       // 16 城；峡谷河流目验
    (0xB12D_E348, "三角洲", "Discovery Delta"),     // 16 城；分叉水系目验
    (0xC041_82E4, "藍綠森林", "Viridian Woods"),    // 16 城；排除法（官方繁中名沿用）
    (0xA0B6_0DDE, "荒涼", "Desolation"),            // 7 城；荒瘠山地+核爆坑目验
    (0xE41A_82B8, "愛華特灣", "Edgewater Bay"),     // 7 城；环抱海湾目验
    (0xE018_3D94, "追日灣", "Twin Cities"),         // 2 城；与 DB25018C 为复用双图，待终验
    (0xDB25_018C, "奮進島", "Tutorial"),            // 2 城；同上，待终验
];

/// 按语言取区域显示名（zh = 中文名，其余 = 英文名）；未收录返回 None。
pub fn region_display_name(group: u32, zh: bool) -> Option<&'static str> {
    REGION_NAMES
        .iter()
        .find(|(id, _, _)| *id == group)
        .map(|(_, zh_name, en_name)| if zh { *zh_name } else { *en_name })
}

/// 区域摘要。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegionSummary {
    pub group: u32,
    /// 玩家可辨识的显示名（中文，未确认的为 None）。
    pub display_name: Option<String>,
    /// 英文名（模板内部名，未确认的为 None）。
    pub display_name_en: Option<String>,
    /// 区域描述内的数字串（= 区域 UI id；组 id = FNV1(数字串)）。
    pub numeric_id: String,
    /// 城市地块数（不含伟大工程位）。
    pub plot_count: usize,
}

/// 渲染输出。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegionRender {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// 裁剪框的世界坐标（左上角）。
    pub origin_world: (f32, f32),
    /// 每米对应的像素数（恒 1/8）。
    pub meters_per_pixel: f32,
    pub water_plane: i32,
    pub desert: bool,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
    pub plots: Vec<(f32, f32)>,
    /// 资源画刷：目标 map 名 + 各 stamp 世界坐标。
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
    /// 资源分布图层（kind, RGBA PNG）——游戏数据视图同款等值线色带，
    /// 尺寸 = 裁剪框 1/2（前端按裁剪框尺寸放大显示）。
    pub resource_layers: Vec<(String, Vec<u8>)>,
}

fn fnv1(s: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in s.bytes() {
        h = h.wrapping_mul(0x0100_0193);
        h ^= b as u32;
    }
    h
}

/// 枚举包内全部区域（F0 tile 计数 = 341 的组），附 UI id 与城市地块数。
pub fn list_regions(package: &dbpf::Package) -> Vec<RegionSummary> {
    let mut f0cnt: HashMap<u32, usize> = HashMap::new();
    let mut numeric: HashMap<u32, String> = HashMap::new();
    let mut region_cities: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in package.entries() {
        match e.id.type_id {
            0x03E4_21F0 => {
                *f0cnt.entry(e.id.group).or_default() += 1;
            }
            0x00B1_B104 => {
                if let Some(pf) = sc_parse(package, e) {
                    // 区域描述：数字 UI id
                    if e.id.instance == 0x51E7_A18D {
                        if let Some(p) = pf.get(0x4EE9_7F5B) {
                            if let crate::Kind::Scalar(crate::Value::String8(s)) = &p.kind {
                                numeric.insert(e.id.group, s.clone());
                            }
                        }
                    }
                    // 城市组：36B property 显式引用区域 desc
                    if e.compressed_size <= 60 {
                        if let Some(crate::Property { kind: crate::Kind::Scalar(crate::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
                            if k.instance == 0x51E7_A18D {
                                region_cities.entry(k.group).or_default().push(e.id.group);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (g, n) in f0cnt {
        if n != 341 {
            continue;
        }
        let plot_count = region_cities.get(&g).map(|c| c.len()).unwrap_or(0);
        let nid = numeric.get(&g).cloned().unwrap_or_default();
        let display_name = region_display_name(g, true).map(|s| s.to_string());
        let display_name_en = region_display_name(g, false).map(|s| s.to_string());
        out.push(RegionSummary {
            group: g,
            display_name,
            display_name_en,
            numeric_id: nid,
            plot_count,
        });
    }
    out.sort_by_key(|r| r.numeric_id.clone());
    out
}

fn sc_parse(package: &dbpf::Package, e: &dbpf::IndexEntry) -> Option<crate::PropertyFile> {
    let data = package.read(e).ok()?;
    crate::PropertyFile::parse(&data).ok()
}

/// 区域地块位置（通过城市 id 交集选择正确的地块表）。
pub fn plot_positions(package: &dbpf::Package, group: u32) -> Vec<(f32, f32)> {
    let mut city_ids: Vec<u32> = Vec::new();
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        if let Some(pf) = sc_parse(package, e) {
            if let Some(crate::Property { kind: crate::Kind::Scalar(crate::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
                if k.instance == 0x51E7_A18D && k.group == group {
                    city_ids.push(e.id.group);
                }
            }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    let mut best: Option<(usize, Vec<(f32, f32)>)> = None;
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C {
            continue;
        }
        let Some(pt) = sc_parse(package, e) else { continue };
        let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pt.get(0x16B7_B1EF) else { continue };
        let ids: Vec<u32> = vs.iter().filter_map(|v| match v { crate::Value::UInt32(x) => Some(*x), _ => None }).collect();
        let ov = ids.iter().filter(|i| set.contains(i)).count();
        if best.as_ref().map(|(b, _)| ov > *b).unwrap_or(true) {
            if let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) {
                let pos: Vec<(f32, f32)> = vs
                    .iter()
                    .filter_map(|v| match v { crate::Value::Vector2(v) => Some((v[0], v[1])), _ => None })
                    .collect();
                best = Some((ov, pos));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

/// 资源画刷清单（目标 map 名 + stamp 世界坐标）。
pub fn resource_brushes(package: &dbpf::Package, group: u32) -> Vec<(String, Vec<(f32, f32)>)> {
    let mut out = Vec::new();
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != group {
            continue;
        }
        let Some(pf) = sc_parse(package, e) else { continue };
        let Some(p) = pf.get(0x00B2_CCCA) else { continue };
        let crate::Kind::Scalar(crate::Value::String8(name)) = &p.kind else { continue };
        if !(name.ends_with("brushes") || name.ends_with("Brushes")) || name == "brushes" {
            continue;
        }
        let mut stamps = Vec::new();
        if let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pf.get(0x02A9_07B6) {
            for v in vs {
                if let crate::Value::Transform(t) = v {
                    if t.matrix.len() >= 11 {
                        stamps.push((t.matrix[9], t.matrix[10]));
                    }
                }
            }
        }
        out.push((name.clone(), stamps));
    }
    out
}

/// 渲染区域彩色俯视图（PNG）。使用全游戏共享 ED 网格（[`SHARED_ED_GRID`]）。
/// `water_override` 显式覆盖水位 raw 值；默认从区域 desc 0x0E16BE1A 读取
/// （世界米 → raw），缺失时退回 [`WATER_LEVEL`]。
pub fn render_region_png(
    package: &dbpf::Package,
    group: u32,
    water_override: Option<i32>,
) -> Result<RegionRender, Box<dyn std::error::Error>> {
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = package.read(e)?;
            if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }
        }
    }
    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..256 {
                    hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256]
                        .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    // ED 场（草量）：全游戏共享网格，直接取该区域数据
    let eg = &SHARED_ED_GRID;
    let mut ed0 = vec![0u8; w * w];
    let mut ed1 = vec![0u8; w * w];
    {
        let mut tmp0 = vec![0u8; w * w];
        let mut tmp1 = vec![0u8; w * w];
        for (ty, row) in eg.iter().enumerate() {
            for (tx, inst) in row.iter().enumerate() {
                let Some(px) = ed_tile(package, group, *inst) else { continue };
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        let k = (ty * 128 + y) * 2048 + tx * 128 + x;
                        tmp0[k] = (v & 0xFF) as u8;
                        tmp1[k] = ((v >> 8) & 0xFF) as u8;
                    }
                }
            }
        }
        // 最近邻上采样 ×2
        for y in 0..w { for x in 0..w {
            let k = (y / 2) * 2048 + (x / 2);
            ed0[y * w + x] = tmp0[k];
            ed1[y * w + x] = tmp1[k];
        }}
    }
    let _ = &mut ed0;

    let plots = plot_positions(package, group);
    let brushes = resource_brushes(package, group);
    let sea = water_override
        .unwrap_or_else(|| region_water_level(package, group).unwrap_or(WATER_LEVEL));

    // 荒漠判定：land 均值 b1
    let mut b1_sum = 0f64;
    let mut b1_n = 0f64;
    for y in (0..w).step_by(4) {
        for x in (0..w).step_by(4) {
            let h = hgt[y * w + x];
            if h > (sea + 100) as u16 && h != 0 {
                b1_sum += f64::from(ed1[y * w + x]);
                b1_n += 1.0;
            }
        }
    }
    let mean_b1 = if b1_n > 0.0 { b1_sum / b1_n } else { 0.0 };
    let desert = mean_b1 < 20.0;
    // 裁剪：地块包围盒 + 2560m
    let mut x0c = w; let mut x1c = 0usize; let mut y0c = w; let mut y1c = 0usize;
    if !plots.is_empty() {
        for (wx, wy) in &plots {
            let cx = ((wx + HALF) / CELL) as usize;
            let cy = ((wy + HALF) / CELL) as usize;
            x0c = x0c.min(cx.saturating_sub(320));
            x1c = x1c.max(cx + 320);
            y0c = y0c.min(cy.saturating_sub(320));
            y1c = y1c.max(cy + 320);
        }
        x0c = x0c.saturating_sub(320);
        x1c = (x1c + 320).min(w);
        y0c = y0c.saturating_sub(320);
        y1c = (y1c + 320).min(w);
    } else {
        x0c = 0; x1c = w; y0c = 0; y1c = w;
    }

    let h_at = |x: usize, y: usize| -> i32 { hgt[y.min(w - 1) * w + x.min(w - 1)] as i32 };
    // 水陆判定用 4×4 box 滤波高度（≈引擎 mip 降采样语义）：设计期雕刻噪声在
    // 游戏内被区域视图的 mip 选择滤除，逐格判定会呈现侵蚀状碎岛假象
    let sm_n = w / 4;
    let mut hsm = vec![0u32; sm_n * sm_n];
    for y in 0..sm_n {
        for x in 0..sm_n {
            let mut s = 0u32;
            for dy in 0..4 {
                for dx in 0..4 {
                    s += u32::from(hgt[(y * 4 + dy) * w + x * 4 + dx]);
                }
            }
            hsm[y * sm_n + x] = s / 16;
        }
    }
    let mut img = RgbImage::new((x1c - x0c) as u32, (y1c - y0c) as u32);
    for y in y0c..y1c {
        for x in x0c..x1c {
            let h = h_at(x, y);
            let dx = h_at(x + 1, y) - h;
            let dy = h_at(x, y + 1) - h;
            let (nx, ny, nz) = (-(dx as f32), -(dy as f32), 60.0f32);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let light = ((nx * 0.5 + ny * 0.5 + nz * 0.7) / nl).max(0.0);
            let rel = hsm[(y / 4) * sm_n + x / 4] as i32 - sea;
            let (mut r, mut g, mut b);
            if rel < 0 || h == 0 {
                // 水面：平整着色（引擎水面为平面，不透射海底地形光照），
                // 仅按滤波深度做深浅渐变
                let d = (-rel as f32 / 600.0).clamp(0.0, 1.0);
                r = 90.0 - 50.0 * d;
                g = 140.0 - 60.0 * d;
                b = 190.0 - 60.0 * d;
            } else {
                let grass = ed1[y * w + x];
                let grass_k = if desert { 0.0 } else { (grass as f32 / 120.0).clamp(0.0, 1.0) };
                let slope = ((dx * dx + dy * dy) as f32).sqrt() / 60.0;
                let slope_k = (slope / 1.6).clamp(0.0, 1.0);
                let g_amt = (grass_k * (1.0 - slope_k * 0.85)).clamp(0.0, 1.0);
                if rel < 350 {
                    r = 190.0; g = 175.0; b = 130.0;
                } else {
                    let t = ((rel - 350) as f32 / 6000.0).clamp(0.0, 1.0);
                    let rock_r = 150.0 + 50.0 * t;
                    let rock_g = 135.0 + 25.0 * t;
                    let rock_b = 100.0 + 35.0 * t;
                    r = rock_r + (95.0 - rock_r) * g_amt;
                    g = rock_g + (160.0 - rock_g) * g_amt;
                    b = rock_b + (70.0 - rock_b) * g_amt;
                }
                if !desert && (100..=120).contains(&ed0[y * w + x]) {
                    let d = 1.0 - (ed0[y * w + x] as f32 - 100.0) / 20.0;
                    r *= 1.0 - 0.5 * d; g *= 1.0 - 0.1 * d; b *= 1.0 - 0.5 * d;
                }
                if ed0[y * w + x] == 15 { r = 130.0; g = 122.0; b = 112.0; }
                let shade = 0.45 + 0.55 * light;
                r *= shade; g *= shade; b *= shade;
            }
            img.put_pixel((x - x0c) as u32, (y - y0c) as u32, Rgb([r as u8, g as u8, b as u8]));
        }
    }

    // 区域路网：ED b0==0（陆地=道路，水面=桥梁段），ED 16m/格 → 渲染 8m/px 邻近放大。
    // void 格（h==0）排除——未雕刻区的 b0 噪点会在外海呈现散点假象。
    // 地块框与资源环不再烘焙进 PNG——由前端 SVG 覆盖层绘制（可逐层开关）。
    for y in y0c..y1c {
        for x in x0c..x1c {
            if ed0[y * w + x] == 0 && hgt[y * w + x] != 0 {
                img.put_pixel((x - x0c) as u32, (y - y0c) as u32, Rgb([60, 58, 56]));
            }
        }
    }

    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(
        &mut std::io::Cursor::new(&mut png),
        image::ImageFormat::Png,
    )?;
    let origin_world = (x0c as f32 * CELL - HALF, y0c as f32 * CELL - HALF);
    let resource_layers = synthesize_resource_layers(
        &brushes,
        &plots,
        origin_world,
        (x1c - x0c) as f32 * CELL,
        (y1c - y0c) as f32 * CELL,
        group,
    );
    Ok(RegionRender {
        png,
        width: (x1c - x0c) as u32,
        height: (y1c - y0c) as u32,
        origin_world,
        meters_per_pixel: 8.0,
        water_plane: sea,
        desert,
        display_name: region_display_name(group, true).map(|s| s.to_string()),
        display_name_en: region_display_name(group, false).map(|s| s.to_string()),
        plots,
        brushes,
        resource_layers,
    })
}

/// 资源 kind 基色（数据视图色带锚点，与前端 RESOURCE_COLORS 对应）。
fn resource_base_color(kind_lower: &str) -> [u8; 3] {
    match kind_lower {
        "coal" => [90, 82, 74],
        "oil" => [58, 54, 50],
        "ore" => [196, 88, 34],
        "watertable" => [47, 110, 200],
        "soil" => [160, 106, 59],
        "forest" => [46, 140, 62],
        "desirability" => [200, 80, 80],
        "desirabilitytwo" => [150, 86, 178],
        "radiation" => [58, 190, 88],
        "groundpollution" => [156, 62, 190],
        _ => [200, 60, 200],
    }
}

/// 确定性二维值噪声（hash 格点 + smoothstep 插值，两倍频）。
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    fn hash(ix: i32, iy: i32, seed: u32) -> f32 {
        let mut h = (ix as u32).wrapping_mul(0x27D4EB2D)
            ^ (iy as u32).wrapping_mul(0x165667B1)
            ^ seed.wrapping_mul(0x9E3779B9);
        h ^= h >> 15;
        h = h.wrapping_mul(0x85EBCA6B);
        h ^= h >> 13;
        (h & 0xFFFF) as f32 / 65535.0
    }
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let octave = |fx: f32, fy: f32, s: u32| -> f32 {
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let tx = smooth(fx - fx.floor());
        let ty = smooth(fy - fy.floor());
        hash(ix, iy, s) * (1.0 - tx) * (1.0 - ty)
            + hash(ix + 1, iy, s) * tx * (1.0 - ty)
            + hash(ix, iy + 1, s) * (1.0 - tx) * ty
            + hash(ix + 1, iy + 1, s) * tx * ty
    };
    0.65 * octave(x, y, seed) + 0.35 * octave(x * 2.3 + 11.0, y * 2.3 + 7.0, seed ^ 0x5F356495)
}

/// 合成资源分布图层：128² 场 → 双线性放大 → 四级色带（对齐游戏数据视图观感）。
/// 场源 = 画刷 stamp（数据驱动中心）+ 每城市地块的确定性伴生矿藏
/// （地块序号 hash 决定资源种类/强度/偏移——游戏里每城都有多种资源）。
/// 尺寸 = 裁剪框的 1/2（前端放大显示）。
fn synthesize_resource_layers(
    brushes: &[(String, Vec<(f32, f32)>)],
    plots: &[(f32, f32)],
    origin_world: (f32, f32),
    crop_w_m: f32,
    crop_h_m: f32,
    group: u32,
) -> Vec<(String, Vec<u8>)> {
    const CELL_M: f32 = 256.0; // 与引擎 typed map 同分辨率语义：128 格覆盖区域
    const STAMP_R_M: f32 = 1400.0; // 画刷 stamp 斑块半径（外带）
    const PLOT_R_BASE_M: f32 = 700.0; // 地块伴生矿藏基础半径
    let gw = ((crop_w_m / CELL_M).ceil() as usize).max(2);
    let gh = ((crop_h_m / CELL_M).ceil() as usize).max(2);
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (name, stamps) in brushes {
        let kind = name
            .strip_suffix("EcoMapBrushes")
            .unwrap_or(name)
            .to_string();
        let kind_lower = kind.to_lowercase();
        if stamps.is_empty() || !seen.insert(kind_lower.clone()) {
            continue;
        }
        let seed = group ^ fnv1(&kind);
        let base = resource_base_color(&kind_lower);
        // 场源：stamp（强）+ 地块伴生（确定性分配）
        let mut sources: Vec<(f32, f32, f32, f32)> = stamps // (wx, wy, r_m, strength)
            .iter()
            .map(|&(sx, sy)| (sx, sy, STAMP_R_M, 1.0))
            .collect();
        for (k, (px, py)) in plots.iter().enumerate() {
            let h = group ^ (k as u32).wrapping_mul(0x9E37_79B9) ^ fnv1(&kind);
            if h % 3 == 0 {
                continue; // 部分地块无该资源，避免全图均匀铺满
            }
            let angle = (((h >> 16) % 360) as f32).to_radians();
            // 中心偏移限幅：deposit 保持在地块框内（框半宽 1024m）
            let dist = 200.0 + ((h >> 20) % 500) as f32;
            let cx = px + angle.cos() * dist;
            let cy = py + angle.sin() * dist;
            let r_m = PLOT_R_BASE_M + ((h >> 12) % 3) as f32 * 250.0;
            let strength = 0.55 + ((h >> 8) % 3) as f32 * 0.15;
            sources.push((cx, cy, r_m, strength));
        }
        let inside_any_plot = |wx: f32, wy: f32| -> bool {
            plots.iter().any(|(px, py)| {
                (wx - px).abs() <= 1024.0 && (wy - py).abs() <= 1024.0
            })
        };
        let mut field = vec![0f32; gw * gh];
        for iy in 0..gh {
            for ix in 0..gw {
                let wx = origin_world.0 + (ix as f32 + 0.5) * crop_w_m / gw as f32;
                let wy = origin_world.1 + (iy as f32 + 0.5) * crop_h_m / gh as f32;
                // 可游玩区域（城市地块框）之外不存在资源
                if !inside_any_plot(wx, wy) {
                    continue;
                }
                let n = value_noise(wx / 700.0, wy / 700.0, seed);
                let mut f = 0f32;
                for &(sx, sy, r_m, strength) in &sources {
                    let dx = wx - sx;
                    let dy = wy - sy;
                    let r = (dx * dx + dy * dy).sqrt();
                    let v = (1.0 - r / (r_m * (0.8 + 0.4 * n))).max(0.0);
                    f = f.max(v * strength * (0.8 + 0.45 * n));
                }
                field[iy * gw + ix] = f;
            }
        }
        // 双线性放大到裁剪框 1/2 + 色带
        let ow = (crop_w_m / 16.0).round().max(2.0) as usize;
        let oh = (crop_h_m / 16.0).round().max(2.0) as usize;
        let mut img = image::RgbaImage::new(ow as u32, oh as u32);
        // 色带：从最深档（高阈值）向浅判定，mix_white 越小越深
        let bands: [(f32, f32); 4] = [(0.76, 0.10), (0.55, 0.34), (0.34, 0.58), (0.16, 0.80)];
        for py in 0..oh {
            for px in 0..ow {
                let fx = (px as f32 + 0.5) * gw as f32 / ow as f32 - 0.5;
                let fy = (py as f32 + 0.5) * gh as f32 / oh as f32 - 0.5;
                let x0 = fx.floor().max(0.0) as usize;
                let y0 = fy.floor().max(0.0) as usize;
                let x1 = (x0 + 1).min(gw - 1);
                let y1 = (y0 + 1).min(gh - 1);
                let tx = (fx - x0 as f32).clamp(0.0, 1.0);
                let ty = (fy - y0 as f32).clamp(0.0, 1.0);
                let v = field[y0 * gw + x0] * (1.0 - tx) * (1.0 - ty)
                    + field[y0 * gw + x1] * tx * (1.0 - ty)
                    + field[y1 * gw + x0] * (1.0 - tx) * ty
                    + field[y1 * gw + x1] * tx * ty;
                let mut c = [0u8, 0, 0, 0];
                for &(threshold, mix_white) in &bands {
                    if v >= threshold {
                        // 深色基色 → 向白色按层混出外带
                        let dark = [base[0] as f32 * 0.72, base[1] as f32 * 0.72, base[2] as f32 * 0.72];
                        c = [
                            (dark[0] + (255.0 - dark[0]) * mix_white) as u8,
                            (dark[1] + (255.0 - dark[1]) * mix_white) as u8,
                            (dark[2] + (255.0 - dark[2]) * mix_white) as u8,
                            255,
                        ];
                        break;
                    }
                }
                img.put_pixel(px as u32, py as u32, image::Rgba(c));
            }
        }
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_ok()
        {
            out.push((kind, png));
        }
    }
    out
}



fn ed_tile(package: &dbpf::Package, group: u32, inst: u32) -> Option<Vec<u32>> {
    let e = package.entries().iter().find(|e| e.id.type_id == 0x03E4_21ED && e.id.group == group && e.id.instance == inst)?;
    let d = package.read(e).ok()?;
    if d.len() != 65556 {
        return None;
    }
    Some(d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// 从区域 desc（0x51E7A18D）读水面世界高度（0x0E16BE1A，米），换算 raw。
/// 与引擎常量 -870.0 相同时即为 [`WATER_LEVEL`]；属性缺失返回 None。
pub fn region_water_level(package: &dbpf::Package, group: u32) -> Option<i32> {
    let e = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == 0x51E7_A18D)?;
    let data = package.read(e).ok()?;
    let pf = crate::PropertyFile::parse(&data).ok()?;
    let p = pf.get(0x0E16_BE1A)?;
    match &p.kind {
        crate::Kind::Scalar(crate::Value::Float(z)) => Some(((z + 1024.0) * 32.0).round() as i32),
        _ => None,
    }
}

/// ED 金字塔网格求解（该区域 ED 槽位独立，需单独匹配；结果可缓存）。
pub fn ed_grid_for_region(package: &dbpf::Package, group: u32) -> Option<[[u32; 16]; 16]> {
    let mut tiles: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let d = package.read(e).ok()?;
            if d.len() == 65556 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
            }
        }
    }
    if tiles.len() != 341 {
        return None;
    }
    let insts: Vec<u32> = tiles.keys().copied().collect();
    let mut ds: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for (&i, px) in &tiles {
        let mut b = vec![(0u32, 0u32); 64 * 64];
        for y in 0..64 {
            for x in 0..64 {
                let k = (2 * y) * 128 + 2 * x;
                let lane = |v: u32, s: u32| ((v >> (s * 8)) & 0xFF) as u32;
                b[y * 64 + x] = (
                    (lane(px[k], 0) + lane(px[k + 1], 0) + lane(px[k + 128], 0) + lane(px[k + 129], 0)) / 4,
                    (lane(px[k], 2) + lane(px[k + 1], 2) + lane(px[k + 128], 2) + lane(px[k + 129], 2)) / 4,
                );
            }
        }
        ds.insert(i, b);
    }
    let mut assign: HashMap<u32, (u32, usize, f64)> = HashMap::new();
    for &child in &insts {
        let cb = &ds[&child];
        let mut best = (0u32, 0usize, f64::INFINITY);
        for &parent in &insts {
            if parent == child { continue; }
            let pp = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 64, (q / 2) * 64);
                let mut e = 0f64;
                for y in 0..64 {
                    let prow = (qy + y) * 128 + qx;
                    let crow = y * 64;
                    for x in 0..64 {
                        let pv = pp[prow + x];
                        let ce = cb[crow + x];
                        e += (f64::from((pv & 0xFF) as u8) - f64::from(ce.0 as u8)).abs()
                            + (f64::from(((pv >> 16) & 0xFF) as u8) - f64::from(ce.1 as u8)).abs();
                    }
                }
                e /= 4096.0;
                if e < best.2 { best = (parent, q, e); }
            }
        }
        assign.insert(child, best);
    }
    let mut ch: Vec<(u32, u32, usize, f64)> = assign.iter().map(|(c, (p, q, e))| (*c, *p, *q, *e)).collect();
    ch.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let mut tree: HashMap<u32, [Option<u32>; 4]> = HashMap::new();
    let mut used_c = std::collections::HashSet::new();
    let mut used_q = std::collections::HashSet::new();
    for (c, p, q, e) in &ch {
        if used_c.contains(c) || used_q.contains(&(*p, *q)) || *e > 60.0 { continue; }
        tree.entry(*p).or_default()[*q] = Some(*c);
        used_c.insert(*c);
        used_q.insert((*p, *q));
    }
    let cof: HashMap<u32, (u32, usize)> = used_c
        .iter()
        .map(|c| {
            let (p, q, _) = assign[c];
            (*c, (p, q))
        })
        .collect();
    let isp: std::collections::HashSet<u32> = tree.keys().copied().collect();
    let roots: Vec<u32> = isp.iter().copied().filter(|x| !cof.contains_key(x)).collect();
    let root = *roots.first()?;
    let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
    pos.insert(root, (0, 0, 0));
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let (x, y, l) = pos[&n];
        if let Some(sl) = tree.get(&n) {
            for (q, c) in sl.iter().enumerate() {
                if let Some(c) = c {
                    pos.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, l + 1));
                    stack.push(*c);
                }
            }
        }
    }
    let mut grid = [[0u32; 16]; 16];
    for (i, (x, y, l)) in &pos {
        if *l == 4 { grid[*y][*x] = *i; }
    }
    Some(grid)
}
