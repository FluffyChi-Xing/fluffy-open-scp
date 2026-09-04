//! Anim（0x70001）关键帧动画解析。
//!
//! 对齐 C# `Anim.Read`（AN000 / AN-channels / AN-names / AN-info 防护，
//! 移植自 SporeModder `rw4_base.KeyframeAnim`）：
//! ```text
//! 12×u32 头：pNames, count, skeleton_id, field_C, pData, pPaddingEnd,
//!            count(重复), field_1C, length(f32), field_24, flags, pInfo
//! pNames:    count×u32 关节名 FNV（对应 HierarchyInfo item）
//! pInfo:     count×(数据相对偏移 u32, poseSize u32, components u32)
//! 数据:      components 0x101 LocRot(36B) | 0x601 LocRotScale(48B) | 0x100 BlendFactor(8B)
//! ```
//! 关键帧数按相邻通道数据的间隔推断；末通道读到时间回退或 section 末尾。
//! 未知 components 的通道不产出关键帧（C# `stride <= 0 → continue`）。

use crate::error::{Error, Result};
use crate::model::Rw4File;
use crate::reader::Reader;
use crate::section::SectionType;

/// Pose 格式：旋转 + 平移（SimCity 主用）。
pub const COMPONENTS_LOC_ROT: u32 = 0x101;
/// Pose 格式：旋转 + 平移 + 缩放（Spore LocRotScale）。
pub const COMPONENTS_LOC_ROT_SCALE: u32 = 0x601;
/// Pose 格式：混合因子（非变换轨道）。
pub const COMPONENTS_BLEND_FACTOR: u32 = 0x100;

/// 单个关键帧：四元数（xyzw）+ 平移 + 缩放（无缩放格式时为 1）+ 时间（秒）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub qx: f32,
    pub qy: f32,
    pub qz: f32,
    pub qw: f32,
    pub tx: f32,
    pub ty: f32,
    pub tz: f32,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
    pub time: f32,
}

/// 一条关节动画通道。
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// 关节名 FNV（对应 HierarchyInfo item）。
    pub id: u32,
    /// `0x601` LocRotScale | `0x101` LocRot | `0x100` BlendFactor。
    pub components: u32,
    /// 每关键帧存储字节数。
    pub pose_size: u32,
    pub keys: Vec<Key>,
}

/// 已解码动画：通道按骨骼名 FNV 关联到骨架层级。
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAnim {
    /// 目标骨架 id（对应 HierarchyInfo.id）。
    pub skeleton_id: u32,
    /// 动画总时长（秒）。
    pub length: f32,
    pub flags: u32,
    pub field_c: u32,
    pub field_1c: u32,
    pub field_24: u32,
    pub channels: Vec<Channel>,
}

impl DecodedAnim {
    /// 每格式 pose 步长（C# `PoseSizeFor`）；未知格式为 0。
    pub fn pose_size_for(components: u32) -> u32 {
        match components {
            COMPONENTS_LOC_ROT_SCALE => 48,
            COMPONENTS_LOC_ROT => 36,
            COMPONENTS_BLEND_FACTOR => 8,
            _ => 0,
        }
    }
}

impl Rw4File {
    /// 解码一个动画 section。
    pub fn decode_anim(&self, data: &[u8], number: u32) -> Result<DecodedAnim> {
        let section = self.section_for(number, "AN000", SectionType::ANIM)?;
        let payload = self.payload(data, section.number)?;
        parse_anim(payload, section.pos as u64)
    }
}

pub(crate) fn parse_anim(payload: &[u8], base: u64) -> Result<DecodedAnim> {
    let mut r = Reader::new(payload);
    let p_names = r.u32("AN_pNames")?;
    let channel_count = r.u32("AN_count")?;
    let skeleton_id = r.u32("AN_skeleton_id")?;
    let field_c = r.u32("AN_fieldC")?;
    let _p_data = r.u32("AN_pData")?;
    let _p_padding_end = r.u32("AN_pPaddingEnd")?;
    let count_repeat = r.u32("AN_count2")?;
    let field_1c = r.u32("AN_field1C")?;
    let length = r.f32("AN_length")?;
    let field_24 = r.u32("AN_field24")?;
    let flags = r.u32("AN_flags")?;
    let p_info = r.u32("AN_pInfo")?;

    // C# 快速失败防护：畸形/外来 section 在此报错（调用方跳过动画）
    if channel_count == 0 || channel_count > 4096 {
        return Err(Error::UnexpectedValue {
            check: "AN-channels",
            expected: 4096,
            actual: channel_count as u64,
        });
    }
    if count_repeat != channel_count {
        return Err(Error::UnexpectedValue {
            check: "AN-count2",
            expected: channel_count as u64,
            actual: count_repeat as u64,
        });
    }
    let names_bytes = channel_count as usize * 4;
    // pNames/pInfo 为绝对文件偏移（C# 以整文件流 seek），转 payload 内相对值
    let names_rel = (p_names as u64)
        .checked_sub(base)
        .ok_or(Error::UnexpectedValue {
            check: "AN-names-base",
            expected: base,
            actual: p_names as u64,
        })? as usize;
    if names_rel > payload.len().saturating_sub(names_bytes) {
        return Err(Error::InsufficientPayload {
            check: "AN-names",
            needed: names_rel + names_bytes,
            actual: payload.len(),
        });
    }
    let info_bytes = channel_count as usize * 12;
    let info_rel = (p_info as u64)
        .checked_sub(base)
        .ok_or(Error::UnexpectedValue {
            check: "AN-info-base",
            expected: base,
            actual: p_info as u64,
        })? as usize;
    if info_rel > payload.len().saturating_sub(info_bytes) {
        return Err(Error::InsufficientPayload {
            check: "AN-info",
            needed: info_rel + info_bytes,
            actual: payload.len(),
        });
    }

    let mut ids = Vec::with_capacity(channel_count as usize);
    r.seek(names_rel)?;
    for _ in 0..channel_count {
        ids.push(r.u32("AN_id")?);
    }

    let mut positions = Vec::with_capacity(channel_count as usize);
    let mut pose_sizes = Vec::with_capacity(channel_count as usize);
    let mut components = Vec::with_capacity(channel_count as usize);
    r.seek(info_rel)?;
    for _ in 0..channel_count {
        positions.push(r.u32("AN_pos")?);
        pose_sizes.push(r.u32("AN_poseSize")?);
        components.push(r.u32("AN_components")?);
    }

    let mut channels = Vec::with_capacity(channel_count as usize);
    for i in 0..channel_count as usize {
        let stride = if pose_sizes[i] != 0 {
            pose_sizes[i]
        } else {
            DecodedAnim::pose_size_for(components[i])
        };
        let mut channel = Channel {
            id: ids[i],
            components: components[i],
            pose_size: pose_sizes[i],
            keys: Vec::new(),
        };
        if stride == 0 {
            channels.push(channel);
            continue; // 未知 pose 格式 → 无关键帧
        }

        let start = positions[i] as usize;
        if start >= payload.len() {
            channels.push(channel);
            continue; // 坏偏移 → 跳过通道
        }
        // 关键帧数 = 到下一通道数据的间隔；末通道到 section 末尾
        let avail = payload.len() - start;
        let span = if i + 1 < channel_count as usize && positions[i + 1] as usize > start {
            positions[i + 1] as usize - start
        } else {
            avail
        };
        let count = span.min(avail) / stride as usize;

        r.seek(start)?;
        let stride = stride as usize;
        let is_last = i + 1 == channel_count as usize;
        let mut last_time = f32::NEG_INFINITY;
        for _ in 0..count {
            if r.pos() + stride > payload.len() {
                break; // 绝不越界
            }
            let key = read_key(&mut r, components[i], stride)?;
            // 末通道精确数未知：时间回退即停止
            if is_last && key.time < last_time {
                break;
            }
            last_time = key.time;
            channel.keys.push(key);
        }
        channels.push(channel);
    }

    Ok(DecodedAnim {
        skeleton_id,
        length,
        flags,
        field_c,
        field_1c,
        field_24,
        channels,
    })
}

fn read_key(r: &mut Reader<'_>, components: u32, stride: usize) -> Result<Key> {
    let mut key = Key {
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 0.0,
        tx: 0.0,
        ty: 0.0,
        tz: 0.0,
        sx: 1.0,
        sy: 1.0,
        sz: 1.0,
        time: 0.0,
    };
    if components == COMPONENTS_BLEND_FACTOR {
        // 混合因子：非变换轨道，保留时间
        let _factor = r.f32("AN_factor")?;
        key.time = r.f32("AN_time")?;
    } else {
        key.qx = r.f32("AN_qx")?;
        key.qy = r.f32("AN_qy")?;
        key.qz = r.f32("AN_qz")?;
        key.qw = r.f32("AN_qw")?;
        key.tx = r.f32("AN_tx")?;
        key.ty = r.f32("AN_ty")?;
        key.tz = r.f32("AN_tz")?;
        if components == COMPONENTS_LOC_ROT_SCALE {
            key.sx = r.f32("AN_sx")?;
            key.sy = r.f32("AN_sy")?;
            key.sz = r.f32("AN_sz")?;
            let _pad = r.u32("AN_pad")?;
        }
        key.time = r.f32("AN_time")?;
    }
    // C# `PoseSizeFor(0x101)=36` 而 `ReadKey` 仅消费 32 字节，存在逐 key 漂移；
    // 此处按 stride 补齐到下一关键帧，两种存储（32/36）都保持对齐。
    let consumed = if components == COMPONENTS_BLEND_FACTOR {
        8
    } else if components == COMPONENTS_LOC_ROT_SCALE {
        48
    } else {
        32
    };
    if stride > consumed {
        r.seek(r.pos() + stride - consumed)?;
    }
    Ok(key)
}
