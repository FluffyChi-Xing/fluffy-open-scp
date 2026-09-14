//! 原子文件替换：写同目录临时文件 → `fsync` → 原子安装。
//!
//! Windows 上用 `MoveFileExW` + 短退避重试：索引器 / 杀软 / 同步盘常以不含
//! `FILE_SHARE_DELETE` 的句柄短暂持有目标文件，`ReplaceFileW` 会直接失败。
//!
//! 抽自 `workspace.rs` 原有实现，供 workspace / 导出 / overlay 写回共用，
//! 避免各处再出现「裸 `fs::write` 覆盖」这种崩溃即损坏的写法。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内自增，保证并发写的临时文件名互不冲突。
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 写临时文件并原子安装到 `target`。
///
/// `replace = false` 时目标若已存在则失败（等价于 create-new）。
pub(crate) fn write_atomic(target: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file has no name"))?
        .to_string_lossy()
        .into_owned();
    let sequence = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{name}.openscp-{}-{sequence}.tmp",
        std::process::id()
    ));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        install(&temporary, target, replace)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(windows)]
fn install(source: &Path, target: &Path, replace: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const RETRY_DELAYS_MS: [u64; 4] = [25, 50, 100, 200];
    let source: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let target: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let flags = if replace { MOVEFILE_REPLACE_EXISTING } else { 0 };
    for attempt in 0..=RETRY_DELAYS_MS.len() {
        let ok = unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), flags) };
        if ok != 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_SHARING_VIOLATION) || attempt == RETRY_DELAYS_MS.len() {
            return Err(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAYS_MS[attempt]));
    }
    unreachable!("retry loop always returns")
}

#[cfg(not(windows))]
fn install(source: &Path, target: &Path, replace: bool) -> io::Result<()> {
    if !replace && target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "target already exists",
        ));
    }
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openscp-atomic-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_existing_content() {
        let dir = temp_dir("replace");
        let target = dir.join("out.bin");
        fs::write(&target, b"old").unwrap();
        write_atomic(&target, b"new-bytes", true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new-bytes");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_when_missing() {
        let dir = temp_dir("create");
        let target = dir.join("out.bin");
        write_atomic(&target, b"first", true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_to_overwrite_without_replace() {
        let dir = temp_dir("no-replace");
        let target = dir.join("out.bin");
        write_atomic(&target, b"first", false).unwrap();
        assert!(write_atomic(&target, b"second", false).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"first");
        let _ = fs::remove_dir_all(&dir);
    }

    /// 性能报告（项目约定：每阶段测试附耗时）。
    /// 运行：`cargo test -p fluffy-open-scp atomic_fs -- --nocapture`
    #[test]
    fn perf_atomic_writes() {
        use std::time::Instant;

        let dir = temp_dir("perf");
        let target = dir.join("out.bin");
        let payload = vec![0x5Au8; 1024 * 1024];
        const RUNS: usize = 200;

        let mut worst = 0.0_f64;
        let start = Instant::now();
        for _ in 0..RUNS {
            let step = Instant::now();
            write_atomic(&target, &payload, true).unwrap();
            worst = worst.max(step.elapsed().as_secs_f64() * 1000.0);
        }
        let mean = start.elapsed().as_secs_f64() * 1000.0 / RUNS as f64;
        let leftovers = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        println!(
            "[perf atomic_fs] 1MB × {RUNS} 次覆盖写：均值 {mean:.2}ms / 最差 {worst:.2}ms；残留 temp = {leftovers}"
        );
        assert_eq!(leftovers, 0, "不得残留临时文件");
        assert_eq!(fs::read(&target).unwrap().len(), 1024 * 1024);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaves_no_temp_files_behind() {
        let dir = temp_dir("no-temp");
        let target = dir.join("out.bin");
        write_atomic(&target, b"a", true).unwrap();
        write_atomic(&target, b"b", true).unwrap();
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件：{leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
