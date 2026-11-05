//! All output side effects live here. Each run publishes an immutable directory
//! only after readback; unsuccessful staging trees are removed by their owner.
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn project_file(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("source must be a project-relative file without traversal".into());
    }
    let path = root.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(root) || !path.is_file() {
        return Err("source escapes project root or is not a file".into());
    }
    Ok(path)
}

pub struct Staging {
    pub path: PathBuf,
    pub destination: PathBuf,
}

impl Staging {
    pub fn create(root: &Path, build_hash: &str) -> Result<Self, String> {
        let base = root.join(".build");
        match fs::create_dir(&base) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
        // Reject symlinks/junctions even if they happen to resolve inside root.
        if base.canonicalize().map_err(|e| e.to_string())? != base || !base.is_dir() {
            return Err(".build must be a real directory under project root".into());
        }
        let destination = base.join(build_hash);
        if destination.exists()
            && destination.canonicalize().map_err(|e| e.to_string())? != destination
        {
            return Err("build destination must not be a link".into());
        }
        for _ in 0..100 {
            let path = base.join(format!(
                ".pending-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path, destination }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("cannot allocate build staging directory".into())
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().ok_or("missing artifact parent")?)
            .map_err(|e| e.to_string())?;
        crate::atomic_fs::write_atomic(&path, bytes, false).map_err(|e| e.to_string())
    }

    pub fn publish(&self) -> Result<(), String> {
        if self.destination.exists() {
            // A deterministic repeated build may reuse a byte-identical tree
            // (run-evidence files excepted; they carry wall-clock data and are
            // refreshed from the latest run below).
            compare_tree(&self.path, &self.destination, "")?;
            return refresh_evidence(&self.path, &self.destination, "");
        }
        match fs::rename(&self.path, &self.destination) {
            Ok(()) => Ok(()),
            Err(_) if self.destination.exists() => {
                compare_tree(&self.path, &self.destination, "")?;
                refresh_evidence(&self.path, &self.destination, "")
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Files whose bytes legitimately vary between runs of the same build_hash
/// (timestamps/durations). They are excluded from the reuse comparison and
/// overwritten with the latest run's evidence on publish.
const EVIDENCE_FILES: [&str; 1] = ["reports/m1-build.json"];

fn compare_tree(staging: &Path, existing: &Path, prefix: &str) -> Result<(), String> {
    if existing.canonicalize().map_err(|e| e.to_string())? != existing {
        return Err("existing build contains a link".into());
    }
    for entry in fs::read_dir(staging).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let relative = if prefix.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{prefix}/{}", name.to_string_lossy())
        };
        let target = existing.join(&name);
        if entry.path().is_dir() {
            compare_tree(&entry.path(), &target, &relative)?;
        } else if EVIDENCE_FILES.contains(&relative.as_str()) {
            continue;
        } else if target.canonicalize().map_err(|e| e.to_string())? != target
            || fs::read(entry.path()).map_err(|e| e.to_string())?
                != fs::read(target).map_err(|e| e.to_string())?
        {
            return Err("existing build differs; refusing to overwrite".into());
        }
    }
    Ok(())
}

fn refresh_evidence(staging: &Path, existing: &Path, prefix: &str) -> Result<(), String> {
    for entry in fs::read_dir(staging).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let relative = if prefix.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{prefix}/{}", name.to_string_lossy())
        };
        if entry.path().is_dir() {
            refresh_evidence(&entry.path(), &existing.join(&name), &relative)?;
        } else if EVIDENCE_FILES.contains(&relative.as_str()) {
            let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
            crate::atomic_fs::write_atomic(&existing.join(&name), &bytes, true)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
