//! 探针：GraphicsCache/Cache 条目数 + SimCity.par 是否 DBPF + 全包扫 0x03E421ED group 段。仅开发用。
fn main() {
    for p in [
        "D:/ea-games/SimCity/SimCityUserData/GraphicsCache.package",
        "D:/ea-games/SimCity/SimCityUserData/Cache.package",
    ] {
        match dbpf::Package::open(p) {
            Ok(g) => println!("{}: {} 条目", p.rsplit('/').next().unwrap(), g.entries().len()),
            Err(e) => println!("{}: 打不开 {e}", p.rsplit('/').next().unwrap()),
        }
    }
    match dbpf::Package::open("D:/ea-games/SimCity/SimCity/SimCity.par") {
        Ok(g) => println!("SimCity.par: DBPF {} 条目", g.entries().len()),
        Err(e) => println!("SimCity.par: 不是 DBPF ({e})"),
    }
    // Graphics 包 0x03E421ED 总量与 group 样本（找可能的 cube 组段）
    let g = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package").unwrap();
    let mut groups: std::collections::HashMap<u32, usize> = Default::default();
    for e in g.entries() {
        if e.id.type_id == 0x03E4_21ED { *groups.entry(e.id.group).or_default() += 1; }
    }
    let mut v: Vec<_> = groups.iter().collect();
    v.sort_by_key(|p| std::cmp::Reverse(*p.1));
    println!("Graphics 0x03E421ED: {} 组", v.len());
    for (grp, n) in v.iter().take(15) { println!("  {grp:08X} ×{n}"); }
}
