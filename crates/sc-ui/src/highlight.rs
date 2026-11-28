use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{DomRect, UiHostError, run_probe};

const PROBE: &str = include_str!("highlight_control.js");

/// Show or clear a passive outline in the active game document.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UiHighlight {
    Show { control_id: u32 },
    Clear,
}

/// Geometry measured in the active document at the time of painting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct HighlightResult {
    /// The painted bounds, or absent after clearing.
    pub rect: Option<DomRect>,
}

/// A fixed palette for previewing highlight styles without accepting CSS.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HighlightTone {
    Cyan,
    Amber,
}

/// Replace the outline in one specific page, or clear it when the ID is absent.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiHighlightUpdate {
    pub page_uri: String,
    pub generation: u32,
    pub control_id: Option<u32>,
    pub tone: HighlightTone,
}

/// Confirmation from the active document after applying an update.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightReceipt {
    pub page_uri: String,
    pub generation: u32,
    pub rect: Option<DomRect>,
}

/// Draw the selected visible control's outline without changing its input.
/// The caller supplies the same active-view evaluator as for tree inspection.
/// A highlight expires after 1.2 seconds of page timer time; a subsequent
/// request replaces it, and navigation discards the document's overlay.
pub fn set_ui_highlight(
    command: UiHighlight,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<HighlightResult, UiHostError> {
    run_probe(PROBE, command, evaluate)
}

/// Apply an ordered update and verify the page and generation in its receipt.
/// A document rejects generations at or below its last applied generation.
pub fn sync_ui_highlight(
    update: &UiHighlightUpdate,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<HighlightReceipt, UiHostError> {
    #[derive(Serialize)]
    struct Command<'a> {
        action: &'static str,
        #[serde(flatten)]
        update: &'a UiHighlightUpdate,
    }

    let receipt: HighlightReceipt = run_probe(
        PROBE,
        Command {
            action: "sync",
            update,
        },
        evaluate,
    )?;
    if receipt.page_uri != update.page_uri
        || receipt.generation != update.generation
        || receipt.rect.is_some() != update.control_id.is_some()
    {
        return Err(UiHostError::Probe(
            "Highlight receipt does not match the requested page, generation, or operation.".into(),
        ));
    }
    Ok(receipt)
}
