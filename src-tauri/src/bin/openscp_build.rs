//! M5 无头构建 CLI（build-pipeline-observability-20261105.md §5）。
//!
//! 用法：`openscp-build <project-root> <schema-path>`
//!
//! 以磁盘 schema 文件驱动 asset_build_m1 责任链（与 build_queue_enqueue 的
//! 前端内存 schema 不同源——这正是 F1 "构建输入不入库" 的无头复现通道）。
//! 成功时把 `M1BuildResult`（含 events 台账）以 pretty JSON 打到 stdout；
//! 失败时把错误文本打到 stderr 并以非零码退出。构建产物与台账仍写入
//! 项目内 `.build/`（与 app 构建同位，便于基线对拍）。

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, schema] = args.as_slice() else {
        eprintln!("usage: openscp-build <project-root> <schema-path>");
        std::process::exit(2);
    };
    let root = PathBuf::from(root);
    let raw = match std::fs::read_to_string(schema) {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("read schema {schema}: {error}");
            std::process::exit(2);
        }
    };
    let started = std::time::Instant::now();
    match fluffy_open_scp_lib::asset_build_m1::build_from_schema(&root, &raw) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("result is JSON-serializable")
            );
            eprintln!(
                "build ok: {} entries, {} events, took {:?}",
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
