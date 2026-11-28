use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Policy {
    max_leaves: usize,
    incompatible: Vec<Vec<String>>,
}

fn load() -> Result<Policy, String> {
    serde_json::from_str(include_str!("../../../src/lib/asset-behavior-policy.json"))
        .map_err(|error| error.to_string())
}

fn leaves(nodes: &[Value]) -> Vec<(&str, &str)> {
    nodes
        .iter()
        .filter_map(|node| {
            let kind = node["type"].as_str()?;
            (kind != "utilities-core").then_some((kind, node["id"].as_str().unwrap_or(kind)))
        })
        .collect()
}

fn incompatible(policy: &Policy, first: &str, second: &str) -> bool {
    policy.incompatible.iter().any(|group| {
        group.split_first().is_some_and(|(head, rest)| {
            (head == first && rest.iter().any(|kind| kind == second))
                || (head == second && rest.iter().any(|kind| kind == first))
        })
    })
}

fn conflict(first: &str, second: &str, first_id: &str, second_id: &str) -> String {
    format!("BEHAVIOR_LEAF_CONFLICT: {first}/{second} ({first_id}, {second_id})")
}

fn check_limit(policy: &Policy, leaves: &[(&str, &str)]) -> Result<(), String> {
    if leaves.len() > policy.max_leaves {
        return Err(format!("BEHAVIOR_LEAF_LIMIT: {}", policy.max_leaves));
    }
    Ok(())
}

pub(super) fn needs_components(nodes: &[Value]) -> Result<bool, String> {
    let policy = load()?;
    let leaves = leaves(nodes);
    check_limit(&policy, &leaves)?;
    let mut separate = false;
    for (index, &(first, first_id)) in leaves.iter().enumerate() {
        for &(second, second_id) in &leaves[index + 1..] {
            if first == second {
                return Err(conflict(first, second, first_id, second_id));
            }
            separate |= incompatible(&policy, first, second);
        }
    }
    Ok(separate)
}

pub(super) fn validate(nodes: &[Value]) -> Result<(), String> {
    let policy = load()?;
    let leaves = leaves(nodes);
    check_limit(&policy, &leaves)?;
    for (index, &(first, first_id)) in leaves.iter().enumerate() {
        for &(second, second_id) in &leaves[index + 1..] {
            if first == second || incompatible(&policy, first, second) {
                return Err(conflict(first, second, first_id, second_id));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn utility_core_does_not_count_and_incompatible_leaves_fail() {
        let nodes = |kinds: &[&str]| {
            kinds
                .iter()
                .map(|kind| json!({"type":kind}))
                .collect::<Vec<_>>()
        };
        assert!(validate(&nodes(&["utilities-core", "hotel", "tree", "landvalue"])).is_ok());
        // Shared park/landvalue state conflicts; the production compiler
        // supports this composition by allocating independent components.
        assert!(validate(&nodes(&["park", "tourism", "landvalue", "office"])).is_err());
        assert!(needs_components(&nodes(&["park", "tourism", "landvalue", "office"])).unwrap());
        assert!(
            validate(&nodes(&["hotel", "tourism"]))
                .unwrap_err()
                .starts_with("BEHAVIOR_LEAF_CONFLICT")
        );
        assert!(validate(&nodes(&["tree", "tree"])).is_err());
        assert!(
            validate(&nodes(&["tree", "landvalue", "hotel", "park", "tourism"]))
                .unwrap_err()
                .starts_with("BEHAVIOR_LEAF_LIMIT")
        );
    }
}
