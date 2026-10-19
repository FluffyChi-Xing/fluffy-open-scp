//! 探针：BC357A2B 逐城场地 z vs 三张 16 城模板 z 的配对差。仅开发用。
use sc_properties::region_3d::{city_site_z, parse_region_templates, plot_table};
use std::collections::HashMap;

fn main() {
    let base = "D:/ea-games/SimCity/SimCityData";
    let rt0 = dbpf::Package::open(format!("{base}/SimCity_RegionTerrain0.package")).unwrap();
    let ep1 = dbpf::Package::open(format!("{base}/SimCityDataEP1.package")).unwrap();
    let tpls = parse_region_templates(&ep1);
    let (uids, _pos, ids) = plot_table(&rt0, 0xA0B6_0DDE).unwrap();
    let mut means: HashMap<String, f64> = HashMap::new();
    for (i, uid) in uids.iter().enumerate() {
        if let Some(z) = city_site_z(&rt0, ids[i]) { means.insert(uid.clone(), z); }
    }
    for t in &tpls {
        if t.cities.len() != 7 { continue; }
        let mut diffs: Vec<String> = Vec::new();
        let mut sum = 0f64;
        let mut n = 0;
        for (uid, _, _, tz) in &t.cities {
            if let (Some(tz), Some(mz)) = (tz, means.get(uid)) {
                let d = mz - tz;
                sum += d.abs(); n += 1;
                diffs.push(format!("{d:+.0}"));
            }
        }
        println!("{:14} 平均|Δ|={:5.1}  Δ=[{}]", t.name, sum / n as f64, diffs.join(","));
    }
}
