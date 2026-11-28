use sc_ui::{
    TestButtonLocale, UpdaterTestButton, UpdaterTestButtonCommand, run_updater_test_button,
};
use serde_json::{Value, json};

#[test]
fn verifies_the_fixed_button_apply_receipt() {
    let request = UpdaterTestButton {
        page_uri: "game://test/updater",
        command: UpdaterTestButtonCommand::Apply {
            generation: 4,
            locale: TestButtonLocale::ZhTw,
        },
    };
    let mut receipt = json!({
        "pageUri": request.page_uri,
        "generation": 4,
        "mounted": true,
        "clicks": 3,
        "rect": {"left": 294, "top": 543, "width": 132, "height": 28, "right": 426, "bottom": 571}
    });
    let state = run_updater_test_button(request, |script, args, _| {
        assert!(script.contains("position:fixed"));
        assert_eq!(
            serde_json::from_str::<Value>(args).unwrap(),
            json!({"pageUri": request.page_uri, "action": "apply", "generation": 4, "locale": "zh-TW"})
        );
        Ok(json!({"status": "success", "result": receipt}).to_string())
    })
    .unwrap();
    assert_eq!(state.rect.unwrap().left, 294);
    assert_eq!(state.clicks, 3);

    receipt["generation"] = json!(3);
    assert!(
        run_updater_test_button(request, |_, _, _| {
            Ok(json!({"status": "success", "result": receipt}).to_string())
        })
        .is_err()
    );
}
