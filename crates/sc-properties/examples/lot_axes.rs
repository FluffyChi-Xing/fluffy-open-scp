//! 取证工具 v2：扫描包内 Lot 属性，按 Unit 类型统计并打印原始变换，
//! 用于确定渲染约定。仅开发用。
//! 用法：cargo run -p sc-properties --example lot_axes -- <package> [min_props]

use dbpf::ResourceId;
use sc_properties::LotUnit;

fn fmt(m: &[f32; 12]) -> String {
    format!(
        "[{:.2} {:.2} {:.2} | {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2}] T({:.1},{:.1},{:.1})",
        m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]
    )
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_axes <package>");
    let min_props: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(0);
    let package = dbpf::Package::open(&path).expect("open package");
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default()) else { continue };
        let document = sc_properties::LotEditorDocument::from_property_file(file);
        let units = document.assemble_units();
        let lights: Vec<_> = units.units.iter().filter(|u| matches!(u, LotUnit::Light { .. })).collect();
        let props: Vec<_> = units.units.iter().filter(|u| matches!(u, LotUnit::Prop { .. })).collect();
        if lights.is_empty() || props.len() < min_props { continue; }
        let spots = lights.iter().filter(|u| matches!(u, LotUnit::Light { light_type: Some("Spot"), .. })).count();
        println!("=== 0x{:08X} units={} lights={} spots={} props={}", entry.id.instance, units.units.len(), lights.len(), spots, props.len());
        for u in &units.units {
            match u {
                LotUnit::Light { index, light_type, transform, length, .. } => println!("  L[{}] {:?} len={:?} {:?}", index, light_type, length, transform.as_ref().map(|t| fmt(&t.matrix))),
                LotUnit::Prop { index, bin, transform, .. } => println!("  P[{}] bin={} {:?}", index, bin, transform.as_ref().map(|t| fmt(&t.matrix))),
                LotUnit::Spawner { index, transform, .. } => println!("  S[{}] {:?}", index, transform.as_ref().map(|t| fmt(&t.matrix))),
                LotUnit::Decal { index, category, transform, scale, .. } => println!("  D[{}] cat={} scale={:?} {:?}", index, category, scale, transform.as_ref().map(|t| fmt(&t.matrix))),
                LotUnit::Effect { index, transform, .. } => println!("  E[{}] {:?}", index, transform.as_ref().map(|t| fmt(&t.matrix))),
                LotUnit::PathPoint { index, point, .. } => println!("  W[{}] {:?}", index, point),
            }
        }
    }
}
