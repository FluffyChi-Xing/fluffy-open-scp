//! 探针：载具卡 3D 链路真实验证。
//! Agent property → Vehicle Models(0x0D897169) → 当前包 RW4 → 网格节(0x20009)。
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MODEL_TYPE: u32 = 0x2F4E_681B;
const MESH_SECTION: u32 = 0x20_009;
const KEY_VEHICLE_MODELS: u32 = 0x0D8_97169;

fn main() {
    let path = r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package";
    let package = Package::open(path).expect("open game package");
    let registry = sc_registry::Registry::open("src-tauri/resources/database_main.s3db").expect("registry");

    // 取 8 个有名字的 Agent 资源验证。
    let mut checked = 0;
    for entry in package.entries() {
        if checked >= 8 {
            break;
        }
        if entry.id.type_id != PROPERTY_TYPE || (entry.id.group & 0xFFFF) != 0xC600 {
            continue;
        }
        let name = registry.instance_name(entry.id.instance);
        if name.starts_with("0x") {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let Some(property) = file.get(KEY_VEHICLE_MODELS) else { continue };
        let mut models: Vec<(u32, u32, u32)> = Vec::new();
        let mut scan = |v: &Value| {
            if let Value::Key(k) = v {
                if k.type_id == MODEL_TYPE {
                    models.push((k.type_id, k.group, k.instance));
                }
            }
        };
        match &property.kind {
            Kind::Scalar(v) => scan(v),
            Kind::Array(vals) => {
                for v in vals {
                    scan(v);
                }
            }
            Kind::Empty => {}
        }
        if models.is_empty() {
            continue;
        }
        checked += 1;
        println!("--- {name} ({} 个模型)", models.len());
        for (t, g, i) in models.iter().take(3) {
            let tgi = dbpf::ResourceId { type_id: *t, group: *g, instance: *i };
            let Some(model_entry) = package.entry(tgi) else {
                println!("    模型 {:08X}: 不在 Game 包内（跨包）", i);
                continue;
            };
            let Ok(model_data) = package.read(model_entry) else { continue };
            let Ok(rw4) = rw4::Rw4File::parse(&model_data) else {
                println!("    模型 {:08X}: RW4 解析失败", i);
                continue;
            };
            let meshes: Vec<_> = rw4
                .sections()
                .iter()
                .filter(|s| s.type_code == MESH_SECTION)
                .map(|s| s.number)
                .collect();
            println!("    模型 {:08X}: 网格节 {:?}", i, meshes);
        }
    }
    println!("共验证 {checked} 个具名 Agent");
}
