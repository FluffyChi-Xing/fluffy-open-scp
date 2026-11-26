//! SCRUI inspection for Rust hosts. The caller supplies an evaluator
//! bound to the game's active EAWebKit view and its owning UI thread.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::time::Duration;

mod highlight;
mod updater_test_button;
pub use highlight::{
    HighlightReceipt, HighlightResult, HighlightTone, UiHighlight, UiHighlightUpdate,
    set_ui_highlight, sync_ui_highlight,
};
pub use ui_inspect;
pub use updater_test_button::{
    TestButtonLocale, UpdaterTestButton, UpdaterTestButtonCommand, UpdaterTestButtonState,
    run_updater_test_button,
};

const PROBE: &str = include_str!("inspect_ui_tree.js");
const TIMEOUT: Duration = Duration::from_secs(8);

/// Controls the scope of a live SCRUI snapshot.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectUiTreeRequest {
    /// Root control ID, or the game UI manager's root when absent.
    pub control_id: Option<u32>,
    /// Maximum depth below the root, clamped to 8.
    pub max_depth: u32,
    /// Skip hidden controls and their descendants.
    pub visible_only: bool,
    /// Maximum number of returned nodes, clamped to 1–300.
    pub max_nodes: u32,
}

impl Default for InspectUiTreeRequest {
    fn default() -> Self {
        Self {
            control_id: None,
            max_depth: 3,
            visible_only: true,
            max_nodes: 120,
        }
    }
}

/// A pre-order snapshot of the visible view or one of its subtrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiTreeSnapshot {
    pub viewport: Viewport,
    pub root_control_id: Option<u32>,
    pub nodes: Vec<UiControl>,
    /// The node limit prevented traversal of a visible control.
    pub truncated: bool,
    /// The depth limit left visible descendants unvisited.
    pub depth_limited: bool,
}

impl UiTreeSnapshot {
    /// Convert SCRUI DOM geometry to a host-independent inspection snapshot.
    /// Use a page-root acquisition for markers intended to survive reopening.
    pub fn inspection_snapshot(
        &self,
        view: ui_inspect::ViewIdentity,
    ) -> Result<ui_inspect::Snapshot<u32>, ui_inspect::InspectError> {
        let viewport = ui_inspect::Rect {
            left: 0.0,
            top: 0.0,
            right: self.viewport.width.into(),
            bottom: self.viewport.height.into(),
        };
        let elements = self
            .nodes
            .iter()
            .map(|node| ui_inspect::Element {
                key: node.control_id,
                depth: node.depth,
                visible: node.visible,
                bounds: node.rect.map(|rect| ui_inspect::Rect {
                    left: rect.left.into(),
                    top: rect.top.into(),
                    right: rect.right.into(),
                    bottom: rect.bottom.into(),
                }),
            })
            .collect();
        ui_inspect::Snapshot::from_preorder(view, viewport, elements)
    }
}

/// EAWebKit viewport size in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// A SCRUI control. Logical geometry and the measured DOM rectangle are kept
/// separately because the UI can scale them differently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiControl {
    pub depth: u32,
    pub control_id: Option<u32>,
    pub parent_id: Option<u32>,
    pub visible: bool,
    pub left: Option<f64>,
    pub top: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub abs_x: Option<f64>,
    pub abs_y: Option<f64>,
    pub h_pin: Option<u32>,
    pub v_pin: Option<u32>,
    pub drawable_type: Option<u32>,
    pub style_list: Option<Vec<serde_json::Value>>,
    pub blur_background: bool,
    pub class_name: String,
    pub overflow: String,
    pub rect: Option<DomRect>,
    pub text: String,
    pub child_count: u32,
}

/// Measured DOM rectangle in CSS pixels, rounded by the probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Failure while running a bundled SCRUI operation.
#[derive(Debug, thiserror::Error)]
pub enum InspectUiTreeError {
    #[error("SCRUI host could not evaluate the probe: {0}")]
    Host(String),
    #[error("SCRUI probe rejected the request: {0}")]
    Probe(String),
    #[error("SCRUI probe returned invalid JSON or a mismatched response: {0}")]
    Response(#[source] serde_json::Error),
}

/// Shared error type for SCRUI inspection and visual highlighting.
pub type UiHostError = InspectUiTreeError;

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum ProbeReply<T> {
    Success { result: T },
    Failure { error: String },
}

/// Run the bundled read-only probe. The evaluator receives an ES5 function,
/// a single JSON string argument, and a timeout. It must return
/// the function's string result from the active game view.
pub fn inspect_ui_tree(
    request: InspectUiTreeRequest,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<UiTreeSnapshot, InspectUiTreeError> {
    let request = InspectUiTreeRequest {
        max_depth: request.max_depth.min(8),
        max_nodes: request.max_nodes.clamp(1, 300),
        ..request
    };
    run_probe(PROBE, request, evaluate)
}

fn run_probe<T: serde::de::DeserializeOwned>(
    script: &str,
    request: impl Serialize,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<T, UiHostError> {
    let packet = serde_json::to_string(&request).map_err(UiHostError::Response)?;
    let reply = evaluate(script, &packet, TIMEOUT).map_err(UiHostError::Host)?;
    match serde_json::from_str(&reply).map_err(UiHostError::Response)? {
        ProbeReply::Success { result } => Ok(result),
        ProbeReply::Failure { error } => Err(UiHostError::Probe(error)),
    }
}
