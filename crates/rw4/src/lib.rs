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

mod error;
mod header;
pub mod mesh;
mod model;
mod reader;
mod section;
mod vertex;

#[cfg(test)]
mod tests;

pub use error::{Error, Result};
pub use mesh::{
    DecodedMesh, DecodedVertex, MeshHeader, NO_VERTEX_SECTION, TriangleArrayHeader,
    VertexArrayHeader,
};
pub use model::{FileType, Rw4File};
pub use section::{Section, SectionType};
pub use vertex::{ComponentValue, DeclarationType, DeclarationUsage, VertexElement, VertexFormat};
