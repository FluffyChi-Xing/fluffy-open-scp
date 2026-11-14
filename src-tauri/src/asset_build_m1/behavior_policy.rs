use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Policy {
    max_leaves: usize,
    incompatible: Vec<Vec<String>>,
}

pub(super) fn validate(nodes: &[Value]) -> Result<(), String> {
    let policy: Policy =
        serde_json::from_str(include_str!("../../../src/lib/asset-behavior-policy.json"))
            .map_err(|error| error.to_string())?;
    let leaves: Vec<_> = nodes
        .iter()
        .filter_map(|node| {
            let kind = node["type"].as_str()?;
            (kind != "utilities-core").then_some(kind)
        })
        .collect();
    if leaves.len() > policy.max_leaves {
        return Err(format!("BEHAVIOR_LEAF_LIMIT: {}", policy.max_leaves));
    }
    for (index, &first) in leaves.iter().enumerate() {
        for &second in &leaves[index + 1..] {
            if first == second
                || policy.incompatible.iter().any(|group| {
                    group.split_first().is_some_and(|(head, rest)| {
                        (head == first && rest.iter().any(|kind| kind == second))
                            || (head == second && rest.iter().any(|kind| kind == first))
                    })
                })
            {
                return Err(format!("BEHAVIOR_LEAF_CONFLICT: {first}/{second}"));
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
        assert!(validate(&nodes(&["park", "tourism", "landvalue", "office"])).is_ok());
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
