//! M2 差分测试（oracle 门控）：
//!
//! - 未配置 `OPEN_SCP_CSHARP_EXPORT_PROP` 时显式跳过（跳过 ≠ 验证通过）；
//! - 配置后运行 C# `SimCityPak.exe export-prop <pkg> <out> --json` 生成金样本，
//!   与 Rust 协议层输出逐文件、逐属性比对。
//!
//! ```text
//! OPEN_SCP_CSHARP_EXPORT_PROP=D:\path\SimCityPak.exe \
//! OPEN_SCP_M2_PACKAGE=docs/packages/app.package \
//! cargo test -p sc-exporter --test m2_differential -- --nocapture
//! ```
//!
//! `name` 字段的差异只告警不计失败：C# 的 PropName 依赖其 exe 目录下的
//! database_main.s3db（相对路径解析），差分机上不可控；`hash/type/value`
//! 为严格比对字段。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use dbpf::Package;
use sc_exporter::prop_json::{PROP_TYPE_ID, dump_resource, fallback_name, to_json};
use sc_registry::Registry;

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[test]
fn rust_prop_json_matches_csharp_oracle_when_configured() {
    let Some(csharp_exe) = env_path("OPEN_SCP_CSHARP_EXPORT_PROP") else {
        eprintln!("skipping: OPEN_SCP_CSHARP_EXPORT_PROP not configured (C# oracle unavailable)");
        return;
    };
    let package_path = env_path("OPEN_SCP_M2_PACKAGE").unwrap_or_else(|| {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/packages/app.package"
        ))
    });
    if !package_path.exists() {
        eprintln!("skipping: package {} not present", package_path.display());
        return;
    }
    assert!(
        csharp_exe.exists(),
        "C# oracle not found at {}",
        csharp_exe.display()
    );

    let out_root = std::env::temp_dir().join(format!("openscp-m2-diff-{}", std::process::id()));
    let csharp_out = out_root.join("csharp");
    let rust_out = out_root.join("rust");
    std::fs::create_dir_all(&csharp_out).unwrap();
    std::fs::create_dir_all(&rust_out).unwrap();

    // C# 侧：export-prop <input> <outputDir> --json
    let status = Command::new(&csharp_exe)
        .arg("export-prop")
        .arg(&package_path)
        .arg(&csharp_out)
        .arg("--json")
        .status()
        .expect("failed to spawn C# oracle");
    assert!(status.success(), "C# export-prop exited with {status}");

    // Rust 侧：同一包、同一命名规则
    let package = Package::open(&package_path).unwrap();
    let registry_path = env_path("OPEN_SCP_M2_REGISTRY");
    let registry = registry_path
        .as_deref()
        .map(Registry::open)
        .transpose()
        .unwrap();
    let entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROP_TYPE_ID)
        .cloned()
        .collect();
    for entry in &entries {
        let dump = dump_resource(&package, entry, registry.as_ref()).expect("rust dump");
        let path = rust_out.join(format!("{}.json", fallback_name(entry.id)));
        std::fs::write(&path, to_json(&dump).unwrap()).unwrap();
    }

    // 目录级比对：文件集合一致
    let list = |dir: &PathBuf| -> BTreeMap<String, serde_json::Value> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .map(|p| {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                let value: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                (name, value)
            })
            .collect()
    };
    let csharp_files = list(&csharp_out);
    let rust_files = list(&rust_out);

    let missing: Vec<_> = csharp_files
        .keys()
        .filter(|k| !rust_files.contains_key(*k))
        .collect();
    let extra: Vec<_> = rust_files
        .keys()
        .filter(|k| !csharp_files.contains_key(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "files missing from rust output: {missing:?}"
    );
    assert!(extra.is_empty(), "unexpected rust output files: {extra:?}");
    assert!(!csharp_files.is_empty(), "oracle produced no files");

    // 逐文件比对：propertyCount + properties 数组严格一致；name 差异仅告警
    let mut failures = 0usize;
    let mut name_warnings = 0usize;
    for (file, csharp) in &csharp_files {
        let Some(rust) = rust_files.get(file) else {
            continue;
        };
        let cs_props = csharp["properties"]
            .as_array()
            .expect("csharp properties array");
        let rs_props = rust["properties"]
            .as_array()
            .expect("rust properties array");

        let cs_count = csharp["propertyCount"]
            .as_u64()
            .unwrap_or(cs_props.len() as u64);
        let rs_count = rust["propertyCount"]
            .as_u64()
            .unwrap_or(rs_props.len() as u64);
        if cs_count != rs_count {
            eprintln!("DIFF {file}: propertyCount {cs_count} != {rs_count}");
            failures += 1;
            continue;
        }
        if cs_props.len() != rs_props.len() {
            eprintln!(
                "DIFF {file}: properties len {} != {}",
                cs_props.len(),
                rs_props.len()
            );
            failures += 1;
            continue;
        }
        for (idx, (c, r)) in cs_props.iter().zip(rs_props.iter()).enumerate() {
            let hash = c["hash"].as_str().unwrap_or("?").to_string();
            for field in ["hash", "type", "value"] {
                if c[field] != r[field] {
                    eprintln!(
                        "DIFF {file} [{hash}] (#{idx}) {field}: {} != {}",
                        c[field], r[field]
                    );
                    failures += 1;
                }
            }
            if c["name"] != r["name"] {
                name_warnings += 1;
                eprintln!(
                    "WARN {file} [{hash}] name differs (oracle db-dependent): {} != {}",
                    c["name"], r["name"]
                );
            }
        }
    }

    eprintln!(
        "compared {} files, {failures} field failures, {name_warnings} name warnings",
        csharp_files.len()
    );
    assert_eq!(failures, 0, "differential mismatches found");
    let _ = std::fs::remove_dir_all(&out_root);
}
