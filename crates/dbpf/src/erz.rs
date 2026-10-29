//! ERZ — EcoGame 编译规则库容器（资源类型 `0x0806_8AEB`，注册名
//! "ER2 Binary Rule File"）。
//!
//! 集中存放 GlassBox「单位规则」编译产物。文本源码为 `0x0806_8AEC`
//! （"ER2 Rule File"，UTF-8 `unitRule/globalRule/...` 语法）。
//!
//! 布局与语义来源（2026-10-29/30，dev-beta PDB 伪码 + 数据精确消耗互证，
//! 见 `docs/re/pdb-sample-four-questions-20261029.md`）：
//!
//! * 头部 `u32×3 + u8 namesIncluded + u32 ruleCount`（dev
//!   `GB::cEcoRuleSet::Read`；三个 u32 是 rules/base/unit 三条独立版本线：
//!   dev 读 (4,2,13)，retail 新包 (5,2,14)）。
//! * unit rule 记录：layout 14 = **144 + 12×T 字节**（主库 28,441 条精确
//!   消耗证明）；layout 13 = **160 + 12×T 字节**（dev 伪码全字段展开恰
//!   40 u32，332 条间隔主峰实证）。T = transport 数。
//! * **规则名哈希 = FNV-1(lowercase(name))**（seed 0x811C9DC5、prime
//!   0x01000193，先乘后异或）——与资源系统 `HashName32` 同算法。实证：
//!   H&V Rosetta 对齐后同期 AEB 324/332 直接命中（余 8 为补丁间改名）。
//!   运行时从不对名字重新哈希（`cEcoRuleBase::Read` 读名字后丢弃），
//!   官方包 `namesIncluded` 全 0。
//! * 字段语义（layout 14 的 u32 下标，经验投票 + dev 名字桥接）：
//!   0=id、1=flags 位域、2=sim 池偏移、3..12=五组 (池偏移,数量)
//!   （local/nearby/map/global/fromBinOps 记号流）、15/16=min/maxApply、
//!   19=onSuccess 目标 id、23=onFail/chain 目标 id、33=T、34/35=tid/hid。
//!   layout 13 相应下标：21=onSuccess、26=onFail、17/18=min/maxApply。
//! * 段链（layout 14）：mapRules（tag=mapVer 5，定长 72B/条）、
//!   binsetRules（tag=8，定长 104B/条）、zoneRules（tag=2，变长）、
//!   常量表、D/E 段、字符串池。`set` 常量为编译期内联，AEC 数值不出现
//!   在 AEB；遥测/常量字符串本身在池内，规则经池偏移引用。
//!
//! 校验基准：SimCity 主库（5,947,232B，28,441 规则）与 HeroesAndVillains
//! 两代补丁包按上述布局精确消耗。

/// ER2 Binary Rule File（编译规则库）。
pub const ERZ_BINARY_RULE_TYPE: u32 = 0x0806_8AEB;
/// ER2 Rule File（UTF-8 文本规则源码）。
pub const ERZ_TEXT_RULE_TYPE: u32 = 0x0806_8AEC;

/// 本模块完整支持的布局版本（rule record layout = 14）。
pub const SUPPORTED_LAYOUT: u32 = 14;
/// 旧补丁布局（dev 构建读取端 (4,2,13)；记录 = 160+12T）。
pub const LEGACY_LAYOUT: u32 = 13;

/// GlassBox 规则名哈希：FNV-1(lowercase(name))。
///
/// 与 `EA::StdC::FNV1_String8(name, 0x811C9DC5, kCharCaseLower)` 及资源
/// 系统 `HashName32` 同一算法；对 AEB `mID`、AEC 资源实例 TGI（去扩展名
/// 文件名）均适用。
pub fn fnv1_name_hash(name: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in name.to_lowercase().as_bytes() {
        h = h.wrapping_mul(0x0100_0193) ^ u32::from(*b);
    }
    h
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErzError {
    #[error("data too short: need {needed} bytes at offset {at}, have {have}")]
    TooShort { needed: usize, at: usize, have: usize },
}

struct Reader<'a> {
    d: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    fn new(d: &'a [u8]) -> Self {
        Reader { d, o: 0 }
    }
    fn need(&self, n: usize) -> Result<(), ErzError> {
        if self.o + n > self.d.len() {
            Err(ErzError::TooShort {
                needed: self.o + n,
                at: self.o,
                have: self.d.len(),
            })
        } else {
            Ok(())
        }
    }
    fn u32(&mut self) -> Result<u32, ErzError> {
        self.need(4)?;
        let v = u32::from_be_bytes([self.d[self.o], self.d[self.o + 1], self.d[self.o + 2], self.d[self.o + 3]]);
        self.o += 4;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8, ErzError> {
        self.need(1)?;
        let v = self.d[self.o];
        self.o += 1;
        Ok(v)
    }
    fn skip(&mut self, n: usize) -> Result<(), ErzError> {
        self.need(n)?;
        self.o += n;
        Ok(())
    }
    fn remaining(&self) -> usize {
        self.d.len() - self.o
    }
}

/// 一条 unit rule 记录（字段下标语义随 layout 不同，见模块注释）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ErzRuleRecord {
    /// 规则名哈希 = `fnv1_name_hash(规则名)`。
    pub id: u32,
    /// 位域（AEC `flags -if …`；具体位义未定名）。
    pub flags: u32,
    /// 条件评估输入集的池偏移（0/`0xFFFF_FFFF` 表空）。
    pub sim_offset: u32,
    /// 五组 (池偏移, 数量)：local/nearby/map/global/fromBinOps 记号流。
    pub scopes: [(u32, u32); 5],
    /// `applyCount`（AEC `applyCount N`）。
    pub min_apply: u32,
    /// `maxApplyCount`（`0xFFFF_FFFF` 表 `maxApplyCount` 无值形式）。
    pub max_apply: u32,
    /// onSuccess 目标规则 id（0/`0xFFFF_FFFF` 表无）。
    pub on_success: u32,
    /// onFail/chain 目标规则 id（两者共用同一“负向路由”槽）。
    pub fail_chain: u32,
    /// transport 数组长度。
    pub transport_count: u32,
    /// transport 条目 (op, x, z)。仅在样本内填充。
    pub transports: Vec<(u32, f32, f32)>,
    pub transport_id: u32,
    pub handler_id: u32,
    /// 定长字段原样行（layout 13 = 40 项、layout 14 = 36 项，含尾部
    /// transport_count/tid/hid），供未定名字段研究。
    pub raw: Vec<u32>,
}

/// 解析产出的结构摘要（预览/统计用；不展开全部规则字段）。
#[derive(Debug, Clone, Default)]
pub struct ErzSummary {
    pub major: u32,
    pub minor: u32,
    pub layout: u32,
    pub flag: u8,
    pub rule_count: u32,
    /// 规则名哈希样本（≤16 条）。
    pub rule_name_hashes: Vec<u32>,
    /// 规则记录样本（≤24 条；字段语义见模块注释）。
    pub rule_samples: Vec<ErzRuleRecord>,
    pub rec_a_count: u32,
    pub rec_b_count: u32,
    pub rec_c_count: u32,
    /// 常量表条目数（语义未定名：key 严格升序、非规则名 FNV-1）。
    pub constant_count: usize,
    /// 常量表样本（≤256 条）。
    pub constants: Vec<(u32, u32)>,
    /// D 段对象数与条目总数。
    pub d_objects: u32,
    pub d_entries: u32,
    /// E 段对象数与条目总数。
    pub e_objects: u32,
    pub e_entries: u32,
    /// 尾部池长度（字符串所在）。
    pub blob_length: usize,
    /// 池内可打印字符串样本（≥6 字符，≤200 条）。
    pub strings: Vec<String>,
    /// 是否精确消耗到文件尾。
    pub exact: bool,
    /// 规则记录布局是否受支持（false = 旧补丁布局，仅解析文件头）。
    pub layout_supported: bool,
}

/// 解析 ERZ 二进制规则库，返回结构摘要。
///
/// 布局 14 与 13 均做 unit-rule 记录级精确走流（记录形状错位即报错）；
/// 段链仅对 14 完整解析（13 的 binset/zone 变长字段集未定名，标记
/// `exact=false`）。其它布局降级为仅解析文件头：零售游戏仍可读取部分
/// 旧布局，不能据解析器能力判断包是否被游戏加载。
pub fn parse_summary(data: &[u8]) -> Result<ErzSummary, ErzError> {
    let mut r = Reader::new(data);
    let major = r.u32()?;
    let minor = r.u32()?;
    let layout = r.u32()?;
    let mut s = ErzSummary {
        major,
        minor,
        layout,
        layout_supported: matches!(layout, SUPPORTED_LAYOUT | LEGACY_LAYOUT),
        ..Default::default()
    };
    s.flag = r.u8()?;
    let rule_count = r.u32()?;
    s.rule_count = rule_count;
    if !s.layout_supported {
        // 头部形状（3×u32 + bool + count）跨版本一致；其余布局未知，
        // 不冒险解读，规则名哈希样本也不可用。
        s.exact = false;
        return Ok(s);
    }
    let sample_cap = 24;
    for i in 0..rule_count {
        let rec = if layout == LEGACY_LAYOUT {
            parse_rule_record(&mut r, &LEGACY_LAYOUT_SPEC)?
        } else {
            parse_rule_record(&mut r, &RETAIL_LAYOUT_SPEC)?
        };
        if (i as usize) < 16 {
            s.rule_name_hashes.push(rec.id);
        }
        if (i as usize) < sample_cap {
            s.rule_samples.push(rec);
        }
    }
    if layout == SUPPORTED_LAYOUT {
        parse_sections(&mut s, &mut r)?;
    } else {
        s.exact = false;
    }
    Ok(s)
}

/// 各布局的记录形状与关键字段下标（见模块注释）。
struct LayoutSpec {
    /// 定长字段数（含 T/tid/hid；不含 12B×T 条目）。
    fields: usize,
    sid: usize,
    fid: usize,
    min: usize,
    max: usize,
    t: usize,
}

const RETAIL_LAYOUT_SPEC: LayoutSpec =
    LayoutSpec { fields: 36, sid: 19, fid: 23, min: 15, max: 16, t: 33 };
const LEGACY_LAYOUT_SPEC: LayoutSpec =
    LayoutSpec { fields: 40, sid: 21, fid: 26, min: 17, max: 18, t: 37 };

fn parse_rule_record(r: &mut Reader, spec: &LayoutSpec) -> Result<ErzRuleRecord, ErzError> {
    r.need(spec.fields * 4)?;
    let mut raw = Vec::with_capacity(spec.fields);
    for _ in 0..spec.fields {
        raw.push(r.u32()?);
    }
    let t = raw[spec.t];
    r.need(t as usize * 12)?;
    let mut transports = Vec::with_capacity(t as usize);
    for _ in 0..t {
        let op = r.u32()?;
        let x = f32::from_bits(r.u32()?);
        let z = f32::from_bits(r.u32()?);
        transports.push((op, x, z));
    }
    let mut scopes = [(0u32, 0u32); 5];
    for (i, sc) in scopes.iter_mut().enumerate() {
        *sc = (raw[3 + i * 2], raw[4 + i * 2]);
    }
    Ok(ErzRuleRecord {
        id: raw[0],
        flags: raw[1],
        sim_offset: raw[2],
        scopes,
        min_apply: raw[spec.min],
        max_apply: raw[spec.max],
        on_success: raw[spec.sid],
        fail_chain: raw[spec.fid],
        transport_count: t,
        transports,
        transport_id: raw[spec.fields - 2],
        handler_id: raw[spec.fields - 1],
        raw,
    })
}

fn parse_sections(s: &mut ErzSummary, r: &mut Reader) -> Result<(), ErzError> {
        // 段 A：mapRules，tag=mapRulesVersion(5)，定长 72B/条（主库实证）
        let _tag = r.u32()?;
        let n_a = r.u32()?;
        r.skip(n_a as usize * 18 * 4)?;
        s.rec_a_count = n_a;
        // 段 B：binsetRules/globalRules，tag=8，定长 104B/条（主库实证）
        let _tag = r.u32()?;
        let n_b = r.u32()?;
        r.skip(n_b as usize * (10 * 4 + 4 * 8 + 8 * 4))?;
        s.rec_b_count = n_b;
        // 段 C：zoneRules，tag=2，变长记录
        let _tag = r.u32()?;
        let n_c = r.u32()?;
        for _ in 0..n_c {
            r.skip(3 * 4 + 1 + 2 * 4)?;
            let n1 = r.u32()?;
            r.skip(n1 as usize * 12)?;
            let n2 = r.u32()?;
            r.skip(n2 as usize * 12)?;
            let n3 = r.u32()?;
            r.skip(n3 as usize * 8)?;
        }
        s.rec_c_count = n_c;
        // 常量表：语义未定名（key 严格升序、非规则名 FNV-1）。
        let pair_count = r.u32()?;
        s.constant_count = pair_count as usize;
        for i in 0..pair_count {
            let h = r.u32()?;
            let v = r.u32()?;
            if (i as usize) < 256 {
                s.constants.push((h, v));
            }
        }
        // D/E 段：外层对象数 × (内层条目数 × 20B)
        let d_outer = r.u32()?;
        s.d_objects = d_outer;
        let mut d_entries = 0u32;
        for _ in 0..d_outer {
            let inner = r.u32()?;
            d_entries = d_entries.saturating_add(inner);
            r.skip(inner as usize * 20)?;
        }
        s.d_entries = d_entries;
        let e_outer = r.u32()?;
        s.e_objects = e_outer;
        let mut e_entries = 0u32;
        for _ in 0..e_outer {
            let inner = r.u32()?;
            e_entries = e_entries.saturating_add(inner);
            r.skip(inner as usize * 20)?;
        }
        s.e_entries = e_entries;
        // 尾部池
        let blob_len = r.u32()? as usize;
        r.need(blob_len)?;
        s.blob_length = blob_len;
        let blob = &r.d[r.o..r.o + blob_len];
        r.o += blob_len;
        s.strings = extract_strings(blob, 200);
        s.exact = r.remaining() == 0;
        Ok(())
}


/// 从池字节中提取可打印 ASCII 字符串（作为符号名候选）。
fn extract_strings(blob: &[u8], cap: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, &b) in blob.iter().enumerate() {
        let printable = (0x20..0x7f).contains(&b);
        if printable && start.is_none() {
            start = Some(i);
        }
        if (!printable || i + 1 == blob.len()) && let Some(s) = start {
            let end = if printable { i + 1 } else { i };
            if end - s >= 6 {
                out.push(String::from_utf8_lossy(&blob[s..end]).into_owned());
                if out.len() >= cap {
                    return out;
                }
            }
            start = None;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手工构造一个最小 ERZ（layout 14）：0 规则、空段、1 条常量、
    /// 1 个 D 对象（1 条目）、空 E、池内含一个符号串。
    #[test]
    fn minimal_fixture_parses_exactly() {
        let mut d = Vec::new();
        let w = |v: u32, d: &mut Vec<u8>| d.extend_from_slice(&v.to_be_bytes());
        w(5, &mut d);
        w(2, &mut d);
        w(14, &mut d);
        d.push(0); // flag
        w(0, &mut d); // ruleCount
        w(5, &mut d);
        w(0, &mut d); // A
        w(8, &mut d);
        w(0, &mut d); // B
        w(2, &mut d);
        w(0, &mut d); // C
        w(1, &mut d); // 常量对
        w(0xDEAD_BEEF, &mut d);
        w(100000, &mut d);
        w(1, &mut d); // D 对象
        w(1, &mut d); // 1 条目
        d.extend_from_slice(&0x1234_5678u32.to_be_bytes()); // hash
        d.extend_from_slice(&0u32.to_be_bytes());
        d.extend_from_slice(&0u32.to_be_bytes());
        d.extend_from_slice(&0x8765_4321u32.to_be_bytes());
        d.extend_from_slice(&0x0102u16.to_be_bytes());
        d.push(3);
        d.push(4);
        w(0, &mut d); // E 对象
        let str_pool = b"SC_RULE_TEST_SYMBOL\x00";
        w(str_pool.len() as u32, &mut d);
        d.extend_from_slice(str_pool);

        let s = parse_summary(&d).expect("parse");
        assert_eq!((s.major, s.minor, s.layout), (5, 2, 14));
        assert_eq!(s.rule_count, 0);
        assert_eq!(s.constant_count, 1);
        assert_eq!(s.constants, vec![(0xDEAD_BEEF, 100000)]);
        assert_eq!(s.d_objects, 1);
        assert_eq!(s.d_entries, 1);
        assert_eq!(s.e_objects, 0);
        assert_eq!(s.strings, vec!["SC_RULE_TEST_SYMBOL"]);
        assert!(s.exact);
    }

    /// layout 13（旧补丁包）：160+12T 记录级解析。
    #[test]
    fn legacy_layout_parses_records() {
        let mut d = Vec::new();
        let w = |v: u32, d: &mut Vec<u8>| d.extend_from_slice(&v.to_be_bytes());
        w(4, &mut d);
        w(2, &mut d);
        w(13, &mut d);
        d.push(0); // namesIncluded
        w(1, &mut d); // ruleCount = 1
        // 一条 160+12T 记录：40 个 u32，T 在 idx37。
        let id = fnv1_name_hash("HeroBase_CanCreate");
        for i in 0..40u32 {
            w(
                match i {
                    0 => id,
                    3 => 0xFFFF_FFFF, // local off 空
                    17 => 3,          // minApply
                    18 => 7,          // maxApply
                    21 => id,         // onSuccess 占位
                    37 => 2,          // T = 2
                    38 => 0x00AA_00BB,
                    39 => 0x00CC_00DD,
                    _ => 0,
                },
                &mut d,
            );
        }
        // 2 × transport (op, x, z)
        for (op, x, z) in [(9u32, 1.5f32, 2.5f32), (10, 3.5, 4.5)] {
            w(op, &mut d);
            d.extend_from_slice(&x.to_be_bytes());
            d.extend_from_slice(&z.to_be_bytes());
        }
        let s = parse_summary(&d).expect("parse");
        assert_eq!((s.major, s.minor, s.layout), (4, 2, 13));
        assert!(s.layout_supported);
        assert_eq!(s.rule_count, 1);
        assert_eq!(s.rule_samples.len(), 1);
        let rec = &s.rule_samples[0];
        assert_eq!(rec.id, id);
        assert_eq!(rec.min_apply, 3);
        assert_eq!(rec.max_apply, 7);
        assert_eq!(rec.on_success, id);
        assert_eq!(rec.transport_count, 2);
        assert_eq!(rec.transports.len(), 2);
        assert_eq!(rec.transport_id, 0x00AA_00BB);
        assert_eq!(rec.handler_id, 0x00CC_00DD);
        assert_eq!(rec.raw.len(), 40);
    }

    /// layout 14 记录形状：144+12T，关键字段按零售下标。
    #[test]
    fn retail_layout_record_fields() {
        let mut d = Vec::new();
        let w = |v: u32, d: &mut Vec<u8>| d.extend_from_slice(&v.to_be_bytes());
        w(5, &mut d);
        w(2, &mut d);
        w(14, &mut d);
        d.push(0);
        w(1, &mut d);
        for i in 0..36u32 {
            w(
                match i {
                    15 => 1,           // minApply
                    16 => 1,           // maxApply
                    19 => 0x1234_5678, // onSuccess
                    23 => 0x9ABC_DEF0, // onFail/chain
                    33 => 1,           // T
                    _ => 0,
                },
                &mut d,
            );
        }
        w(11, &mut d);
        d.extend_from_slice(&1.0f32.to_be_bytes());
        d.extend_from_slice(&2.0f32.to_be_bytes());
        // 段链 + 尾池（最小形状）
        w(5, &mut d);
        w(0, &mut d);
        w(8, &mut d);
        w(0, &mut d);
        w(2, &mut d);
        w(0, &mut d);
        w(0, &mut d); // 常量
        w(0, &mut d); // D
        w(0, &mut d); // E
        let pool = b"x";
        w(pool.len() as u32, &mut d);
        d.extend_from_slice(pool);
        let s = parse_summary(&d).expect("parse");
        assert!(s.exact, "layout 14 fixture should consume exactly");
        let rec = &s.rule_samples[0];
        assert_eq!(rec.min_apply, 1);
        assert_eq!(rec.on_success, 0x1234_5678);
        assert_eq!(rec.fail_chain, 0x9ABC_DEF0);
        assert_eq!(rec.transport_count, 1);
        assert_eq!(rec.transports[0], (11, 1.0, 2.0));
    }

    /// 破解的规则名哈希：FNV-1(lowercase)，真实 AEB 锚点（Rosetta 对齐
    /// 后的直接命中对）。
    #[test]
    fn fnv1_name_hash_matches_aeb_ids() {
        assert_eq!(fnv1_name_hash("MSHeroesVilliansNoRegrets4"), 0x168F_72B2);
        assert_eq!(
            fnv1_name_hash("HeroGarage_MaxisManReturn_CheckReticulatorUnlock"),
            0x635D_7F92
        );
    }
}
