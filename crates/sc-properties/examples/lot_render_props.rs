//! 只读取证：dump 一个 lot property 的关键渲染属性值。
//! 目标：Model Bounding Box (0xF9EFBA 系) 与 LotBorderColor/borderWidth
//! （0xD7AF042-49）——地面描边与建筑在 lot 内偏移的数据源。
//! 用法：cargo run -p sc-properties --release --example lot_render_props -- <package> <instance>
use dbpf::Package;
use sc_properties::PropertyFile;

const LOT_PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("hex");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && e.id.type_id == LOT_PROPERTY_TYPE)
        .cloned()
        .expect("lot property not found");
    let raw = package.read(&entry).expect("read");
    let file = PropertyFile::parse(&raw).expect("parse property");

    let targets: &[(u32, &str)] = &[
        (0x00F9_EFBA, "Model Bounding Box"),
        (0x00F9_EFBB, "LOD1"),
        (0x0CCB_7FC9, "LotOverlayBoxOffset"),
        (0x0CCB_7FC8, "LotSize"),
        (0x0CCB_7FD0, "GroundTilePeriod(0xCCB7FD0)"),
        (0x0CCB_7FD1, "BaseTileFloat(0xCCB7FD1)"),
        (0x0CCB_7FD2, "0xCCB7FD2"),
        (0x0CCB_7FD3, "0xCCB7FD3"),
        (0x0D7A_F042, "LotBorderColor1"),
        (0x0D7A_F043, "LotBorderColor2"),
        (0x0D7A_F044, "LotBorderColor3"),
        (0x0D7A_F045, "LotBorderColor4"),
        (0x0D7A_F046, "borderWidth1"),
        (0x0D7A_F047, "borderWidth2"),
        (0x0D7A_F048, "borderWidth3"),
        (0x0D7A_F049, "borderWidth4"),
        (0x0DB7_FB17, "LotPlacementTransform"),
    ];
    for (hash, label) in targets {
        match file.get(*hash) {
            None => println!("0x{hash:08X} {label}: (absent)"),
            Some(prop) => println!("0x{hash:08X} {label}: {:?}", prop.kind),
        }
    }
}
