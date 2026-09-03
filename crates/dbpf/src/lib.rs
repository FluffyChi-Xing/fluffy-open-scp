//! DBPF (Database Packed File) — SimCity 2013 `.package` 容器格式。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `SimCityPak.Packages` 库
//! （`DataBasePackedFile.cs` / `DataBaseIndex.cs` / `StreamHelpers.RefPackDecompress`）。
//!
//! 与 C# 版的关键差异：文件通过 mmap 常驻地址空间，资源读取为
//! `(offset, len)` 切片（消除 C# 每次重开文件的开销）；解压输出按索引中的
//! `decompressed_size` 预分配，循环内零重分配。
//!
//! # 用法
//!
//! ```no_run
//! use dbpf::{Package, CachedPackage};
//!
//! # fn main() -> dbpf::Result<()> {
//! let package = Package::open("data/SimCity_DLC0.package")?;
//! println!("{} resources", package.entries().len());
//!
//! let entry = package.entries()[0].clone();
//! let bytes = package.read(&entry)?;            // 惰性 RefPack 解压
//!
//! let cached = CachedPackage::new(package, 128); // 解压结果 LRU 缓存
//! let bytes2 = cached.read(&entry)?;
//! # Ok(())
//! # }
//! ```

mod error;
mod header;
mod index;
mod package;
mod reader;
mod refpack;

pub use error::{Error, Result};
pub use header::{Header, PackageKind};
pub use index::{IndexEntry, ResourceId};
pub use package::{CachedPackage, Package};
pub use refpack::{decompress as refpack_decompress, parse_stream_header as refpack_stream_header};
