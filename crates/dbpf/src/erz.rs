//! ERZ — EcoGame 编译规则库容器（资源类型 `0x0806_8AEB`，注册名
//! "ER2 Binary Rule File"）。
//!
//! 集中存放 GlassBox「单位规则」编译产物。文本源码为 `0x0806_8AEC`
//! （"ER2 Rule File"，UTF-8 `unitRule/globalRule/...` 语法）。
//!
//! 布局自 SimCity.exe 序列化器（`FUN_005fc7a0` 及各段子函数）逆向，
//! 全部整数为 **大端**（与 `.egb` 存档状态同一序列化家族）：
//!
//! ```text
//! u32 5          布局主版本（样本实证 5；旧补丁包为 4，布局不同）
//! u32 2          布局次版本
//! u32 14         规则记录布局版本（旧包 13，字段集不同）
//! u8   flag      布尔（样本全 0）
//! u32 ruleCount  规则数（SimCity 主规则集 28441）
//! ruleCount × RuleRecord
//! u32 5, u32 nA, nA × RecA(18×u32)
//! u32 8, u32 nB, nB × RecB
//! u32 2, u32 nC, nC × RecC
//! u32 pairCount, pairCount × (u32 hash, u32 value)   —— `set` 常量表
//! u32 outerD, outerD × (u32 innerCount, innerCount × 20B 条目)
//! u32 outerE, outerE × (u32 innerCount, innerCount × 20B 条目)
//! u32 blobLen, blobLen 字节                          —— 字符串/池
//! ```
//!
//! 校验基准：三个真实规则集（SimCity 6.0MB / HeroesAndVillains 67KB /
//! Sandbox 11KB，patch 287520926）按此布局**精确消耗到文件尾**。

/// ER2 Binary Rule File（编译规则库）。
pub const ERZ_BINARY_RULE_TYPE: u32 = 0x0806_8AEB;
/// ER2 Rule File（UTF-8 文本规则源码）。
pub const ERZ_TEXT_RULE_TYPE: u32 = 0x0806_8AEC;

/// 本模块实证支持的布局版本（rule record layout = 14）。
pub const SUPPORTED_LAYOUT: u32 = 14;

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
    pub rec_a_count: u32,
    pub rec_b_count: u32,
    pub rec_c_count: u32,
    /// `set` 常量表条目数。
    pub constant_count: usize,
    /// `set` 常量表样本（≤256 条）。
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
/// 受支持的布局（record == `SUPPORTED_LAYOUT`）完整走一遍流（含全部规则
/// 记录的形状校验），任何布局错位都会在这里报错而不是产出错误摘要。
/// 旧补丁布局（record != 14，游戏已不读取的残留包）降级为仅解析文件头，
/// `layout_supported=false`、`exact=false`。
pub fn parse_summary(data: &[u8]) -> Result<ErzSummary, ErzError> {
    let mut r = Reader::new(data);
    let major = r.u32()?;
    let minor = r.u32()?;
    let layout = r.u32()?;
    let mut s = ErzSummary {
        major,
        minor,
        layout,
        layout_supported: layout == SUPPORTED_LAYOUT,
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
    for i in 0..rule_count {
        let name_hash = r.u32()?;
        r.skip(4)?; // 伴随哈希（模块/作用域归属，推断）
        r.skip(4)?; // f2
        for _ in 0..5 {
            r.skip(8)?; // (p, q) 条件/动作对
        }
        r.skip(16)?; // g13
        r.skip(10 * 4)?; // h17..h26
        r.skip(12)?; // v3
        r.skip(12)?; // h30, f31, f32
        let sub_count = r.u32()?;
        // 子条目：hash + 2×u32
        r.skip(sub_count as usize * 12)?;
        r.skip(8)?; // h37, h38
        if i < 16 {
            s.rule_name_hashes.push(name_hash);
        }
    }
    parse_sections(&mut s, &mut r)?;
    Ok(s)
}

fn parse_sections(s: &mut ErzSummary, r: &mut Reader) -> Result<(), ErzError> {
        // 段 A：tag=5，18×u32 记录
        let _tag = r.u32()?;
        let n_a = r.u32()?;
        r.skip(n_a as usize * 18 * 4)?;
        s.rec_a_count = n_a;
        // 段 B：tag=8，记录 = 10×u32 + 4×u64 + 8×u32 = 104B（ser_b 逆变换）
        let _tag = r.u32()?;
        let n_b = r.u32()?;
        r.skip(n_b as usize * (10 * 4 + 4 * 8 + 8 * 4))?;
        s.rec_b_count = n_b;
        // 段 C：tag=2，变长记录（3×u32 + u8 + 2×u32 + 3 个子数组）
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
        // set 常量表：(hash, value) 对
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

    /// layout 13（旧补丁包）应降级返回头部摘要而不是错误对齐。
    #[test]
    fn old_layout_degrades() {
        let mut d = Vec::new();
        d.extend_from_slice(&4u32.to_be_bytes());
        d.extend_from_slice(&2u32.to_be_bytes());
        d.extend_from_slice(&13u32.to_be_bytes());
        d.push(0);
        d.extend_from_slice(&21u32.to_be_bytes());
        let s = parse_summary(&d).expect("parse");
        assert_eq!((s.major, s.minor, s.layout), (4, 2, 13));
        assert!(!s.layout_supported);
        assert!(!s.exact);
        assert_eq!(s.rule_count, 21);
        assert!(s.constants.is_empty());
    }
}
