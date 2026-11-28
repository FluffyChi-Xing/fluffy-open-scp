//! Graphics inherited by resource-only children; existing direct paths stay first.
use super::*;

pub(super) fn resolve_inherited(
    packages: &[(u64, Arc<Package>)],
    resource_id: u32,
) -> Option<ResolvedPropModel> {
    fn walk(
        packages: &[(u64, Arc<Package>)],
        instance: u32,
        visited: &mut Vec<u32>,
    ) -> Option<ResolvedPropModel> {
        if visited.len() >= 32 || visited.contains(&instance) {
            return None;
        }
        visited.push(instance);
        for (_, file, _) in find_records(packages, instance) {
            let Some((parent, _, _)) = first_key(&file, KEY_PARENT) else {
                continue;
            };
            if visited.contains(&parent) {
                continue;
            }
            if let Some(model) =
                resolve_one(packages, parent).or_else(|| walk(packages, parent, visited))
            {
                return Some(model);
            }
        }
        None
    }
    let mut model = walk(packages, resource_id, &mut Vec::new())?;
    model.resource_id = resource_id;
    model.source = format!("inherited_{}", model.source);
    Some(model)
}
