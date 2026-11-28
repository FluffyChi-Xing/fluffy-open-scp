use crate::{UiHostError, run_probe};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ui_inspect::Rect;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomPointRequest<'a> {
    pub page_uri: &'a str,
    /// Active EAWebKit viewport coordinates in CSS pixels.
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DomElement {
    pub tag: String,
    pub id: String,
    pub classes: String,
    pub display: String,
    pub position: String,
    pub bounds: Rect,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomPointSnapshot {
    pub page_uri: String,
    /// Hit element first, followed by its ancestors. Empty for no hit.
    pub path: Vec<DomElement>,
    pub truncated: bool,
}

/// Inspect the painted DOM at one point without changing focus or selection.
pub fn inspect_dom_point(
    request: DomPointRequest<'_>,
    evaluate: impl FnOnce(&str, &str, Duration) -> Result<String, String>,
) -> Result<DomPointSnapshot, UiHostError> {
    run_probe(include_str!("inspect_dom_point.js"), request, evaluate)
}
