//! Structured tracing for the `.blend` import chain.
//!
//! One JSON object per line on stderr, with a stable prefix so the Tauri
//! console / external log capture can filter it:
//!
//! ```text
//! [openscp.blend-import] {"event":"native_ok","lotm_bytes":...}
//! ```
//!
//! Tracing is observational only: it never changes the import result.  Face-
//! or window-level events are opt-in (`OPENSCP_TRACE_GROUPS`) to avoid flooding.

use serde_json::{Map, Value};

pub fn enabled(event: &str) -> bool {
    match event {
        "native_group" => std::env::var("OPENSCP_TRACE_GROUPS").is_ok(),
        _ => true,
    }
}

pub fn emit(event: &str, fields: Value) {
    if !enabled(event) {
        return;
    }
    let mut object = Map::new();
    object.insert("event".into(), Value::String(event.to_string()));
    match fields {
        Value::Object(fields) => {
            for (key, value) in fields {
                object.insert(key, value);
            }
        }
        Value::Null => {}
        other => {
            object.insert("detail".into(), other);
        }
    }
    let line = serde_json::to_string(&object)
        .unwrap_or_else(|_| format!("{{\"event\":\"{event}\"}}"));
    eprintln!("[openscp.blend-import] {line}");
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_is_always_emitted_unless_opt_in() {
        assert!(enabled("import_start"));
        assert!(enabled("native_ok"));
        assert_eq!(
            enabled("native_group"),
            std::env::var("OPENSCP_TRACE_GROUPS").is_ok()
        );
    }
}
