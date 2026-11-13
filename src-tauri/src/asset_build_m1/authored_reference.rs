//! Bootstrap the same stock reference used by the Asset Editor's DLC0 fallback.
//! The reference supplies build inputs only. The authored ground stays in the project.
use std::{fs, io, path::Path};

pub(super) fn prepare(root: &Path) -> Result<(), String> {
    let target = root.join("build/reference.package");
    if target.is_file() {
        return validate(&target);
    }
    let source = super::schema_discovery::resolve_source_package("")
        .map_err(|error| format!("authored ground reference package: {error}"))?;
    validate(&source)?;
    install(&source, &target, false).map_err(|error| {
        format!(
            "prepare authored ground reference {} -> {}: {error}",
            source.display(),
            target.display(),
        )
    })
}

fn validate(path: &Path) -> Result<(), String> {
    dbpf::Package::open(path)
        .map(|_| ())
        .map_err(|error| format!("open authored ground reference {}: {error}", path.display()))
}

/// Publish a complete file. Linked lots must keep the same bytes as their donor.
pub(super) fn install(source: &Path, target: &Path, require_match: bool) -> io::Result<()> {
    let bytes = fs::read(source)?;
    let check_existing = || {
        if require_match && fs::read(target)? != bytes {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project reference differs from the selected lot source",
            ));
        }
        Ok(())
    };
    if target.is_file() {
        return check_existing();
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match crate::atomic_fs::write_atomic(target, &bytes, false) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && target.is_file() => {
            check_existing()
        }
        result => result,
    }
}
