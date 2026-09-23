//! 探针：确认 RegionRenderDto 序列化键名。仅开发用。
// 注意：DTO 在 src-tauri 里，这里复刻同结构验证 serde 行为。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct X {
    origin_world: (f32, f32),
    meters_per_pixel: f32,
    png_base64: String,
}
fn main() {
    let x = X { origin_world: (1.0, 2.0), meters_per_pixel: 8.0, png_base64: "a".into() };
    println!("{}", serde_json::to_string(&x).unwrap());
}
