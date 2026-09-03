//! RenderWare4 (RW4) 资源解析 — SimCity 2013 模型 / 贴图 / 骨骼 / 动画。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `SimCityPak\RenderWare4\` 目录。
//!
//! 迁移范围：
//! - `RW4Model.Read`：section 树解析（`SectionTypeCodes`：Mesh=0x20009,
//!   Texture=0x20003, RW4Skeleton=0x7000c, Anim=0x70001, RW4Material=0x2000b …）
//! - 顶点格式与语义（POSITION/NORMAL/TEXCOORD FLOAT2/FLOAT4/BLENDINDICES…，
//!   注意 BLENDINDICES 存的是关节索引 ×3）
//! - 材质：`RW4Material` 的 `MaterialTextureReference` 槽位（u1=0 调色板、
//!   1 区域遮罩、2 法线、3 副遮罩；纹理常在独立资源中，类型 0x2f4e681b/0x2f4e681c）
//! - 贴图：DXT1/DXT5 块压缩解码、raw bitmap (pixFmt 21)、DDS 头写出
//! - 骨骼与动画：关键帧（LocRot/LocRotScale）、shell-rig 刚体蒙皮标记
//!   （TEXCOORD1 UBYTE4，(127,127,127,0)=静态）
//! - 坐标系：RW4 为 Z-up，导出 glTF 时需 -90° X 旋转
//!
//! 注意：C# 版依赖 XNA 的 Vector/Matrix（x86 only），Rust 端用纯数学实现即可。
