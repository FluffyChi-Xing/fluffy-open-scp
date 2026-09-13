//! 顶点格式普查：模型是否携带 TANGENT/BINORMAL 顶点流（法线贴图 TBN 的来源）。
//! 用法：cargo run -p rw4 --release --example tangent_scan -- <package> [more...] [--max=N]
use std::collections::BTreeMap;

/// 模型类型（与 src-tauri 的 RW4_MODEL_TYPE 同值）。
const MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let max: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--max=").and_then(|v| v.parse().ok()))
        .unwrap_or(6);
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if let Some(hex) = args
        .iter()
        .find_map(|a| a.strip_prefix("--dump="))
        .map(str::to_string)
    {
        dump(&paths, &hex);
        return;
    }

    let mut with_tangent = 0usize;
    let mut without_tangent = 0usize;
    let mut layouts: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: Vec<(String, String)> = Vec::new();
    let mut norm_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut tangent_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut first_by_norm_type: BTreeMap<String, String> = BTreeMap::new();

    for path in &paths {
        let package = match dbpf::Package::open(path) {
            Ok(p) => p,
            Err(e) => {
                println!("skip {path}: {e}");
                continue;
            }
        };
        for entry in package.entries() {
            if entry.id.type_id != MODEL_TYPE {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else {
                continue;
            };
            let mut has_tangent = false;
            let mut layout = Vec::new();
            for s in file.sections_of_type(rw4::SectionType::VERTEX_FORMAT) {
                let Ok(vf) = file.decode_vertex_format(&data, s.number) else {
                    continue;
                };
                let mut parts = Vec::new();
                for el in &vf.elements {
                    let name = el.usage.name().to_string();
                    if el.usage == rw4::DeclarationUsage::Tangent
                        || el.usage == rw4::DeclarationUsage::Binormal
                    {
                        has_tangent = true;
                    }
                    if el.usage == rw4::DeclarationUsage::Normal {
                        let key = format!("{:?}", el.decl_type);
                        *norm_types.entry(key.clone()).or_default() += 1;
                        first_by_norm_type
                            .entry(key)
                            .or_insert_with(|| format!("0x{:08X}", entry.id.instance));
                    }
                    if el.usage == rw4::DeclarationUsage::Tangent {
                        *tangent_types.entry(format!("{:?}", el.decl_type)).or_default() += 1;
                    }
                    parts.push(format!("{name}{}:{:?}", el.index, el.decl_type));
                }
                layout.push(format!("stride={} [{}]", vf.vertex_size, parts.join(",")));
            }
            if layout.is_empty() {
                continue;
            }
            if has_tangent {
                with_tangent += 1;
                if samples.len() < max {
                    samples.push((format!("0x{:08X}", entry.id.instance), layout.join(" | ")));
                }
            } else {
                without_tangent += 1;
            }
            for l in &layout {
                *layouts.entry(l.clone()).or_default() += 1;
            }
        }
    }

    println!("== 带 TANGENT/BINORMAL: {with_tangent}  不带: {without_tangent}");
    println!("== NORMAL 声明类型: {norm_types:?}");
    println!("== TANGENT 声明类型: {tangent_types:?}");
    println!("== 各 NORMAL 类型首个实例: {first_by_norm_type:?}");
    println!("== 布局分布（前 8）");
    let mut sorted: Vec<_> = layouts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (layout, count) in sorted.iter().take(8) {
        println!("  x{count:<6} {layout}");
    }
    println!("== 带切线样例");
    for (instance, layout) in samples {
        println!("  {instance}  {layout}");
    }
}

/// 打印某模型每个 mesh 的前 2 条顶点的原始分量值（UB4/TexCoord 编码取证）。
fn dump(paths: &[&String], hex: &str) {
    let want = u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex");
    for path in paths {
        let Ok(package) = dbpf::Package::open(path) else {
            continue;
        };
        let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.instance == want && e.id.type_id == MODEL_TYPE)
            .cloned()
        else {
            continue;
        };
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(file) = rw4::Rw4File::parse(&data) else {
            continue;
        };
        for s in file.sections_of_type(rw4::SectionType::MESH) {
            let Ok(mesh) = file.decode_mesh(&data, s.number) else {
                continue;
            };
            println!("-- mesh #{}  verts={}", s.number, mesh.vertices.len());
            for (i, v) in mesh.vertices.iter().take(2).enumerate() {
                let parts: Vec<String> = v
                    .components
                    .iter()
                    .map(|(e, val)| format!("{}#{}={}", e.usage.name(), e.index, fmt(val)))
                    .collect();
                println!("   v{i}: {}", parts.join("  "));
            }
        }
    }
}

fn fmt(val: &rw4::ComponentValue) -> String {
    use rw4::ComponentValue as C;
    match val {
        C::Float1(a) => format!("F1({a:.4})"),
        C::Float2(a) => format!("F2({:.4},{:.4})", a[0], a[1]),
        C::Float3(a) => format!("F3({:.4},{:.4},{:.4})", a[0], a[1], a[2]),
        C::Float4(a) => format!("F4({:.4},{:.4},{:.4},{:.4})", a[0], a[1], a[2], a[3]),
        C::UByte4(b) => format!("UB4({},{},{},{})", b[0], b[1], b[2], b[3]),
        C::D3DColor { a, r, g, b } => format!("COLOR(a{a},r{r},g{g},b{b})"),
        C::Short2(a) => format!("S2({},{})", a[0], a[1]),
        C::Short4(a) => format!("S4({},{},{},{})", a[0], a[1], a[2], a[3]),
        C::Short4N(a) => format!("S4N({:.4},{:.4},{:.4},{:.4})", a[0], a[1], a[2], a[3]),
    }
}
