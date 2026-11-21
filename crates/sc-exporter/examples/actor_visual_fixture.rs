use dbpf::{Package, ResourceId};
use sc_exporter::actor_assets::{palette, read_part};
fn main() {
    let root =
        std::path::Path::new("D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData");
    let a = Package::open(root.join("SimCity_App.package")).unwrap();
    let g = Package::open(root.join("SimCity_Graphics.package")).unwrap();
    let p = vec![&a, &g];
    let part = |i| {
        read_part(
            &p,
            ResourceId {
                type_id: 0x2F4E681B,
                group: 0,
                instance: i,
            },
        )
        .unwrap()
    };
    let data = serde_json::json!({"body":part(0xE5537FAB),"head":part(0x22A445FE),"bodyPalette":palette(&p,0x69C94C5D).unwrap(),"clothesPalette":palette(&p,0x14C5F13F).unwrap()});
    std::fs::write(
        "tmp/ui-review/actor-native.json",
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
}
