//! RW4Skeleton（0x7000C）解析：骨骼层级 + bind 矩阵。
//!
//! 对齐 C# `RW4Skeleton.Read`（SK000–SK001）与 `RW4HierarchyInfo.Read`
//!（HI000–HI012）、`Matrices<T>.Read`（MS001/MS002 + 数量防挂起护栏）：
//! ```text
//! Skeleton:  expect 0x400000 | unk1 | ref→Matrices4x3(0x7000F)
//!                                  | ref→HierarchyInfo(0x70002)
//!                                  | ref→Matrices4x4(0x7000B)
//! Hierarchy: p2,p3,p1,c1,id,expect c1 | c1×name_fnv | c1×flags | c1×parent(-1=根)
//! Matrices:  p1,count,0,0（p1==pos）| count×12|16 f32
//! ```
//! bind 矩阵（Matrices4x4）即 glTF inverseBindMatrix 的逐字来源：
//! 列主序 R^T，0 填充的行只差 `[15]=1`（HANDOFF §4）。

use crate::error::{Error, Result};
use crate::model::Rw4File;
use crate::reader::Reader;
use crate::section::SectionType;

/// 单个关节：FNV 名称哈希、标志位（低 位或全零——1 可能是叶子）、父索引（-1=根）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Joint {
    /// 关节名 FNV-1 哈希（如 `"joint1".fnv()`）。
    pub name_fnv: u32,
    /// 标志（观察值 0..=3，&1 可能表示叶子）。
    pub flags: u32,
    /// 父关节索引；`-1` 表示根。
    pub parent: i32,
}

/// 骨骼层级（关节顺序即索引顺序）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hierarchy {
    /// 哈希/guid，Anim 以此选择要驱动的骨骼组。
    pub id: u32,
    pub joints: Vec<Joint>,
}

/// 已解码骨骼：层级 + bind 矩阵组。
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSkeleton {
    /// 头部 `unk1`（观察值 0x8d6da0 等，非通用常量）。
    pub unknown: u32,
    pub hierarchy: Hierarchy,
    /// Matrices4x4：每关节一列的 bind/逆绑定矩阵（列主序 16 f32）。
    pub bind_matrices: Vec<[f32; 16]>,
    /// Matrices4x3：12 f32/关节。
    pub matrices_4x3: Vec<[f32; 12]>,
}

impl Rw4File {
    /// 解码一个骨骼 section 及其引用的层级/矩阵 sections。
    pub fn decode_skeleton(&self, data: &[u8], number: u32) -> Result<DecodedSkeleton> {
        let section = self.section_for(number, "SK000", SectionType::RW4_SKELETON)?;
        let payload = self.payload(data, section.number)?;

        let mut r = Reader::new(payload);
        r.expect_u32(0x40_0000, "SK001")?;
        let unknown = r.u32("SK_unk1")?;
        let mat3_ref = section_ref(&mut r)?;
        let hierarchy_ref = section_ref(&mut r)?;
        let mat4_ref = section_ref(&mut r)?;

        self.section_for(mat3_ref, "SK100", SectionType::MATRICES_4X3)?;
        self.section_for(hierarchy_ref, "SK101", SectionType::HIERARCHY_INFO)?;
        // Matrices4x4 有两个真实存在的类型码：SectionTypeCodes 枚举 0x7000B
        // 与 C# 类常量 `Matrices4x4.type_code = 0x70003`（真实骨骼资源使用后者）
        let mat4_section = self
            .section(mat4_ref)
            .ok_or(Error::SectionNumberOutOfRange {
                number: mat4_ref,
                count: self.sections().len() as u32,
            })?;
        if mat4_section.type_code != SectionType::MATRICES_4X4 && mat4_section.type_code != 0x70_003
        {
            return Err(Error::BadSectionType {
                check: "SK102",
                number: mat4_ref,
                expected: SectionType::MATRICES_4X4,
                actual: mat4_section.type_code,
            });
        }

        let hierarchy = parse_hierarchy(
            self.payload(data, hierarchy_ref)?,
            base_of(self.section(hierarchy_ref)),
        )?;
        let bind_matrices = parse_matrices::<16>(
            self.payload(data, mat4_ref)?,
            base_of(self.section(mat4_ref)),
        )?;
        let matrices_4x3 = parse_matrices::<12>(
            self.payload(data, mat3_ref)?,
            base_of(self.section(mat3_ref)),
        )?;

        if bind_matrices.len() < hierarchy.joints.len() {
            return Err(Error::InsufficientPayload {
                check: "SK200",
                needed: hierarchy.joints.len(),
                actual: bind_matrices.len(),
            });
        }

        Ok(DecodedSkeleton {
            unknown,
            hierarchy,
            bind_matrices,
            matrices_4x3,
        })
    }
}

/// C# `r.ReadS32()` 的 section 引用；负数映射为 u32::MAX 自然越界。
fn section_ref(r: &mut Reader<'_>) -> Result<u32> {
    Ok(u32::try_from(r.i32("SK_ref")?).unwrap_or(u32::MAX))
}

fn base_of(section: Option<&crate::section::Section>) -> u64 {
    section.map(|s| s.pos as u64).unwrap_or(0)
}

pub(crate) fn parse_hierarchy(payload: &[u8], base: u64) -> Result<Hierarchy> {
    let mut r = Reader::new(payload);
    let p2 = r.u32("HI_p2")?;
    let p3 = r.u32("HI_p3")?;
    let p1 = r.u32("HI_p1")?;
    let count = r.u32("HI_count")?;
    let id = r.u32("HI_id")?;
    r.expect_u32(count, "HI001")?;
    // 头部指针为绝对文件偏移（C# 以整文件流 seek），转为 payload 内相对值
    let rel = |p: u32| -> Result<usize> {
        p.checked_sub(base as u32)
            .map(|v| v as usize)
            .ok_or(Error::UnexpectedValue {
                check: "HI-base",
                expected: base,
                actual: p as u64,
            })
    };
    let p1_rel = rel(p1)?;
    if p1_rel != r.pos() {
        return Err(Error::UnexpectedValue {
            check: "HI010",
            expected: p1 as u64,
            actual: (base as usize + r.pos()) as u64,
        });
    }
    if p2 != p1 + count * 4 {
        return Err(Error::UnexpectedValue {
            check: "HI011",
            expected: (p1 + count * 4) as u64,
            actual: p2 as u64,
        });
    }
    if p3 != p1 + count * 8 {
        return Err(Error::UnexpectedValue {
            check: "HI012",
            expected: (p1 + count * 8) as u64,
            actual: p3 as u64,
        });
    }

    let mut names = Vec::with_capacity(count as usize);
    r.seek(p1_rel)?;
    for _ in 0..count {
        names.push(r.u32("HI_name")?);
    }
    let mut flags = Vec::with_capacity(count as usize);
    r.seek(rel(p2)?)?;
    for _ in 0..count {
        flags.push(r.u32("HI_flags")?);
    }
    let mut joints = Vec::with_capacity(count as usize);
    r.seek(rel(p3)?)?;
    for i in 0..count {
        let parent = r.i32("HI_parent")?;
        if parent >= count as i32 {
            return Err(Error::UnexpectedValue {
                check: "HI020",
                expected: (count - 1) as u64,
                actual: parent as u64,
            });
        }
        joints.push(Joint {
            name_fnv: names[i as usize],
            flags: flags[i as usize],
            parent,
        });
    }
    Ok(Hierarchy { id, joints })
}

/// Matrices4x4（16 f32/条）或 Matrices4x3（12 f32/条），`N` 为条目字长/4。
pub(crate) fn parse_matrices<const N: usize>(payload: &[u8], base: u64) -> Result<Vec<[f32; N]>> {
    let mut r = Reader::new(payload);
    let p1 = r.u32("MS_p1")?;
    let count = r.u32("MS_count")?;
    r.expect_u32(0, "MS001")?;
    r.expect_u32(0, "MS002")?;
    // p1 为绝对文件偏移（C# `p1 != r.Position`）；兼容相对存储
    let p1_rel = if (p1 as u64) >= base {
        p1 as usize - base as usize
    } else {
        p1 as usize
    };
    if p1_rel != r.pos() {
        return Err(Error::UnexpectedValue {
            check: "MS000",
            expected: p1 as u64,
            actual: (base as usize + r.pos()) as u64,
        });
    }
    // C# 数量护栏（MS-count）：count > size/Tsize + 1 视为垃圾数据
    if count as usize > payload.len() / (N * 4) + 1 {
        return Err(Error::InsufficientPayload {
            check: "MS-count",
            needed: count as usize * N * 4,
            actual: payload.len(),
        });
    }
    r.seek(p1_rel)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut m = [0f32; N];
        for v in &mut m {
            *v = r.f32("MS_f32")?;
        }
        out.push(m);
    }
    Ok(out)
}
