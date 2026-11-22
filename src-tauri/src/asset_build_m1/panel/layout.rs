use serde_json::{Value, json};

pub(crate) const COMPILER_VERSION: &str = "panel-layout/2";

pub(crate) fn prepare_shell(shell: &mut Value) -> Result<(), String> {
    validate_children(shell)?;
    clean_zot(shell);
    validate_children(shell)
}

fn clean_zot(v: &mut serde_json::Value) {
    if v.pointer("/drawable/images")
        .and_then(|a| a.as_array())
        .is_some_and(|a| {
            a.iter().any(|p| {
                p.as_str()
                    .is_some_and(|p| p.ends_with("/rlvr-Zot_Header.png"))
            })
        })
    {
        v["visibility"] = json!(false);
        if let Some(node) = v.as_object_mut() {
            node.remove("drawable");
        }
    }
    // IndexMut inserts `children: null` on leaves. Retail DeserializeFromObject
    // checks field presence and immediately reads children.length, so that
    // apparently harmless insertion prevents the whole layout from loading.
    if let Some(a) = v
        .get_mut("children")
        .and_then(serde_json::Value::as_array_mut)
    {
        for c in a {
            clean_zot(c);
        }
    }
}

fn validate_children(v: &serde_json::Value) -> Result<(), String> {
    if let Some(children) = v.get("children") {
        let children = children
            .as_array()
            .ok_or_else(|| format!("PANEL_LAYOUT_CHILDREN_NOT_ARRAY:{}", v["controlID"]))?;
        for child in children {
            validate_children(child)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoration_cleanup_preserves_native_leaf_shape() {
        let mut shell = json!({"type":"cLayout", "children":[
            {"controlID":1, "drawable":{"images":["Images/rlvr-Zot_Header.png"]}},
            {"controlID":2, "children":[{"controlID":3, "visibility":true}]}
        ]});
        clean_zot(&mut shell);
        assert!(shell["children"][0].get("children").is_none());
        assert!(
            shell["children"][1]["children"][0]
                .get("children")
                .is_none()
        );
        assert_eq!(shell["children"][0]["visibility"], false);
        assert!(shell["children"][0].get("drawable").is_none());
        assert_eq!(shell["children"][1]["children"][0]["visibility"], true);
        validate_children(&shell).unwrap();
    }

    #[test]
    fn native_deserializer_requires_present_children_to_be_arrays() {
        assert!(
            validate_children(&json!({"children":[{"controlID":7,"children":null}]}))
                .unwrap_err()
                .starts_with("PANEL_LAYOUT_CHILDREN_NOT_ARRAY")
        );
        assert!(validate_children(&json!({"children":{}})).is_err());
        validate_children(&json!({"children":[]})).unwrap();
        validate_children(&json!({"controlID":7})).unwrap();
    }
}
