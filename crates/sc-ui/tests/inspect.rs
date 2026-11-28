use sc_ui::{InspectUiTreeError, InspectUiTreeRequest, inspect_ui_tree};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn sends_a_bounded_request_and_decodes_the_snapshot() {
    let request = InspectUiTreeRequest {
        control_id: Some(0x8000_0001),
        max_depth: u32::MAX,
        visible_only: false,
        max_nodes: 0,
    };

    let snapshot = inspect_ui_tree(request, |script, packet, timeout| {
        assert!(script.contains("FindControlByID"));
        assert_eq!(timeout, Duration::from_secs(8));
        assert_eq!(
            serde_json::from_str::<Value>(packet).unwrap(),
            json!({"controlId": 0x8000_0001_u32, "maxDepth": 8, "visibleOnly": false, "maxNodes": 1})
        );
        Ok(json!({
            "status": "success",
            "result": {
                "viewport": {"width": 1600, "height": 900},
                "rootControlId": 0x8000_0001_u32,
                "nodes": [{
                    "depth": 0, "controlId": 0x8000_0001_u32, "visible": true,
                    "left": 10, "width": 80, "blurBackground": false, "className": "",
                    "overflow": "", "text": "Options", "childCount": 1,
                    "rect": {"left": 20, "top": 30, "right": 180, "bottom": 80, "width": 160, "height": 50}
                }],
                "truncated": false,
                "depthLimited": true
            }
        })
        .to_string())
    })
    .unwrap();

    assert_eq!(snapshot.root_control_id, request.control_id);
    assert_eq!(snapshot.viewport.width, 1600);
    assert!(!snapshot.truncated);
    assert!(snapshot.depth_limited);
    let inspection = snapshot
        .inspection_snapshot(sc_ui::ui_inspect::ViewIdentity {
            build: "test-build".into(),
            page: "options".into(),
        })
        .unwrap();
    let selected = inspection
        .hit_candidates(sc_ui::ui_inspect::Point { x: 150.0, y: 40.0 })
        .next()
        .unwrap();
    let marker = inspection.mark(selected).unwrap();
    assert_eq!(marker.path, vec![request.control_id]);
    let persisted = serde_json::to_string(&marker).unwrap();
    let restored = serde_json::from_str(&persisted).unwrap();
    assert_eq!(inspection.resolve(&restored), Ok(selected));
}

#[test]
fn distinguishes_host_probe_and_response_errors() {
    let request = InspectUiTreeRequest::default();
    let host = inspect_ui_tree(request, |_, _, _| Err("view unavailable".into()));
    assert!(
        matches!(host, Err(InspectUiTreeError::Host(message)) if message == "view unavailable")
    );

    let probe = inspect_ui_tree(request, |_, _, _| {
        Ok(r#"{"status":"failure","error":"No root UI control is available."}"#.into())
    });
    assert!(
        matches!(probe, Err(InspectUiTreeError::Probe(message)) if message.contains("No root"))
    );

    let response = inspect_ui_tree(request, |_, _, _| Ok(r#"{"status":"success"}"#.into()));
    assert!(matches!(response, Err(InspectUiTreeError::Response(_))));
}
