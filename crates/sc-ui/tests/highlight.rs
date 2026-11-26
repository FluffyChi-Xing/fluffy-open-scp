use sc_ui::{
    DomRect, HighlightTone, UiHighlight, UiHighlightUpdate, set_ui_highlight, sync_ui_highlight,
};
use serde_json::{Value, json};

#[test]
fn highlights_a_control_and_reads_the_painted_bounds() {
    let result = set_ui_highlight(
        UiHighlight::Show {
            control_id: 0x8000_0001,
        },
        |script, packet, _| {
            assert!(script.contains("pointer-events:none"));
            assert_eq!(
                serde_json::from_str::<Value>(packet).unwrap(),
                json!({"action": "show", "controlId": 0x8000_0001_u32})
            );
            Ok(json!({
                "status": "success",
                "result": {
                    "rect": {"left": 0, "top": 3, "right": 22, "bottom": 18, "width": 22, "height": 15}
                }
            })
            .to_string())
        },
    )
    .unwrap();
    assert_eq!(
        result.rect,
        Some(DomRect {
            left: 0,
            top: 3,
            right: 22,
            bottom: 18,
            width: 22,
            height: 15,
        })
    );
}

#[test]
fn clearing_uses_an_explicit_command() {
    let result = set_ui_highlight(UiHighlight::Clear, |_, packet, _| {
        assert_eq!(
            serde_json::from_str::<Value>(packet).unwrap(),
            json!({"action": "clear"})
        );
        Ok(r#"{"status":"success","result":{"rect":null}}"#.into())
    })
    .unwrap();
    assert_eq!(result.rect, None);
}

#[test]
fn accepts_a_matching_reload_receipt_and_rejects_other_pages_or_generations() {
    let update = UiHighlightUpdate {
        page_uri: "game://localhost/resource/updater".into(),
        generation: 2,
        control_id: None,
        tone: HighlightTone::Amber,
    };
    let matched = sync_ui_highlight(&update, |_, packet, _| {
        assert_eq!(
            serde_json::from_str::<Value>(packet).unwrap(),
            json!({
                "action": "sync", "pageUri": update.page_uri, "generation": 2,
                "controlId": null, "tone": "amber"
            })
        );
        Ok(json!({
            "status": "success",
            "result": {"pageUri": update.page_uri, "generation": 2, "rect": null}
        })
        .to_string())
    })
    .unwrap();
    assert_eq!(matched.generation, 2);

    for (page, generation) in [(update.page_uri.as_str(), 1), ("game://other", 2)] {
        let mismatched = sync_ui_highlight(&update, |_, _, _| {
            Ok(json!({
                "status": "success",
                "result": {"pageUri": page, "generation": generation, "rect": null}
            })
            .to_string())
        });
        assert!(mismatched.is_err());
    }
}
