use super::spec::Source;
use dbpf::{Package, ResourceId};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const GAME_JS: ResourceId = ResourceId {
    type_id: 0x6777_1F5C,
    group: 0x4046_4200,
    instance: 0x7CCC_548C,
};
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub fn path(root: &Path, raw: &str) -> Result<PathBuf, String> {
    if Path::new(raw).is_absolute() {
        let p = PathBuf::from(raw);
        if p.is_file() {
            Ok(p)
        } else {
            Err(format!("PANEL_SOURCE_MISSING:{raw}"))
        }
    } else {
        super::super::filesystem::project_file(root, raw)
    }
}
pub fn open(root: &Path, sources: &[Source]) -> Result<Vec<Package>, String> {
    if sources.is_empty() || sources.len() > 24 {
        return Err("PANEL_SOURCES_REQUIRED".into());
    }
    sources
        .iter()
        .map(|s| {
            let p = path(root, &s.path)?;
            if digest_file(&p)? != s.sha256 {
                return Err(format!("PANEL_SOURCE_CHANGED:{}", s.path));
            }
            Package::open(&p).map_err(|e| e.to_string())
        })
        .collect()
}
pub fn read(packages: &[Package], id: ResourceId) -> Result<Vec<u8>, String> {
    for p in packages.iter().rev() {
        if let Some(e) = p.entry(id) {
            return p.read(e).map_err(|e| e.to_string());
        }
    }
    Err(format!("PANEL_RESOURCE_MISSING:{id}"))
}
pub fn source(path: &Path) -> Result<Source, String> {
    Ok(Source {
        path: path.to_string_lossy().into(),
        sha256: digest_file(path)?,
    })
}
