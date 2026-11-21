//! Actor graphics references: cGraphicsInstanced::FillFromProps (0x7872F0).
use crate::vignette::Reference;
use crate::{Kind, PropertyFile, Value};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parts {
    pub bodies: Vec<i32>,
    pub bodies_max: Vec<i32>,
    pub heads: Vec<i32>,
    pub heads_max: Vec<i32>,
    pub outfits: Vec<i32>,
    pub outfits_max: Vec<i32>,
}
impl Parts {
    pub fn present(&self) -> bool {
        !self.bodies.is_empty() || !self.heads.is_empty()
    }
}
fn values(file: &PropertyFile, hash: u32) -> Vec<&Value> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Array(v) => v.iter().collect(),
            Kind::Scalar(v) => vec![v],
            Kind::Empty => vec![],
        })
        .collect()
}
pub fn keys(file: &PropertyFile, hash: u32) -> Vec<Reference> {
    values(file, hash)
        .iter()
        .filter_map(|v| {
            if let Value::Key(k) = v {
                Some(Reference {
                    type_id: k.type_id,
                    group_id: k.group,
                    instance_id: k.instance,
                })
            } else {
                None
            }
        })
        .collect()
}
pub fn parts(file: &PropertyFile) -> Parts {
    let col = |hash| {
        values(file, hash)
            .iter()
            .filter_map(|v| {
                if let Value::Int32(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect()
    };
    Parts {
        bodies: col(0x0CBD25C2),
        bodies_max: col(0x0CBD25C5),
        heads: col(0x0CBD25C1),
        heads_max: col(0x0CBD25C4),
        outfits: col(0x0CBD25C3),
        outfits_max: col(0x0CBD25C6),
    }
}
