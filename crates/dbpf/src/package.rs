use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lru::LruCache;
use memmap2::Mmap;

use crate::error::{Error, Result};
use crate::header::{self, Header};
use crate::index::{self, IndexEntry, ResourceId};
use crate::refpack;

/// A read-only view over a `.package` file.
///
/// The file is memory-mapped once at open time; resource reads are slices
/// into the mapping, so opening a large package only costs its index parse.
#[derive(Debug)]
pub struct Package {
    path: PathBuf,
    mmap: Mmap,
    header: Header,
    entries: Vec<IndexEntry>,
}

impl Package {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Package> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)?;
        // SAFETY: the mapping is used read-only for the lifetime of `Package`.
        // External truncation of the file while mapped could raise SIGBUS;
        // this is the accepted trade-off of the mmap design (roadmap P1).
        let mmap = unsafe { Mmap::map(&file)? };

        let header = header::parse(&mmap)?;
        let entries = index::parse(&mmap, &header)?;
        Ok(Self {
            path: path.to_path_buf(),
            mmap,
            header,
            entries,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    pub fn entry(&self, id: ResourceId) -> Option<&IndexEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The stored (on-disk) payload of an entry, without decompression.
    pub fn read_raw(&self, entry: &IndexEntry) -> Result<&[u8]> {
        let start = entry.offset;
        let len = entry.stored_len();
        let end = start.checked_add(len).ok_or(Error::OffsetOutOfRange {
            offset: start,
            size: self.mmap.len() as u64,
        })?;
        if end > self.mmap.len() as u64 {
            return Err(Error::OffsetOutOfRange {
                offset: end,
                size: self.mmap.len() as u64,
            });
        }
        Ok(&self.mmap[start as usize..end as usize])
    }

    /// The resource content: RefPack-decompressed when the entry is
    /// compressed, otherwise a copy of the stored bytes.
    pub fn read(&self, entry: &IndexEntry) -> Result<Vec<u8>> {
        let raw = self.read_raw(entry)?;
        if entry.compressed {
            refpack::decompress(raw, entry.decompressed_size as usize)
        } else {
            Ok(raw.to_vec())
        }
    }
}

/// A [`Package`] wrapper that caches decompressed resources keyed by TGI.
///
/// Duplicate TGI keys inside one package would alias to the first-decoded
/// payload; the C# implementation likewise treats TGI as resource identity.
pub struct CachedPackage {
    package: Arc<Package>,
    cache: Mutex<LruCache<ResourceId, Arc<[u8]>>>,
}

impl CachedPackage {
    pub fn new(package: Package, capacity: usize) -> CachedPackage {
        CachedPackage {
            package: Arc::new(package),
            cache: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(capacity.max(1)).unwrap(),
            )),
        }
    }

    pub fn package(&self) -> &Package {
        &self.package
    }

    /// Read (and decompress if needed) a resource, serving repeat reads from
    /// the LRU cache. Uncompressed resources are wrapped directly from the
    /// memory mapping without decoding.
    pub fn read(&self, entry: &IndexEntry) -> Result<Arc<[u8]>> {
        if !entry.compressed {
            return Ok(Arc::from(self.package.read_raw(entry)?));
        }

        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        if let Some(bytes) = cache.get(&entry.id) {
            return Ok(bytes.clone());
        }
        let bytes: Arc<[u8]> = self.package.read(entry)?.into();
        cache.put(entry.id, bytes.clone());
        Ok(bytes)
    }
}
