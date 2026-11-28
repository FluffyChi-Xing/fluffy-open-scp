//! Add the validated retail UI patch to the exact r2 map package.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::Path};

const R2_SHA: &str = "e244a5b4ca1d0393fa2236c79bd7bb2cc4a3a333a8453118a4d91bab5bb779b6";
const STOCK_UI_SHA: &str = "683c66fe0d0146b9b630bbe59e1df89848fc05d907f3028147c01ff4f407431b";
const UI: ResourceId = ResourceId {
    type_id: 0x67771f5c,
    group: 0x40464200,
    instance: 0x7ccc548c,
};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn patch(stock: &[u8], output: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    // Run the fingerprinted transformation and its two failure replays before packaging.
    assert_eq!(sha(stock), STOCK_UI_SHA);
    let stock_path = format!("{output}.stock.js");
    let patch_path = format!("{output}.patched.js");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stock_path)?
        .write_all(stock)?;
    let status = std::process::Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/edgewater-single-slot-ui.cjs"))
        .arg(&stock_path)
        .arg(&patch_path)
        .status()?;
    if !status.success() {
        return Err("retail UI patch replay failed".into());
    }
    Ok(fs::read(patch_path)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [r2, game_package, output] = args.as_slice() else {
        return Err("expected exact-r2.package SimCity_Game.package out.package".into());
    };
    assert_eq!(sha(&fs::read(r2)?), R2_SHA);
    let r2 = Package::open(r2)?;
    assert_eq!(r2.entries().len(), 5);
    assert!(r2.entry(UI).is_none());
    let game = Package::open(game_package)?;
    let stock = game.read(game.entry(UI).ok_or("retail UI resource missing")?)?;
    let mut entries = r2
        .entries()
        .iter()
        .map(|e| r2.read(e).map(|bytes| OverlayEntry::new(e.id, bytes)))
        .collect::<Result<Vec<_>, _>>()?;
    entries.push(OverlayEntry::new(UI, patch(&stock, output)?));
    let bytes = write_uncompressed_overlay(&entries)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    let written = Package::open(output)?;
    assert_eq!(written.entries().len(), entries.len());
    for entry in entries {
        assert_eq!(written.read(written.entry(entry.id).unwrap())?, entry.data);
    }
    println!(
        "5 unchanged r2 resources + verified UI fix; SHA-256 {}",
        sha(&bytes)
    );
    Ok(())
}
