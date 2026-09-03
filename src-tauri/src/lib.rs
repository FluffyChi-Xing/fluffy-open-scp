//! OpenSCP — SimCity (2013) package explorer（fluffy-open-scp）。
//!
//! Tauri 应用壳：将 `dbpf` / `rw4` / `sc-properties` / `sc-exporter` 各解析库
//! 以 command 形式暴露给 Vue 前端。原 WPF GUI（`MainWindow.xaml` 及各 View）
//! 的功能由前端 + commands 迁移实现。
//!
//! 外部工具调度（对应原 `CliRunner`）：
//! - 音频导出：bundled vgmstream（`Tools\vgmstream\`）
//! - 视频导出：ffmpeg（`Tools\ffmpeg\` 或 PATH）

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! OpenSCP Tauri backend is running.")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
