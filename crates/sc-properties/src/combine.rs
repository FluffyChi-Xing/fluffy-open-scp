//! 资产聚合（对应 C# `CliRunner.RunExportPropCombined` / `ExportCombinedPropsToFolder`）。
//!
//! 规则（HANDOFF §4 已验证，勿扩展）：
//! 1. 同一 InstanceId 的属性表条目属于同一资产（模型 prop 与玩法 prop 共享实例）；
//! 2. 含 `Model Details`（hash `0x0975695F`）Key 属性的目录 prop 与其指向实例的
//!    属性表条目合并——这是唯一干净的资产身份链接；更多引用 hash 会产生跨家族
//!    过度合并（HANDOFF 已实证），刻意不加。

use std::collections::HashMap;

use dbpf::ResourceId;

use crate::{Kind, PropertyFile, Value};

pub const MODEL_DETAILS_HASH: u32 = 0x0975_695F;

/// One aggregated asset: the prop entries describing a single game asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetGroup {
    /// Prop entry TGIs, sorted by (type, group, instance).
    pub members: Vec<ResourceId>,
    /// Localized asset name if any member instance resolves in the locale map.
    pub name: Option<String>,
}

/// Aggregate prop entries into assets.
///
/// `names` maps instance id → localized name (see [`crate::collect_name_map`]);
/// `None` skips naming.
pub fn combine_assets<'a, I>(entries: I, names: Option<&HashMap<u32, String>>) -> Vec<AssetGroup>
where
    I: IntoIterator<Item = (ResourceId, &'a PropertyFile)>,
{
    let entries: Vec<(ResourceId, &PropertyFile)> = entries.into_iter().collect();

    // Union-find over entry indices.
    let mut parent: Vec<usize> = (0..entries.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // Rule 1: entries sharing an instance id.
    let mut by_instance: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, (id, _)) in entries.iter().enumerate() {
        by_instance.entry(id.instance).or_default().push(i);
    }
    for indices in by_instance.values() {
        for &j in indices.iter().skip(1) {
            union(&mut parent, indices[0], j);
        }
    }

    // Rule 2: Model Details references.
    for (i, (_, file)) in entries.iter().enumerate() {
        for prop in &file.values {
            if prop.hash != MODEL_DETAILS_HASH {
                continue;
            }
            for key in prop.keys() {
                if let Some(indices) = by_instance.get(&key.instance) {
                    for &j in indices {
                        union(&mut parent, i, j);
                    }
                }
            }
        }
    }

    // Collect components.
    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..entries.len() {
        let root = find(&mut parent, i);
        components.entry(root).or_default().push(i);
    }

    let mut groups: Vec<AssetGroup> = components
        .into_values()
        .map(|mut indices| {
            indices.sort_by_key(|&i| {
                let id = entries[i].0;
                (id.type_id, id.group, id.instance)
            });
            let members = indices.iter().map(|&i| entries[i].0).collect();
            let name = names.and_then(|names| {
                indices
                    .iter()
                    .filter_map(|&i| names.get(&entries[i].0.instance))
                    .next()
                    .cloned()
            });
            AssetGroup { members, name }
        })
        .collect();

    groups.sort_by(|a, b| {
        let ka = a.members[0];
        let kb = b.members[0];
        (ka.type_id, ka.group, ka.instance).cmp(&(kb.type_id, kb.group, kb.instance))
    });
    groups
}

/// Convenience: does a property file carry the Model Details link?
pub fn has_model_details(file: &PropertyFile) -> bool {
    file.values.iter().any(|p| {
        p.hash == MODEL_DETAILS_HASH
            && match &p.kind {
                Kind::Scalar(Value::Key(_)) => true,
                Kind::Array(vals) => vals.iter().any(|v| matches!(v, Value::Key(_))),
                _ => false,
            }
    })
}
