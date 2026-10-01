//! sc-shader CLI：
//!   extract — 容器无损解析（对照人工清理版，报告缺失/差异）
//!   emit    — 组合 4 族 GLSL 资产 → src/assets/shaders/decal/
use anyhow::{bail, Context, Result};
use std::path::PathBuf;

mod compose;
mod container;
mod fragments;
mod translate;

#[derive(clap::Parser)]
#[allow(rustdoc::invalid_html_tags)]
enum Cmd {
    /// 解析容器 bin，列出全部片段（含大小），并对照人工清理表
    Extract {
        /// 容器 bin 路径（可多个）
        bins: Vec<PathBuf>,
    },
    /// 组合 4 族 GLSL 资产
    Emit {
        /// 输出目录（默认 src/assets/shaders/decal）
        #[clap(long, default_value = "src/assets/shaders/decal")]
        out: PathBuf,
        /// 同时写入 uniform 清单 JSON（供前端工厂校验）
        #[clap(long, default_value_t = true)]
        manifest: bool,
    },
}

fn main() -> Result<()> {
    match sc_cli() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("错误: {e:#}");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn sc_cli() -> Result<()> {
    use clap::Parser as _;
    let cmd = Cmd::parse();
    match cmd {
        Cmd::Extract { bins } => {
            if bins.is_empty() {
                bail!("至少给一个容器 bin（tmp/dynamic/shader_container_*.bin）");
            }
            let mut all: std::collections::BTreeMap<String, usize> = Default::default();
            for bin in &bins {
                let data = std::fs::read(bin).with_context(|| format!("read {bin:?}"))?;
                for f in container::parse(&data) {
                    let e = all.entry(f.name).or_default();
                    if f.body.len() > *e {
                        *e = f.body.len();
                    }
                }
            }
            println!("共 {} 个命名片段：", all.len());
            for (name, size) in &all {
                let cleaned = if fragments::get(name).is_some() { "✓清理版" } else { "" };
                println!("  {name} ({size}B) {cleaned}");
            }
            let core = fragments::CORE_FRAGMENTS.len();
            let missing: Vec<_> = fragments::CORE_FRAGMENTS
                .iter()
                .filter(|(n, _)| !all.contains_key(*n))
                .map(|(n, _)| *n)
                .collect();
            if missing.is_empty() {
                println!("清理表 {core} 条全部在容器中命中");
            } else {
                println!("⚠ 清理表中 {} 条未在容器命中（人工重建片段，属预期）：{missing:?}", missing.len());
            }
        }
        Cmd::Emit { out, manifest } => {
            std::fs::create_dir_all(&out)?;
            let mut manifest_obj = serde_json::Map::new();
            for family in compose::Family::ALL {
                let (vs, ps) = compose::compose(family)?;
                let base = family.name();
                let vs_path = out.join(format!("{}.vert.glsl", base));
                let ps_path = out.join(format!("{}.frag.glsl", base));
                std::fs::write(&vs_path, &vs)?;
                std::fs::write(&ps_path, &ps)?;
                println!("写出 {} / {}", vs_path.display(), ps_path.display());
                manifest_obj.insert(
                    format!("{base}.vert"),
                    serde_json::Value::String(vs_path.file_name().unwrap().to_string_lossy().into()),
                );
                manifest_obj.insert(
                    format!("{base}.frag"),
                    serde_json::Value::String(ps_path.file_name().unwrap().to_string_lossy().into()),
                );
            }
            if manifest {
                let path = out.join("manifest.json");
                std::fs::write(&path, serde_json::to_string_pretty(&manifest_obj)?)?;
                println!("写出 {}", path.display());
            }
        }
    }
    Ok(())
}
