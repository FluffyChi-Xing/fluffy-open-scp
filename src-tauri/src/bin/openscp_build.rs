//! M5 无头构建 CLI（build-pipeline-observability-20261105.md §5）。
//!
//! 用法：`openscp-build <project-root> [schema-path]`
//!
//! 以磁盘 schema 文件驱动 asset_build_m1 责任链（与 build_queue_enqueue 的
//! 前端内存 schema 不同源——这正是 F1 "构建输入不入库" 的无头复现通道）。
//! 成功时把 `M1BuildResult`（含 events 台账）以 pretty JSON 打到 stdout；
//! 失败时把错误文本打到 stderr 并以非零码退出。构建产物与台账仍写入
//! 项目内 `.build/`（与 app 构建同位，便于基线对拍）。

use serde_json::Value;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        let document_path = root.join(&candidate.document);
        if document_path.is_file() {
            let document: Value = match serde_json::from_slice(&std::fs::read(&document_path).unwrap_or_default()) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!("parse document {}: {error}", candidate.document);
                    std::process::exit(2);
                }
            };
            let compiled = match fluffy_open_scp_lib::asset_build_m1::workbench_logic::compile(
                &document,
                &|path| root.join(path).is_file(),
            ) {
                Ok(compiled) => compiled,
                Err(errors) => {
                    eprintln!("compile workbench: {}", errors.join("; "));
                    std::process::exit(2);
                }
            };
            if let Err(errors) = fluffy_open_scp_lib::asset_build_m1::workbench_logic::apply_to_schema(&mut schema, &compiled) {
                eprintln!("apply workbench: {}", errors.join("; "));
                std::process::exit(2);
            }
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
    match fluffy_open_scp_lib::asset_build_m1::build_from_schema(&root, &raw) {
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
