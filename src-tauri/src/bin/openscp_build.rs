//! M5 无头构建 CLI（build-pipeline-observability-20261105.md §5）。
//!
//! 用法：`openscp-build <project-root> [schema-path]`
//!
//! 默认从已保存项目解析 schema 和场景，使用与 Actions 相同的生产编译器。
//! 显式 schema 参数供独立命令行诊断使用。
//! 成功时把 `M1BuildResult`（含 events 台账）以 pretty JSON 打到 stdout；
//! 失败时把错误文本打到 stderr 并以非零码退出。构建产物与台账仍写入
//! 项目内 `.build/`（与 app 构建同位，便于基线对拍）。

use fluffy_open_scp_lib::road_asset;
use serde_json::Value;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--panel-runtime-export") {
        let [_, game_data, output] = args.as_slice() else {
            eprintln!("usage: openscp-build --panel-runtime-export <SimCityData> <output-directory>");
            std::process::exit(2);
        };
        match fluffy_open_scp_lib::asset_build_m1::panel::export_shared_runtime(
            std::path::Path::new(game_data), std::path::Path::new(output),
        ) {
            Ok(path) => println!("{}", path.display()),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    if args.first().is_some_and(|arg|arg=="--panel-runtime-check") {
        match panel_runtime_check(&args[1..]) {
            Ok(report)=>println!("{}",serde_json::to_string_pretty(&report).unwrap()),
            Err(error)=>{eprintln!("{error}");std::process::exit(1);},
        }
        return;
    }
    let Some(root_arg) = args.first() else {
        eprintln!("usage: openscp-build <project-root> [schema-path]");
        std::process::exit(2);
    };
    if args.len() > 2 {
        eprintln!("usage: openscp-build <project-root> [schema-path]");
        std::process::exit(2);
    }
    let root = PathBuf::from(root_arg);
    let (raw, input_label) = if let Some(schema) = args.get(1) {
        (match std::fs::read_to_string(schema) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!("read schema {schema}: {error}");
                std::process::exit(2);
            }
        }, schema.clone())
    } else if let Some((path, raw)) = fluffy_open_scp_lib::asset_build_m1::vanilla_variant_project_schema(&root)
        .and_then(|schema| match schema {
            Some(schema) => Ok(Some(schema)),
            None => road_asset::project_schema(&root),
        }).unwrap_or_else(|error| {
        eprintln!("discover asset schema: {error}");
        std::process::exit(2);
    }) {
        (raw, path)
    } else {
        let candidate = match fluffy_open_scp_lib::asset_build_m1::schema_discovery::discover_or_adapt(&root) {
            Ok(candidate) => candidate,
            Err(error) => {
                eprintln!("discover schema: {error}");
                std::process::exit(2);
            }
        };
        let mut schema: Value = match serde_json::from_str(&candidate.raw) {
            Ok(schema) => schema,
            Err(error) => {
                eprintln!("parse schema {}: {error}", candidate.path);
                std::process::exit(2);
            }
        };
        if let Err(errors) =
            fluffy_open_scp_lib::asset_build_m1::workbench_logic::overlay_document_on_schema(
                &root,
                &mut schema,
                &candidate.document,
            )
        {
            eprintln!("apply workbench: {}", errors.join("; "));
            std::process::exit(2);
        }
        (match serde_json::to_string(&schema) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!("serialize adapted schema: {error}");
                std::process::exit(2);
            }
        }, candidate.path)
    };
    let started = std::time::Instant::now();
    let result = if road_asset::is_schema(&raw) {
        road_asset::build(&root, &raw, None)
    } else {
        fluffy_open_scp_lib::asset_build_m1::build_from_schema(&root, &raw)
    };
    match result {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("result is JSON-serializable")
            );
            eprintln!(
                "build ok ({}): {} entries, {} events, took {:?}",
                input_label,
                result.resource_count,
                result.events.len(),
                started.elapsed()
            );
        }
        Err(error) => {
            eprintln!("build failed: {error}");
            std::process::exit(1);
        }
    }
}

fn panel_runtime_check(args:&[String])->Result<Value,String>{
    use fluffy_open_scp_lib::asset_build_m1::panel::runtime_bundle;
    if args.len()<3{return Err("usage: openscp-build --panel-runtime-check <stock.package> <shared-runtime.package> <required-runtime.package>...".into());}
    let read=|path:&str|runtime_bundle::read_package(std::path::Path::new(path));
    let stock=read(&args[0])?;let candidate=read(&args[1])?;
    let receipt=runtime_bundle::inspect(&candidate)?;
    let mut rows=vec![];
    for path in &args[2..] {
        let required=read(path)?;
        runtime_bundle::compatible(&stock,&required,&candidate).map_err(|e|format!("{e}:{path}"))?;
        rows.push(serde_json::json!({"requiredRuntime":path,"compatible":true}));
    }
    Ok(serde_json::json!({"sharedRuntime":args[1],"receipt":receipt,"requirements":rows,
        "comparisonLayer":"offline runtime source and declared API compatibility","runtimeVerified":false}))
}
