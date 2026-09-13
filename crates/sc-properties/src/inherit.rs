//! 属性继承（`Parent` = `0x00B2CCCB`）展平。
//!
//! 迁移自 C# `PropertyFile.GetParentProperties`：本级已有的键优先，父级只补
//! 本级缺失的键，逐级向上递归。大量资产的财产列表只存"差异"（如 lot 变体只覆盖
//! `LotMask` 与 LOD，而 `LotColors`/`Lot Textures`/`LotSize` 挂在父级），不展平
//! 就会读到空的地表授权。

use std::collections::HashSet;

use crate::model::{Kind, Value};
use crate::{Key, PropertyFile};

/// `Parent` 属性哈希。
pub const PARENT_HASH: u32 = 0x00B2_CCCB;

/// 继承链最大深度（防环 + 防病态数据）。
const MAX_DEPTH: usize = 64;

/// 沿 `Parent` 链展平属性：返回合并后的文件（本级键在前，祖先键按就近优先补入），
/// 并去掉 `Parent` 键本身。
///
/// `resolve` 接收一个父级 Key，返回其已解析的属性文件（找不到返回 `None`，
/// 此时按"继承到此为止"处理）。
pub fn flatten_parent_inheritance<F>(root: PropertyFile, mut resolve: F) -> PropertyFile
where
    F: FnMut(&Key) -> Option<PropertyFile>,
{
    let mut merged = PropertyFile::default();
    let mut seen: HashSet<u32> = HashSet::new();
    push_keys(&mut merged, &root);
    let mut current = parent_key(&root);
    let mut depth = 0usize;
    while let Some(key) = current {
        if depth >= MAX_DEPTH || !seen.insert(key.instance) {
            break;
        }
        let Some(parent) = resolve(&key) else { break };
        push_keys(&mut merged, &parent);
        current = parent_key(&parent);
        depth += 1;
    }
    merged.claimed_count = merged.values.len() as u32;
    merged
}

/// 展平，同时回报走过的链（供诊断/探针打印）。
pub fn flatten_parent_inheritance_traced<F>(
    root: PropertyFile,
    mut resolve: F,
) -> (PropertyFile, Vec<Key>)
where
    F: FnMut(&Key) -> Option<PropertyFile>,
{
    let mut merged = PropertyFile::default();
    let mut chain = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    push_keys(&mut merged, &root);
    let mut current = parent_key(&root);
    let mut depth = 0usize;
    while let Some(key) = current {
        if depth >= MAX_DEPTH || !seen.insert(key.instance) {
            break;
        }
        let Some(parent) = resolve(&key) else { break };
        push_keys(&mut merged, &parent);
        chain.push(key.clone());
        current = parent_key(&parent);
        depth += 1;
    }
    merged.claimed_count = merged.values.len() as u32;
    (merged, chain)
}

/// 把 `file` 中尚未出现过的键追加进去（跳过 `Parent`）。
fn push_keys(merged: &mut PropertyFile, file: &PropertyFile) {
    for property in file.values.iter().filter(|p| p.hash != PARENT_HASH) {
        if merged.values.iter().all(|m| m.hash != property.hash) {
            merged.values.push(property.clone());
        }
    }
}

/// 取本级 `Parent` 键（标量或单元素数组均可）。
fn parent_key(file: &PropertyFile) -> Option<Key> {
    let property = file.get(PARENT_HASH)?;
    let value = match &property.kind {
        Kind::Scalar(value) => value,
        Kind::Array(values) => values.first()?,
        Kind::Empty => return None,
    };
    match value {
        Value::Key(key) => Some(key.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PropType, Property, PropertyEncoding};

    fn scalar(hash: u32, value: Value) -> Property {
        Property {
            hash,
            prop_type: PropType::Float,
            kind: Kind::Scalar(value),
            encoding: PropertyEncoding::default(),
        }
    }

    fn parent(hash: u32, instance: u32) -> Property {
        scalar(
            hash,
            Value::Key(Key {
                instance,
                type_id: 0x00B1_B104,
                group: 0,
            }),
        )
    }

    fn file_of(values: Vec<Property>) -> PropertyFile {
        PropertyFile {
            values,
            claimed_count: 0,
        }
    }

    const H_CHILD: u32 = 0x1111_1111;
    const H_PARENT: u32 = 0x2222_2222;

    #[test]
    fn child_keys_win_and_parent_fills_gaps() {
        let root = file_of(vec![
            scalar(H_CHILD, Value::Float(1.0)),
            parent(PARENT_HASH, 0xAAAA),
        ]);
        let merged = flatten_parent_inheritance(root, |key| {
            assert_eq!(key.instance, 0xAAAA);
            Some(file_of(vec![
                scalar(H_CHILD, Value::Float(2.0)),
                scalar(H_PARENT, Value::Float(3.0)),
            ]))
        });
        assert_eq!(merged.get(H_CHILD).unwrap().scalar(), Some(&Value::Float(1.0)));
        assert_eq!(merged.get(H_PARENT).unwrap().scalar(), Some(&Value::Float(3.0)));
        assert!(merged.get(PARENT_HASH).is_none(), "Parent 键不应保留");
        assert_eq!(merged.values.len(), 2);
    }

    #[test]
    fn chain_is_walked_recursively() {
        let root = file_of(vec![parent(PARENT_HASH, 0xAAAA)]);
        let merged = flatten_parent_inheritance(root, |key| match key.instance {
            0xAAAA => Some(file_of(vec![
                scalar(H_CHILD, Value::Float(2.0)),
                parent(PARENT_HASH, 0xBBBB),
            ])),
            0xBBBB => Some(file_of(vec![scalar(H_PARENT, Value::Float(9.0))])),
            _ => None,
        });
        assert_eq!(merged.get(H_CHILD).unwrap().scalar(), Some(&Value::Float(2.0)));
        assert_eq!(merged.get(H_PARENT).unwrap().scalar(), Some(&Value::Float(9.0)));
    }

    #[test]
    fn cycle_terminates_without_duplicates() {
        let root = file_of(vec![parent(PARENT_HASH, 0xAAAA)]);
        let merged = flatten_parent_inheritance(root, |_| {
            Some(file_of(vec![
                scalar(H_CHILD, Value::Float(5.0)),
                parent(PARENT_HASH, 0xAAAA),
            ]))
        });
        assert_eq!(merged.values.len(), 1);
        assert_eq!(merged.get(H_CHILD).unwrap().scalar(), Some(&Value::Float(5.0)));
    }

    #[test]
    fn missing_parent_keeps_child_values() {
        let root = file_of(vec![
            scalar(H_CHILD, Value::Float(7.0)),
            parent(PARENT_HASH, 0xAAAA),
        ]);
        let merged = flatten_parent_inheritance(root, |_| None);
        assert_eq!(merged.get(H_CHILD).unwrap().scalar(), Some(&Value::Float(7.0)));
        assert!(merged.get(PARENT_HASH).is_none());
        assert_eq!(merged.values.len(), 1);
    }
}
