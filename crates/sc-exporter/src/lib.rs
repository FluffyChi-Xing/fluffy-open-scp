//! SimCity 资源导出器。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `SimCityPak\RenderWare4\Exporters\`
//! 与 `SimCityPak\Cli\CliRunner.cs` 中的各导出命令。
//!
//! 迁移范围（对应原 CLI 命令）：
//! - `export-prop`   → 可读 .txt/.json（含 combine 模式与本地化命名）
//! - `export-obj`    → Wavefront .obj（`WaveFrontOBJConverter`）
//! - `export-gltf`   → 二进制 glTF 2.0 .glb（`GltfConverter`）：几何 + 材质
//!   （基础色/法线/高光贴图解析、法线 R↔B swizzle、facade 亮度合成、
//!   骨骼蒙皮 + 动画采样）
//! - `export-texture`→ png/jpg/tga/dds（DXT 解码 / DDS 直写）
//! - `export-audio`  → Wwise Vorbis → wav（经外部 vgmstream，Tauri 层调度）
//! - `export-video`  → VP6 → mp4（经外部 ffmpeg，Tauri 层调度）

pub mod batch;
mod error;
pub mod gltf;
pub mod magic;
pub mod obj;
pub mod prop_json;
pub mod texture;
mod texture_index;

pub use batch::{BatchExport, ExportFailure, ExportProgress, export_batch, export_textures};
pub use error::{Error, Result};
pub use gltf::{
    AnimClip, AnimTrack, EmbeddedTextures, GlbOutput, SkinData, export_glb,
    export_glb_with_textures, extract_skin,
};
pub use magic::{MAGIC_FORMATS, MagicFormat, detect_magic};
pub use obj::{export_obj, export_obj_with_colors};
pub use texture::{TextureOutputFormat, export_texture};
pub use texture_index::{ResolvedTexture, TextureConflict, TextureIndex};
