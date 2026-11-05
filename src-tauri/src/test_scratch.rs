//! 测试脚手架共享工具（仅测试构建）。
//!
//! 沙箱/受限 shell 里 `$TMP` 可能是 MSYS POSIX 路径（`/tmp`），Windows
//! 文件 API 对它常返回 PermissionDenied（m1 tests 的 `temp_root` 早已因此
//! 加固）。测试临时目录统一优先落在工作区 `target/test-scratch`（cargo
//! 本就写 target），系统临时目录仅作兜底。

use std::{fs, path::PathBuf};

/// 每个测试模块可写的临时根目录（调用方自行拼唯一子目录名）。
pub fn scratch_base() -> PathBuf {
    let workspace_target =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-scratch");
    if fs::create_dir_all(&workspace_target).is_ok() {
        return workspace_target;
    }
    std::env::temp_dir()
}
