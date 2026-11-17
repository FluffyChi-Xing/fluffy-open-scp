use crate::mod_flow::{digest, safe_path};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const LIMIT: usize = 256 * 1024 * 1024;
static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Snapshot {
    pub files: BTreeMap<PathBuf, Vec<u8>>,
    pub fingerprint: String,
}

fn fingerprint(files: &BTreeMap<PathBuf, Vec<u8>>) -> Result<String, String> {
    let hashes: Vec<_> = files
        .iter()
        .map(|(path, bytes)| (path, digest(bytes)))
        .collect();
    Ok(digest(
        &serde_json::to_vec(&hashes).map_err(|e| e.to_string())?,
    ))
}

fn read_folder(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
    budget: &mut usize,
    depth: usize,
) -> Result<(), String> {
    if depth > 8 {
        return Err("Save directory nesting is unsupported".into());
    }
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root)
        {
            return Err("Save entry resolves outside the selected directory".into());
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            read_folder(root, &path, files, budget, depth + 1)?;
        } else if kind.is_file() && files.len() < 2048 {
            let mut input = Vec::new();
            fs::File::open(&path)
                .map_err(|e| e.to_string())?
                .take(*budget as u64 + 1)
                .read_to_end(&mut input)
                .map_err(|e| e.to_string())?;
            *budget = budget
                .checked_sub(input.len())
                .ok_or("Save file budget exceeded")?;
            files.insert(
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_owned(),
                input,
            );
        } else {
            return Err("Save contains an unsupported entry or too many files".into());
        }
    }
    Ok(())
}

impl Snapshot {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        let mut budget = LIMIT;
        read_folder(&root, &root, &mut files, &mut budget, 0)?;
        if !files.contains_key(Path::new("misc").join("MetaData").as_path()) {
            return Err("Select the region save directory containing 0 and misc/MetaData".into());
        }
        Ok(Self {
            fingerprint: fingerprint(&files)?,
            files,
        })
    }

    pub fn latest(&self, site: &str) -> Option<(&PathBuf, &Vec<u8>)> {
        self.files
            .iter()
            .filter_map(|(path, bytes)| {
                if path.parent()? != Path::new(site) {
                    return None;
                }
                let name = path
                    .file_name()?
                    .to_str()?
                    .strip_prefix("state_file_")?
                    .strip_suffix(".egb")?;
                let (tick, time) = name.split_once('_')?;
                Some((
                    (time.parse::<u64>().ok()?, tick.parse::<u64>().ok()?),
                    path,
                    bytes,
                ))
            })
            .max_by_key(|(timestamp, ..)| *timestamp)
            .map(|(_, path, bytes)| (path, bytes))
    }

    pub fn export(&self, root: &Path) -> Result<(PathBuf, String), String> {
        let hash = fingerprint(&self.files)?;
        let folder = safe_path(root, &format!(".build/{hash}"))?;
        let output = safe_path(root, &format!(".build/{hash}/region-save"))?;
        if output.exists() {
            if Self::load(&output)?.fingerprint == hash {
                return Ok((output, hash));
            }
            return Err("Existing save export does not match its fingerprint".into());
        }
        fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let seq = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = safe_path(
            root,
            &format!(".build/{hash}/.save-{}-{seq}", std::process::id()),
        )?;
        fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let result = (|| -> Result<(), String> {
            for (relative, bytes) in &self.files {
                let path = staging.join(relative);
                fs::create_dir_all(path.parent().ok_or("Missing export parent")?)
                    .map_err(|e| e.to_string())?;
                crate::atomic_fs::write_atomic(&path, bytes, false).map_err(|e| e.to_string())?;
            }
            if Self::load(&staging)?.fingerprint != hash {
                return Err("Save export verification failed".into());
            }
            fs::rename(&staging, &output).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            // The directory was created by this invocation beneath the build root.
            let _ = fs::remove_dir_all(&staging);
        }
        result?;
        Ok((output, hash))
    }
}
