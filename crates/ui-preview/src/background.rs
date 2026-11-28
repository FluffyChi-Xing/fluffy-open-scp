use bytes::Bytes;
use image::{ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

const LIMIT: u64 = 10 * 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;
pub(crate) const TOKEN: &str = "token.txt";
pub(crate) const BACKGROUND: &str = "background.png";

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(crate) fn random_hex() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Create the credential consumed by the package builder and local server.
pub fn init(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let token = random_hex()?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(TOKEN))?
        .write_all(token.as_bytes())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", random_hex()?));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[derive(Clone)]
pub(crate) struct Asset {
    pub(crate) sha256: String,
    pub(crate) bytes: Bytes,
}

impl Asset {
    fn read(path: &Path) -> io::Result<Self> {
        let mut data = Vec::new();
        fs::File::open(path)?
            .take(LIMIT + 1)
            .read_to_end(&mut data)?;
        if data.is_empty() || data.len() as u64 > LIMIT {
            return Err(invalid("Expected a PNG of at most 10 MiB"));
        }
        let decoder = || ImageReader::with_format(Cursor::new(&data), ImageFormat::Png);
        let (width, height) = decoder()
            .into_dimensions()
            .map_err(|e| invalid(e.to_string()))?;
        if !(32..=MAX_DIMENSION).contains(&width) || !(32..=MAX_DIMENSION).contains(&height) {
            return Err(invalid("Unsupported PNG dimensions"));
        }
        let mut reader = decoder();
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().map_err(|e| invalid(e.to_string()))?;
        Ok(Self {
            sha256: format!("{:x}", Sha256::digest(&data)),
            bytes: Bytes::from(data),
        })
    }
}

/// Validate and publish the entire replacement as one atomic file change.
pub fn publish(directory: &Path, source: &Path) -> io::Result<String> {
    let picture = Asset::read(source)?;
    write_atomic(&directory.join(BACKGROUND), &picture.bytes)?;
    Ok(picture.sha256)
}

pub(crate) struct Background {
    path: PathBuf,
    stamp: Option<(SystemTime, u64)>,
    current: Option<Asset>,
}

impl Background {
    pub(crate) fn new(directory: &Path) -> Self {
        Self {
            path: directory.join(BACKGROUND),
            stamp: None,
            current: None,
        }
    }

    pub(crate) fn current(&mut self) -> Option<Asset> {
        if let Ok(stat) = fs::metadata(&self.path)
            && let Ok(modified) = stat.modified()
        {
            let stamp = Some((modified, stat.len()));
            if stamp != self.stamp {
                self.stamp = stamp;
                if let Ok(asset) = Asset::read(&self.path) {
                    self.current = Some(asset);
                }
            }
        } else {
            self.stamp = None;
        }
        self.current.clone()
    }
}
