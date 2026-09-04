//! `export-prop` CLI —— 对齐 C# `SimCityPak.exe export-prop <input> <outputDir> --json`。
//!
//! ```text
//! cargo run -p sc-exporter --bin export-prop -- <input.package> <outputDir> [--registry <database_main.s3db>]
//! ```
//!
//! 遍历包内全部 `0x00B1B104` 资源，按 C# 相同的 TGI 回退名写出 `<name>.json`。
//! 进度与摘要写 stderr；任一资源失败时退出码为 1（与 C# 行为可比）。

use std::path::PathBuf;
use std::process::ExitCode;

use dbpf::Package;
use sc_exporter::prop_json::{self, PROP_TYPE_ID};
use sc_registry::Registry;

struct Args {
    input: PathBuf,
    output_dir: PathBuf,
    registry: Option<PathBuf>,
    format: String,
}

fn parse_args() -> Option<Args> {
    let mut it = std::env::args().skip(1);
    let input = PathBuf::from(it.next()?);
    let output_dir = PathBuf::from(it.next()?);
    let mut registry = None;
    let mut format = "json".to_string();
    while let Some(arg) = it.next() {
        if arg == "--registry" {
            registry = it.next().map(PathBuf::from);
        } else if arg == "--format" {
            format = it.next()?;
        } else {
            eprintln!("unknown argument: {arg}");
            return None;
        }
    }
    Some(Args {
        input,
        output_dir,
        registry,
        format,
    })
}

fn fail(message: String) {
    eprintln!("{message}");
}

fn run() -> Result<(), ()> {
    let Some(args) = parse_args() else {
        fail(
            "Usage: export-prop <input.package> <outputDir> [--registry <database_main.s3db>] [--format json|text]"
                .into(),
        );
        return Err(());
    };

    let package = match Package::open(&args.input) {
        Ok(p) => p,
        Err(e) => {
            fail(format!(
                "ERROR: cannot open package {}: {e}",
                args.input.display()
            ));
            return Err(());
        }
    };
    let registry = match &args.registry {
        Some(path) => match Registry::open(path) {
            Ok(r) => Some(r),
            Err(e) => {
                fail(format!(
                    "ERROR: cannot open registry {}: {e}",
                    path.display()
                ));
                return Err(());
            }
        },
        None => None,
    };
    if let Err(e) = std::fs::create_dir_all(&args.output_dir) {
        fail(format!(
            "ERROR: cannot create {}: {e}",
            args.output_dir.display()
        ));
        return Err(());
    }

    let mut ok = 0usize;
    let mut fails = 0usize;
    let entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROP_TYPE_ID)
        .cloned()
        .collect();

    let extension =
        if args.format.eq_ignore_ascii_case("text") || args.format.eq_ignore_ascii_case("txt") {
            "txt"
        } else {
            "json"
        };
    for entry in &entries {
        let name = prop_json::fallback_name(entry.id);
        let outcome = prop_json::dump_resource(&package, entry, registry.as_ref())
            .map_err(|e| format!("dump: {e}"))
            .and_then(|dump| {
                let body = if extension == "txt" {
                    Ok(prop_json::to_text(&dump))
                } else {
                    prop_json::to_json(&dump)
                };
                body.map_err(|e| format!("serialize: {e}"))
            })
            .and_then(|body| {
                let path = args.output_dir.join(format!("{name}.{extension}"));
                std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))
            });
        match outcome {
            Ok(()) => {
                ok += 1;
                eprintln!("OK   {name}.{extension}");
            }
            Err(reason) => {
                fails += 1;
                eprintln!("FAIL {name} :: {reason}");
            }
        }
    }

    eprintln!();
    eprintln!("Done. dumped={ok} failed={fails}");
    if fails > 0 { Err(()) } else { Ok(()) }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}
