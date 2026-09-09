//! 游戏 UI 资产盘点：扫描 SimCityData 全部 package，统计 CSS / 布局 JSON /
//! 图片资源的分布与命名，可选 `--extract <dir>` 解包到磁盘。
//!
//! ```text
//! cargo run -p dbpf --example ui_assets -- <simcity-data-dir> [--extract <out>]
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

const TYPE_CSS: u32 = 0x2C97_8DB6;
const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_JPG: u32 = 0x3F86_62EA;
const TYPE_GIF: u32 = 0x2F7D_0007;
const TYPE_JSON: u32 = 0x0A98_EAF0;
const GROUP_LAYOUT: u32 = 0x0B07_4E5A;

#[derive(Default)]
struct Bucket {
    count: usize,
    decompressed: u64,
    names: Vec<String>,
}

fn name_hint(ext: &str, id: &dbpf::ResourceId, sample: &str) -> String {
    // 布局 JSON 内通常引用自身路径；CSS 取首行注释或首个 url() 目标
    let _ = sample;
    format!("{ext}/{:08X}_{:08X}", id.group, id.instance)
}

fn main() -> dbpf::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut extract: Option<PathBuf> = None;
    if let Some(pos) = args.iter().position(|a| a == "--extract") {
        extract = Some(PathBuf::from(args[pos + 1].clone()));
        args.drain(pos..=pos + 1);
    }
    let root = args
        .first()
        .expect("usage: ui_assets <simcity-data-dir> [--extract <out>]");
    let extract = extract.unwrap_or_else(|| PathBuf::from("tmp/game-ui"));

    // 递归收集全部 .package（覆盖 SimCityData/Locale/<lang>/*.package）
    fn collect_packages(dir: &std::path::Path, out: &mut Vec<PathBuf>, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                collect_packages(&path, out, depth + 1);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("package"))
            {
                out.push(path);
            }
        }
    }
    let mut packages: Vec<PathBuf> = Vec::new();
    collect_packages(std::path::Path::new(root), &mut packages, 0);
    packages.sort();

    // (kind, package) -> Bucket
    let mut buckets: BTreeMap<(String, String), Bucket> = BTreeMap::new();
    let mut layouts_with_names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut extracted = 0usize;

    for path in &packages {
        scan_package(path, &mut buckets, &mut layouts_with_names, &extract, &mut extracted, true)?;
    }

    println!("=== UI asset census ===");
    for ((kind, package), bucket) in &buckets {
        println!(
            "{kind:>8}  {package:<40} {:>5} files  {:>12} bytes  e.g. {}",
            bucket.count,
            bucket.decompressed,
            bucket.names.first().map(String::as_str).unwrap_or("-")
        );
    }
    println!("\n=== layout JSON windows (top groups) ===");
    for (package, names) in layouts_with_names.iter().take(60) {
        println!("{package}: {} layouts, e.g. {}", names.len(), names.first().map(String::as_str).unwrap_or("-"));
    }
    println!("\nextracted {extracted} files to {}", extract.display());
    Ok(())
}

fn scan_package(
    path: &std::path::Path,
    buckets: &mut BTreeMap<(String, String), Bucket>,
    layouts_with_names: &mut BTreeMap<String, Vec<String>>,
    extract: &std::path::Path,
    extracted: &mut usize,
    do_extract: bool,
) -> dbpf::Result<()> {
    let package_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let package = match dbpf::Package::open(path) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    for entry in package.entries() {
        // locale 字符串表 / UI 清单 JSON（同为 0x0A98EAF0，靠内容区分）
        if entry.id.type_id == TYPE_JSON {
            let data = package.read(entry).unwrap_or_default();
            let text = String::from_utf8_lossy(&data);
            if text.contains("\"files\"") {
                let out = extract.join(format!("manifest/{:08X}.json", entry.id.instance));
                std::fs::create_dir_all(out.parent().unwrap()).ok();
                std::fs::write(&out, &*data).ok();
            } else {
                // 去掉 UTF-8 BOM 后存为 locale/<instance>.json
                let payload = data.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&data);
                let out = extract.join(format!("locale/{:08X}.json", entry.id.instance));
                std::fs::create_dir_all(out.parent().unwrap()).ok();
                std::fs::write(&out, payload).ok();
            }
            continue;
        }
        let kind = match entry.id.type_id {
            TYPE_CSS => "css",
            TYPE_PNG => "png",
            TYPE_JPG => "jpg",
            TYPE_GIF => "gif",
            _ => continue,
        };
        let _ = GROUP_LAYOUT;
        let bucket = buckets
            .entry((kind.to_string(), package_name.clone()))
            .or_default();
        bucket.count += 1;
        bucket.decompressed += u64::from(entry.decompressed_size);

        let data = package.read(entry).unwrap_or_default();
        let text = String::from_utf8_lossy(&data).into_owned();
        let label = describe(kind, entry.id, &text);
        if bucket.names.len() < 6 {
            bucket.names.push(label.clone());
        }
        if kind == "layout" {
            layouts_with_names
                .entry(package_name.clone())
                .or_default()
                .push(label.clone());
        }

        if do_extract {
            let rel = sanitize(&label);
            let out = extract.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if std::fs::write(&out, &data).is_ok() {
                *extracted += 1;
            }
        }
    }
    Ok(())
}

fn is_json(type_id: u32) -> bool {
    type_id == 0x0A98_EAF0
}

fn describe(kind: &str, id: dbpf::ResourceId, text: &str) -> String {
    let mut end = text.len().min(2048);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let head = &text[..end];
    match kind {
        // CSS 头部注释自标识：`/* TextInputStyles.css`
        "css" => {
            // 1) 头部注释自标识（`/* TextInputStyles.css`）；
            // 2) 无标识的 autogen 文件取首个类名（`.autogenButtonNineSliceStyles`）。
            let name = head
                .split_whitespace()
                .find(|token| token.ends_with(".css"))
                .map(|token| {
                    let mut name = sanitize_filename(token.trim_end_matches(','));
                    if !name.ends_with(".css") {
                        name.push_str(".css");
                    }
                    name
                })
                .or_else(|| {
                    head.find('.').map(|start| {
                        let rest = &head[start + 1..];
                        let end = rest
                            .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
                            .unwrap_or(rest.len());
                        let mut name = sanitize_filename(&rest[..end]);
                        if !name.ends_with(".css") {
                            name.push_str(".css");
                        }
                        name
                    })
                })
                .unwrap_or_else(|| format!("{:08X}_{:08X}", id.group, id.instance));
            format!("css/{name}")
        }
        other => format!(
            "{other}/{:08X}_{:08X}.{other}",
            id.group, id.instance
        ),
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}

fn extract_json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let q1 = rest.find('"')? + 1;
    let q2 = rest[q1..].find('"')? + q1;
    Some(rest[q1..q2].to_string())
}

fn sanitize(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '/' => c,
            _ => '_',
        })
        .collect()
}
