//! 探针：转储 zh-tw locale 区域命名 JSON 全文。仅开发用。
fn main() {
    let loc = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/Locale/zh-tw/Data.package").unwrap();
    for e in loc.entries() {
        if e.id.type_id == 0x0A98_EAF0 && e.id.instance == 0x2C1D_9BDE {
            let d = loc.read(e).unwrap();
            std::fs::write("tmp/js/locale_region_names.json", &d).unwrap();
            println!("转储 {} B", d.len());
        }
    }
}
