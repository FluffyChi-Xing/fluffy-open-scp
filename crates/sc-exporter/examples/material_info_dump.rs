//! MaterialInfo（材质信息表）解析——exe `SP::cMaterialManager::ReadMaterials` 的逆向对拍。
//!
//! 资源：type 0x0469A3F7 / group 0x40212000（kGroupMaterialInfo）/ instance = shaderPath(0..3)。
//! CompiledStates arena：type 0x2F4E681B / group 0x40212001 / 同 instance。
//!
//! MaterialInfo 格式（全大端，exe 行 453644-453875 证据）：
//!   version u32 (=1)
//!   loop:
//!     materialID u32（0xFFFFFFFF 终止）
//!     numTextures u16
//!     ×numTextures: instance u32, group u32, samplerA u16, samplerB u16
//!     32 bytes（16×u8 pair，语义待定）
//!     16 × u8 hasCompiledState 标志（逐 RenderType；置位则消耗一个 arena 导出对象序号）
//!
//! 用法：cargo run -p sc-exporter --release --example material_info_dump -- <pkg> [...]

use dbpf::Package;
use std::collections::BTreeMap;

const MATERIAL_INFO_TYPE: u32 = 0x0469_A3F7;
const MATERIAL_INFO_GROUP: u32 = 0x4021_2000;
const COMPILED_STATES_TYPE: u32 = 0x2F4E_681B;
const COMPILED_STATES_GROUP: u32 = 0x4021_2001;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn u32(&mut self) -> Option<u32> {
        let b = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_be_bytes(b.try_into().unwrap()))
    }
    fn u16(&mut self) -> Option<u16> {
        let b = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_be_bytes(b.try_into().unwrap()))
    }
    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(())
    }
}

#[derive(Debug)]
struct MaterialRecord {
    id: u32,
    textures: Vec<(u32, u32, u16, u16)>,
    unknown32: [u8; 32],
    compiled_state_flags: [u8; 16],
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut found_any = false;
    // instance -> (MaterialInfo 数据, CompiledStates 数据)
    let mut infos: BTreeMap<u32, Vec<u8>> = Default::default();
    let mut arenas: BTreeMap<u32, Vec<u8>> = Default::default();
    for path in &paths {
        let pkg = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        let name = path.rsplit(['/', '\\']).next().unwrap().to_string();

        for e in pkg.entries() {
            if e.id.type_id == MATERIAL_INFO_TYPE && e.id.group == MATERIAL_INFO_GROUP {
                found_any = true;
                let data = pkg.read(e).expect("read material info");
                println!(
                    "=== [{name}] MaterialInfo instance={} size={} ===",
                    e.id.instance,
                    data.len()
                );
                parse_material_info(&data);
                infos.entry(e.id.instance).or_insert(data);
            }
        }
        for e in pkg.entries() {
            if e.id.type_id == COMPILED_STATES_TYPE && e.id.group == COMPILED_STATES_GROUP {
                found_any = true;
                let data = pkg.read(e).expect("read compiled states");
                println!(
                    "=== [{name}] CompiledStates instance={} size={} ===",
                    e.id.instance,
                    data.len()
                );
                dump_arena_overview(&data);
                arenas.entry(e.id.instance).or_insert(data);
            }
        }
    }
    // 联动对拍：shader path 0（与渲染脚本 dump_40212015_2 同路径）
    for (inst, info) in &infos {
        if let Some(arena) = arenas.get(inst) {
            cross_reference(info, arena, &format!("path{inst}"));
        }
    }
    if !found_any {
        println!("未找到 MaterialInfo / CompiledStates 资源");
    }
}

fn parse_material_info(data: &[u8]) {
    let mut r = Reader::new(data);
    let version = r.u32().expect("version");
    println!("version={version}");
    if version != 1 {
        println!("!! version != 1，中止");
        return;
    }
    let mut records: Vec<MaterialRecord> = Vec::new();
    loop {
        let id = match r.u32() {
            Some(v) => v,
            None => break,
        };
        if id == 0xFFFF_FFFF {
            break;
        }
        let num_textures = r.u16().expect("numTextures") as usize;
        let mut textures = Vec::with_capacity(num_textures);
        for _ in 0..num_textures {
            let instance = r.u32().expect("tex instance");
            let group = r.u32().expect("tex group");
            let sa = r.u16().expect("samplerA");
            let sb = r.u16().expect("samplerB");
            textures.push((instance, group, sa, sb));
        }
        let mut unknown32 = [0u8; 32];
        for b in unknown32.iter_mut() {
            *b = r.u8().expect("unknown32");
        }
        let mut flags = [0u8; 16];
        for f in flags.iter_mut() {
            *f = r.u8().expect("flags");
        }
        records.push(MaterialRecord {
            id,
            textures,
            unknown32,
            compiled_state_flags: flags,
        });
    }
    println!(
        "解析 {} 条材质记录，终止于 offset {}/{}",
        records.len(),
        r.pos,
        data.len()
    );

    // 重点关注三个 decal 族材质
    const DECAL_MATERIALS: [(u32, &str); 3] = [
        (0x4491DE3A, "破洞/废墟"),
        (0x73684EFC, "招牌"),
        (0xE5390A98, "涂鸦"),
    ];
    for (id, label) in DECAL_MATERIALS {
        match records.iter().find(|m| m.id == id) {
            Some(m) => {
                let slots: Vec<String> = m
                    .compiled_state_flags
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| **f != 0)
                    .map(|(i, _)| format!("rt{i}"))
                    .collect();
                println!(
                    "[decal族] {id:08X} ({label}): 纹理{} 个, compiled-state 槽: [{}], unk32={:02x?}",
                    m.textures.len(),
                    slots.join(","),
                    &m.unknown32[..8]
                );
                for t in &m.textures {
                    println!(
                        "    tex inst={:08X} grp={:08X} sampler=({},{})",
                        t.0, t.1, t.2, t.3
                    );
                }
            }
            None => println!("[decal族] {id:08X} ({label}): 不在本表"),
        }
    }

    // 汇总：全部材质的 compiled-state 槽位分布
    let mut slot_hist: BTreeMap<usize, usize> = Default::default();
    for m in &records {
        for (i, f) in m.compiled_state_flags.iter().enumerate() {
            if *f != 0 {
                *slot_hist.entry(i).or_insert(0) += 1;
            }
        }
    }
    print!("compiled-state 槽位直方图:");
    for (slot, count) in &slot_hist {
        print!(" rt{slot}={count}");
    }
    println!();

    // 导出全量表供对照
    let out = "tmp/material_info_records.txt";
    let mut s = String::new();
    for m in &records {
        let slots: Vec<String> = m
            .compiled_state_flags
            .iter()
            .enumerate()
            .filter(|(_, f)| **f != 0)
            .map(|(i, _)| format!("rt{i}"))
            .collect();
        s.push_str(&format!(
            "{:08X} tex={} slots=[{}]\n",
            m.id,
            m.textures.len(),
            slots.join(",")
        ));
    }
    std::fs::create_dir_all("tmp").ok();
    std::fs::write(out, s).ok();
    println!("全量材质表 -> {out}（{} 条）", records.len());
}

/// arena 段索引记录（24 字节）。
#[derive(Debug, Clone, Copy)]
struct ArenaSection {
    pos: u32,
    size: u32,
    type_code: u32,
}

/// 解析 RW4 arena（0xCAFED00D）段索引。
/// 布局（tmp/compiled_states_0.bin 逐字节核对）：
/// 28B magic | 0xCAFED00D | count | count | 16 | 0 | index_begin | 0x98 | 0,0,0
/// | index_end | 16 | 0 | …常量区… | 0x98: 0x10004 + 6×相对偏移 | 0x10005 类型表 …
/// | index_begin: count×24B (pos, 0, size, align, indirect, type_code)
fn parse_arena(data: &[u8]) -> Option<(Vec<ArenaSection>, Vec<u32>)> {
    let le = |off: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            data.get(off..off + 4)?.try_into().unwrap(),
        ))
    };
    if data.get(0x1C..0x20)? != 0xCAFED00Du32.to_le_bytes() {
        return None;
    }
    let count = le(0x20)? as usize;
    let index_begin = le(0x30)? as usize;
    let first_header = le(0x34)? as usize;
    // 0x10004 块：6 个相对 first_header 的偏移；offsets[2] 指向 0x10005 类型表
    if le(first_header)? != 0x10004 {
        return None;
    }
    let type_table_off = first_header + le(first_header + 4 + 8)? as usize;
    if le(type_table_off)? != 0x10005 {
        return None;
    }
    let type_count = le(type_table_off + 4)? as usize;
    let mut types = Vec::with_capacity(type_count);
    for i in 0..type_count {
        types.push(le(type_table_off + 12 + i * 4)?);
    }
    let mut sections = Vec::with_capacity(count);
    for i in 0..count {
        let base = index_begin + i * 24;
        sections.push(ArenaSection {
            pos: le(base)?,
            size: le(base + 8)?,
            type_code: le(base + 20)?,
        });
    }
    Some((sections, types))
}

/// 解析一个 MaterialInfo，返回材质记录与「flag 消耗的 arena 导出序号」映射。
/// 导出序号按记录顺序 × rt 槽位顺序累计（exe: compiledStateIndex 单调递增）。
fn parse_material_info_records(data: &[u8]) -> Vec<(MaterialRecord, Vec<(usize, usize)>)> {
    let mut r = Reader::new(data);
    if r.u32() != Some(1) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut export_index = 0usize;
    loop {
        let id = match r.u32() {
            Some(v) => v,
            None => break,
        };
        if id == 0xFFFF_FFFF {
            break;
        }
        let num_textures = r.u16().expect("numTextures") as usize;
        let mut textures = Vec::with_capacity(num_textures);
        for _ in 0..num_textures {
            textures.push((
                r.u32().unwrap(),
                r.u32().unwrap(),
                r.u16().unwrap(),
                r.u16().unwrap(),
            ));
        }
        let mut unknown32 = [0u8; 32];
        for b in unknown32.iter_mut() {
            *b = r.u8().unwrap();
        }
        let mut flags = [0u8; 16];
        for f in flags.iter_mut() {
            *f = r.u8().unwrap();
        }
        let mut slots = Vec::new();
        for (rt, f) in flags.iter().enumerate() {
            if *f != 0 {
                slots.push((rt, export_index));
                export_index += 1;
            }
        }
        out.push((
            MaterialRecord {
                id,
                textures,
                unknown32,
                compiled_state_flags: flags,
            },
            slots,
        ));
    }
    out
}

fn dump_arena_overview(data: &[u8]) {
    match parse_arena(data) {
        Some((sections, types)) => {
            println!(
                "  arena: {} sections, 类型表 {:?}",
                sections.len(),
                types.iter().map(|t| format!("0x{t:05X}")).collect::<Vec<_>>()
            );
            let mut hist: BTreeMap<u32, usize> = Default::default();
            for s in &sections {
                *hist.entry(s.type_code).or_insert(0) += 1;
            }
            for (t, n) in &hist {
                println!("    type 0x{t:05X}: {n} sections");
            }
        }
        None => println!("  arena 解析失败"),
    }
}

/// 对拍主流程：MaterialInfo + arena 联动，导出材质→compiled state 段映射。
fn cross_reference(info: &[u8], arena: &[u8], label: &str) {
    let records = parse_material_info_records(info);
    let Some((sections, _)) = parse_arena(arena) else {
        println!("[{label}] arena 解析失败");
        return;
    };
    let total_exports: usize = records.iter().map(|(_, s)| s.len()).sum();
    println!(
        "[{label}] {} 材质, {} 个 compiled-state 引用, arena 有 {} sections",
        records.len(),
        total_exports,
        sections.len()
    );
    const DECAL_MATERIALS: [(u32, &str); 3] = [
        (0x4491DE3A, "破洞/废墟"),
        (0x73684EFC, "招牌"),
        (0xE5390A98, "涂鸦"),
    ];
    let mut out = String::new();
    for (id, name) in DECAL_MATERIALS {
        let Some((rec, slots)) = records.iter().find(|(m, _)| m.id == id) else {
            println!("[{label}] {id:08X} ({name}) 不在表");
            continue;
        };
        for (rt, export_idx) in slots {
            let sec = sections.get(*export_idx);
            println!(
                "[{label}] {id:08X} ({name}) rt{rt} -> 导出#{export_idx}: {}",
                sec.map(|s| format!("pos=0x{:x} size={} type=0x{:05X}", s.pos, s.size, s.type_code))
                    .unwrap_or_else(|| "越界".into())
            );
            if let Some(s) = sec {
                out.push_str(&format!("{id:08X}\t{name}\trt{rt}\t{export_idx}\t0x{:x}\t{}\n", s.pos, s.size));
                // dump 该 section 原文
                let start = s.pos as usize;
                if let Some(bytes) = arena.get(start..start + s.size as usize) {
                    let fname = format!("tmp/cstate_{id:08X}_rt{rt}.bin");
                    std::fs::write(&fname, bytes).ok();
                    println!("    dumped -> {fname}");
                }
            }
        }
        let _ = rec;
    }
    std::fs::write(format!("tmp/decal_cstate_map_{label}.txt"), out).ok();
}

