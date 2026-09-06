//! RenderWare4 (RW4) 资源解析 — SimCity 2013 模型 / 贴图 / 骨骼 / 动画。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `SimCityPak\RenderWare4\` 目录。
//!
//! 当前实现（M3 第一阶段）：文件头 + 扁平 section 索引解析
//! （对齐 C# `RW4Header.Read` / `RW4Section.LoadHeader` / `SectionTypeCodes`）。
//! 真实格式没有 section 树——mesh/triangle/vertex 通过 **section 编号** 互相
//! 引用；Blob section 的 pos 以 section index 结束处为基准。
//!
//! 后续里程碑：VertexFormat/VertexArray/TriangleArray/Mesh（注意
//! BLENDINDICES 存的是关节索引 ×3）、材质槽位（u1=0..3，外部纹理
//! 0x2f4e681b/0x2f4e681c，不追 0x2001a）、DXT1/DXT5、Skeleton/Anim、
//! shell-rig（TEXCOORD1 UBYTE4，(127,127,127,0)=静态）。
//! 坐标系保持 Z-up，glTF 的 -90° X 旋转封装在导出层。
//!
//! # 用法
//!
//! ```no_run
//! let data = std::fs::read("model.rw4").unwrap();
//! let file = rw4::Rw4File::parse(&data).unwrap();
//! println!("{:?} with {} sections", file.file_type(), file.sections().len());
//! for section in file.sections() {
//!     println!("  #{number} {name:?} {size}B", number = section.number,
//!              name = section.type_name(), size = section.size);
//! }
//! let mesh = file.sections_of_type(rw4::SectionType::MESH).next().unwrap();
//! let bytes = file.payload(&data, mesh.number).unwrap();
//! ```

pub mod anim;
mod error;
mod header;
pub mod material;
pub mod math;
pub mod mesh;
mod model;
pub mod raster;
mod reader;
mod section;
mod skeleton;
pub mod texture;
mod vertex;

#[cfg(test)]
mod tests;

pub use anim::{
    COMPONENTS_BLEND_FACTOR, COMPONENTS_LOC_ROT, COMPONENTS_LOC_ROT_SCALE, Channel, DecodedAnim,
    Key,
};
pub use error::{Error, Result};
pub use material::{DecodedMaterial, MaterialSection, SHADER_DEF_MARKER, TextureSlotRef};
pub use math::{Mat4, mat4_decompose_trs, mat4_inverse, mat4_mul};
pub use mesh::{
    DecodedMesh, DecodedVertex, MeshHeader, NO_VERTEX_SECTION, TriangleArrayHeader,
    VertexArrayHeader,
};
pub use model::{FileType, Rw4File};
pub use raster::{RASTER_PIXEL_FORMAT_A8R8G8B8, RasterImage, unswizzle_simcity_normal};
pub use section::{Section, SectionType};
pub use skeleton::{DecodedSkeleton, Hierarchy, Joint};
pub use texture::{
    DecodedTexture, TEXTURE_TYPE_DXT1, TEXTURE_TYPE_DXT5, TEXTURE_TYPE_PALETTE_F32,
    TEXTURE_TYPE_RAW_BGRA, TextureFormat, decode_dxt1, decode_dxt5,
};
pub use vertex::{ComponentValue, DeclarationType, DeclarationUsage, VertexElement, VertexFormat};
