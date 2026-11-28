//! Build and verify the updater test button and optional backdrop.

use dbpf::{OverlayEntry, Package, ResourceId};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::Path};

#[path = "updater_test_background.rs"]
mod background;

const UPDATER_HTML: ResourceId = ResourceId {
    type_id: 0xDD62_33D6,
    group: 0x4046_4200,
    instance: 0xCB74_143C,
};
const PROBE: &str = include_str!("../../crates/sc-ui/src/updater_test_button.js");
const BOOTSTRAP: &str = include_str!("updater_test_bootstrap.js");
const LIVE_RELOAD: &str = include_str!("updater_live_reload.js");

const RESOURCE_DIALOG: &str = include_str!("updater_resource_dialog.js");

fn patch(stock: &str, token: Option<&str>) -> Result<String, &'static str> {
    if stock.matches("</body>").count() != 1
        || stock.matches("simcity.gUpdater.Initialize();").count() != 1
        || !stock.contains("Packed/SCRUI_min.js")
        || !stock.contains("Packed/Updater_min.js")
        || stock.contains("openscp-updater-ui-test")
    {
        return Err("unexpected or already modified updater document");
    }
    if [PROBE, BOOTSTRAP, LIVE_RELOAD, RESOURCE_DIALOG]
        .iter()
        .any(|script| script.to_ascii_lowercase().contains("</script"))
    {
        return Err("unexpected script end tag in bundled JavaScript");
    }
    let live = match token {
        Some(value) if value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()) => {
            format!("\n{LIVE_RELOAD}(\"{value}\", {RESOURCE_DIALOG});")
        }
        Some(_) => return Err("live preview token must be 32 hex digits"),
        None => String::new(),
    };
    Ok(stock.replacen(
        "</body>",
        &format!(
            "<script type=\"text/javascript\">\n{BOOTSTRAP}({PROBE});{live}\n</script>\n    </body>"
        ),
        1,
    ))
}

fn build(
    stock_path: &Path,
    output: &Path,
    image: Option<&Path>,
    token: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let stock = Package::open(stock_path)?;
    let mut matches = stock
        .entries()
        .iter()
        .filter(|entry| entry.id == UPDATER_HTML);
    let entry = matches.next().ok_or("stock updater resource missing")?;
    if matches.next().is_some() {
        return Err("stock updater resource is ambiguous".into());
    }
    let baseline = stock.read(entry)?;
    let patched = patch(std::str::from_utf8(&baseline)?, token)?;
    let mut entries = vec![OverlayEntry::new(UPDATER_HTML, patched.as_bytes())];
    if let Some(image) = image {
        entries.extend(background::entries(&stock, image, token.is_some())?);
    }
    let bytes = dbpf::write_uncompressed_overlay(&entries)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    let verified = Package::open(output)?;
    if verified.entries().len() != entries.len() {
        return Err("updater package readback does not match the candidate".into());
    }
    for expected in &entries {
        let entry = verified
            .entry(expected.id)
            .ok_or("output resource missing")?;
        if verified.read(entry)? != expected.data {
            return Err(format!("output resource does not match: {}", expected.id).into());
        }
    }
    println!(
        "stock updater payload SHA-256: {:x}",
        Sha256::digest(baseline)
    );
    println!(
        "patched updater payload SHA-256: {:x}",
        Sha256::digest(patched)
    );
    println!("overlay package SHA-256: {:x}", Sha256::digest(bytes));
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [command, paths @ ..] if command == "scan" && !paths.is_empty() => {
            for path in paths.iter().map(Path::new) {
                let package = Package::open(path)?;
                if package.entry(UPDATER_HTML).is_some() {
                    println!("{}", path.display());
                }
            }
            Ok(())
        }
        [command, stock, output] if command == "build" => {
            build(Path::new(stock), Path::new(output), None, None)
        }
        [command, stock, output, image] if command == "build" => {
            build(Path::new(stock), Path::new(output), Some(Path::new(image)), None)
        }
        [command, stock, output, image, token_file] if command == "live" => {
            let token = fs::read_to_string(token_file)?;
            build(Path::new(stock), Path::new(output), Some(Path::new(image)), Some(token.trim()))
        }
        _ => Err("usage: updater_test_package build <stock.package> <output.package> [background.png] | live <stock.package> <output.package> <background.png> <token-file> | scan <package>...".into()),
    }
}
