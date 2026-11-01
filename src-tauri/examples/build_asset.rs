//! Manual backend build entry point.
//!
//! Usage:
//!   cargo run -p fluffy-open-scp --example build_asset -- <project-root> <schema.json>
//!
//! This deliberately does not touch the Tauri asset workbench.  It exists so
//! the packaging pipeline can be exercised and validated on fixtures.

use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let root = args
        .next()
        .ok_or("usage: build_asset <root> <schema.json>")?;
    let schema_path = args
        .next()
        .ok_or("usage: build_asset <root> <schema.json>")?;
    if args.next().is_some() {
        return Err("usage: build_asset <root> <schema.json>".into());
    }
    let schema = fs::read_to_string(&schema_path)?;
    let result = fluffy_open_scp_lib::asset_build_m1::build_from_schema(Path::new(&root), &schema)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
