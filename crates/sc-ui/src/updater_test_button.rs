use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{DomRect, UiHostError, run_probe};

const PROBE: &str = include_str!("updater_test_button.js");

/// Language of the updater test button.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TestButtonLocale {
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
}

/// Page-specific operation for the bundled test button.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum UpdaterTestButtonCommand {
    Apply {
        generation: u32,
        locale: TestButtonLocale,
    },
    Read,
    Clear {
        generation: u32,
    },
}

/// A command bound to an observed updater page URI.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterTestButton<'a> {
    pub page_uri: &'a str,
    #[serde(flatten)]
    pub command: UpdaterTestButtonCommand,
}

/// Page-side receipt for the fixed-position test button.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterTestButtonState {
    pub page_uri: String,
    pub generation: Option<u32>,
    pub mounted: bool,
    pub clicks: u32,
    pub rect: Option<DomRect>,
}

/// Run a fixed ES5 operation through the caller's active-view evaluator.
pub fn run_updater_test_button(
    request: UpdaterTestButton<'_>,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<UpdaterTestButtonState, UiHostError> {
    let state: UpdaterTestButtonState = run_probe(PROBE, request, evaluate)?;
    let operation_matched = match request.command {
        UpdaterTestButtonCommand::Apply { generation, .. } => {
            state.generation == Some(generation) && state.mounted && state.rect.is_some()
        }
        UpdaterTestButtonCommand::Read => true,
        UpdaterTestButtonCommand::Clear { generation } => {
            state.generation == Some(generation) && !state.mounted && state.rect.is_none()
        }
    };
    if state.page_uri != request.page_uri || !operation_matched {
        return Err(UiHostError::Probe(
            "Updater test button receipt does not match the requested page or operation.".into(),
        ));
    }
    Ok(state)
}
