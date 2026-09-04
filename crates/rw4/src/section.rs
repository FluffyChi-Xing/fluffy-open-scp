/// 已知 section 类型码（C# `SectionTypeCodes`）。
///
/// 保留为 `u32` 常量而非封闭 enum：真实资源中存在未收录类型，
/// 必须原样保留而不是解析失败。
pub struct SectionType;

impl SectionType {
    pub const TEXTURE: u32 = 0x20_003;
    pub const TRIANGLE_ARRAY: u32 = 0x20_007;
    pub const MESH: u32 = 0x20_009;
    pub const MATERIAL: u32 = 0x20_00B;
    pub const RW4_SKELETON: u32 = 0x70_00C;
    pub const HIERARCHY_INFO: u32 = 0x70_002;
    pub const MESH_MATERIAL_ASSIGNMENT: u32 = 0x20_01A;
    pub const BBOX: u32 = 0x80_005;
    pub const MODEL_HANDLES: u32 = 0xFF_0000;
    pub const ANIM: u32 = 0x70_001;
    pub const VERTEX_FORMAT: u32 = 0x20_004;
    pub const ANIMATIONS: u32 = 0xFF_0001;
    pub const MATRICES_4X4: u32 = 0x70_00B;
    pub const MATRICES_4X3: u32 = 0x70_00F;
    pub const VERTEX_ARRAY: u32 = 0x20_005;
    pub const COLLISION_MESH: u32 = 0x80_003;
    pub const BLOB: u32 = 0x10_030;

    /// C# 枚举名（诊断输出用）。
    pub fn name(code: u32) -> Option<&'static str> {
        Some(match code {
            Self::TEXTURE => "Texture",
            Self::TRIANGLE_ARRAY => "TriangleArray",
            Self::MESH => "Mesh",
            Self::MATERIAL => "Material",
            Self::RW4_SKELETON => "RW4Skeleton",
            Self::HIERARCHY_INFO => "HierarchyInfo",
            Self::MESH_MATERIAL_ASSIGNMENT => "MeshMaterialAssignment",
            Self::BBOX => "BBox",
            Self::MODEL_HANDLES => "ModelHandles",
            Self::ANIM => "Anim",
            Self::VERTEX_FORMAT => "VertexFormat",
            Self::ANIMATIONS => "Animations",
            Self::MATRICES_4X4 => "Matrices4x4",
            Self::MATRICES_4X3 => "Matrices4x3",
            Self::VERTEX_ARRAY => "VertexArray",
            Self::COLLISION_MESH => "CollisionMesh",
            Self::BLOB => "Blob",
            _ => return None,
        })
    }
}

/// 一条 section 索引记录（24 字节 entry + fixups）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// 在 section 索引中的序号（mesh/triangle 等用编号互相引用）。
    pub number: u32,
    /// payload 绝对偏移；Blob 已按 C# 语义重定位到 section index 之后。
    pub pos: u32,
    pub size: u32,
    pub alignment: u32,
    /// 类型表下标（C# `type_code_indirect`）。
    pub type_code_indirect: u32,
    /// 真实类型码。
    pub type_code: u32,
    /// section index 中记录的 fixup 偏移（加载期重定位地址）。
    pub fixups: Vec<u32>,
}

impl Section {
    pub fn type_name(&self) -> Option<&'static str> {
        SectionType::name(self.type_code)
    }

    pub fn is_blob(&self) -> bool {
        self.type_code == SectionType::BLOB
    }
}
