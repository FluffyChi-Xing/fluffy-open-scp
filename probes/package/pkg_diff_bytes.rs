//! 打印两包中指定 TGI 解压内容的逐字节差异位置与值。仅开发用。
//! 用法：pkg_diff_bytes <vanilla> <modded> <type> <group> <instance> [...]
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02X}")).collect() }
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (van, modded) = (&args[0], &args[1]);
    let a = dbpf::Package::open(van).unwrap();
    let b = dbpf::Package::open(modded).unwrap();
    for tgi in args[2..].chunks(3) {
        let id = dbpf::ResourceId {
            type_id: u32::from_str_radix(&tgi[0], 16).unwrap(),
            group: u32::from_str_radix(&tgi[1], 16).unwrap(),
            instance: u32::from_str_radix(&tgi[2], 16).unwrap(),
        };
        let (Some(ea), Some(eb)) = (a.entry(id), b.entry(id)) else { println!("missing {id:?}"); continue };
        let (va, vb) = (a.read(ea).unwrap(), b.read(eb).unwrap());
        println!("== {:08X}:{:08X}:{:08X} len {} vs {}", id.type_id, id.group, id.instance, va.len(), vb.len());
        let mut i = 0;
        while i < va.len().min(vb.len()) {
            if va[i] != vb[i] {
                let s = i.saturating_sub(8); let e = (i + 9).min(va.len().min(vb.len()));
                println!("  @0x{i:04X}: van={} boc={}", hex(&va[s..e]), hex(&vb[s..e]));
            }
            i += 1;
        }
    }
}
