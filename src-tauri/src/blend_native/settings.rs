//! Per-face authoring state (`settings_json`) and the plug-in's defaults.
//!
//! `channels.defaults(p)` reads the RNA property defaults, not the current
//! scene values.  Those defaults are fixed by the plug-in class and are
//! reproduced here; `settings_json` overlays the values that were actually
//! authored for a material/layer.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceState {
    pub style: String,
    pub layout: String,
    pub groups: i32,
    pub columns: i32,
    pub rows: i32,
    pub width: f64,
    pub height: f64,
    pub col_gap: f64,
    pub group_gap: f64,
    pub row_gap: f64,
    pub margin: f64,
    pub shift_x: f64,
    pub shift_y: f64,
    pub offset: f64,
    pub panel: bool,
    pub panel_height: f64,
    pub normal: f64,
    pub roughness: f64,
    pub glow: f64,
    pub frame_color: [f32; 4],
    pub vertical_mode: String,
    pub row_pitch: f64,
    pub preserve_frame: bool,
    pub frame_border: f64,
    pub decor_style: String,
    pub panel_color: [f32; 4],
    pub panel_normal: f64,
    pub wall_texture: String,
    pub roof_texture: String,
    pub texture_scale: f64,
    pub texture_rotation: f64,
    pub texture_contrast: f64,
    pub texture_normal: f64,
    pub texture_offset_x: f64,
    pub texture_offset_y: f64,
    pub night_room_mode: String,
    pub night_ratio: f64,
    pub night_variation: f64,
    pub night_seed: i32,
    pub relief_strength: f64,
    pub base_uv_mode: String,
    pub base_metallic: f64,
    pub base_roughness: f64,
    pub base_role: String,
    pub palette_row: f64,
    pub wall_color: [f32; 4],
    pub roof_color: [f32; 4],
    pub wall_normal: f64,
    pub roof_angle: f64,
    pub roof_style: String,
    pub decal_enabled: bool,
    pub decal_style: String,
    pub decal_layout: String,
    pub decal_groups: i32,
    pub decal_columns: i32,
    pub decal_rows: i32,
    pub decal_width: f64,
    pub decal_height: f64,
    pub decal_col_gap: f64,
    pub decal_group_gap: f64,
    pub decal_row_gap: f64,
    pub decal_margin: f64,
    pub decal_shift_x: f64,
    pub decal_shift_y: f64,
    pub decal_color: [f32; 4],
    pub decal_normal: f64,
    /// `material_state.recover` outputs used by the recipe key:
    /// `_source_window_palette` is the outer engine control captured before
    /// the pattern override; `_source_body_palette(_enabled)` come from the
    /// resource-pattern group input default.
    #[serde(default, skip_serializing)]
    pub source_window_palette: Option<f64>,
    #[serde(default, skip_serializing)]
    pub source_body_palette: Option<f64>,
    #[serde(default, skip_serializing)]
    pub source_body_palette_enabled: Option<bool>,
}

impl Default for SurfaceState {
    fn default() -> Self {
        Self {
            style: "CLASSIC".into(),
            layout: "FILL".into(),
            groups: 3,
            columns: 2,
            rows: 6,
            width: 1.45,
            height: 2.3,
            col_gap: 0.55,
            group_gap: 1.1,
            row_gap: 1.05,
            margin: 0.5,
            shift_x: 0.0,
            shift_y: 0.0,
            offset: 0.025,
            panel: false,
            panel_height: 0.65,
            normal: 1.0,
            roughness: 0.35,
            glow: 0.5,
            frame_color: [0.8, 0.85, 0.87, 1.0],
            vertical_mode: "FIXED".into(),
            row_pitch: 3.8,
            preserve_frame: true,
            frame_border: 0.1,
            decor_style: "NONE".into(),
            panel_color: [0.82, 0.84, 0.81, 1.0],
            panel_normal: 1.0,
            wall_texture: "NONE".into(),
            roof_texture: "NONE".into(),
            texture_scale: 1.0,
            texture_rotation: 0.0,
            texture_contrast: 0.6,
            texture_normal: 1.0,
            texture_offset_x: 0.0,
            texture_offset_y: 0.0,
            night_room_mode: "GROUP".into(),
            night_ratio: 0.45,
            night_variation: 0.6,
            night_seed: 1,
            relief_strength: 0.15,
            base_uv_mode: "MATERIAL".into(),
            base_metallic: 0.0,
            base_roughness: 0.65,
            base_role: "WALL".into(),
            palette_row: -1.0,
            wall_color: [0.62, 0.57, 0.47, 1.0],
            roof_color: [0.13, 0.17, 0.18, 1.0],
            wall_normal: 0.5,
            roof_angle: 0.65,
            roof_style: "FLAT".into(),
            decal_enabled: false,
            decal_style: String::new(),
            decal_layout: "GRID".into(),
            decal_groups: 1,
            decal_columns: 1,
            decal_rows: 1,
            decal_width: 1.0,
            decal_height: 1.0,
            decal_col_gap: 0.3,
            decal_group_gap: 1.0,
            decal_row_gap: 0.5,
            decal_margin: 0.0,
            decal_shift_x: 0.0,
            decal_shift_y: 0.0,
            decal_color: [1.0, 1.0, 1.0, 1.0],
            decal_normal: 1.0,
            source_window_palette: None,
            source_body_palette: None,
            source_body_palette_enabled: None,
        }
    }
}

impl SurfaceState {
    /// Overlay a stored `settings_json` on the plug-in defaults.  This is the
    /// exact semantics of `channels.face_state` (`defaults(p)` + `update`):
    /// missing keys keep their class default (no old-document migration).
    pub fn overlay(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// Whether a stored settings object explicitly contains `key`.
    pub fn has_key(text: &str, key: &str) -> bool {
        serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| value.as_object().map(|map| map.contains_key(key)))
            .unwrap_or(false)
    }

    /// Parse a stored `settings_json` with the plug-in's restore defaults for
    /// older documents (channels.restore).  Editing uses this path; export
    /// uses [`SurfaceState::overlay`].
    pub fn restored(text: &str) -> Self {
        let mut value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let Some(map) = value.as_object_mut() else {
            return Self::default();
        };
        if !map.contains_key("vertical_mode") {
            let height = map.get("height").and_then(Value::as_f64).unwrap_or(2.3);
            let row_gap = map.get("row_gap").and_then(Value::as_f64).unwrap_or(1.05);
            map.insert("vertical_mode".into(), json!("GAP"));
            map.insert("row_pitch".into(), json!(height + row_gap));
            map.insert("preserve_frame".into(), json!(false));
        }
        if !map.contains_key("decor_style") {
            map.insert("decor_style".into(), json!("PLAIN"));
        }
        for (key, value) in [
            ("palette_row", json!(-1.0)),
            ("wall_texture", json!("NONE")),
            ("roof_texture", json!("NONE")),
            ("texture_scale", json!(1.0)),
            ("texture_rotation", json!(0.0)),
            ("texture_offset_x", json!(0.0)),
            ("texture_offset_y", json!(0.0)),
            ("texture_contrast", json!(0.6)),
            ("texture_normal", json!(1.0)),
        ] {
            map.entry(key.to_string()).or_insert(value);
        }
        serde_json::from_value(value).unwrap_or_default()
    }

    pub fn is_window(&self) -> bool {
        self.base_role == "WINDOW"
    }

    pub fn base_color(&self) -> [f32; 4] {
        if self.base_role == "ROOF" {
            self.roof_color
        } else {
            self.wall_color
        }
    }

    pub fn texture_for_role(&self) -> &str {
        if self.base_role == "ROOF" {
            &self.roof_texture
        } else {
            &self.wall_texture
        }
    }

    pub fn panel_active(&self) -> bool {
        self.panel && self.decor_style != "NONE"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_plugin_class() {
        let state = SurfaceState::default();
        assert_eq!(state.style, "CLASSIC");
        assert_eq!(state.layout, "FILL");
        assert_eq!(state.vertical_mode, "FIXED");
        assert_eq!(state.night_room_mode, "GROUP");
        assert_eq!(state.base_uv_mode, "MATERIAL");
        assert_eq!(state.base_role, "WALL");
        assert!((state.row_pitch - 3.8).abs() < 1e-12);
        assert_eq!(state.wall_color, [0.62, 0.57, 0.47, 1.0]);
    }

    #[test]
    fn legacy_settings_get_gap_layout_when_restored() {
        let state = SurfaceState::restored(r#"{"height":2.0,"row_gap":0.5}"#);
        assert_eq!(state.vertical_mode, "GAP");
        assert!((state.row_pitch - 2.5).abs() < 1e-12);
        assert!(!state.preserve_frame);
        assert_eq!(state.decor_style, "PLAIN");
        assert_eq!(state.palette_row, -1.0);
    }

    #[test]
    fn export_overlay_keeps_class_defaults() {
        let state = SurfaceState::overlay(r#"{"height":2.0,"row_gap":0.5}"#);
        assert_eq!(state.vertical_mode, "FIXED");
        assert!((state.row_pitch - 3.8).abs() < 1e-12);
        assert!(state.preserve_frame);
        assert_eq!(state.decor_style, "NONE");
    }

    #[test]
    fn authored_values_overlay_defaults() {
        let state = SurfaceState::overlay(
            r#"{"style":"TALL","width":1.2,"wall_color":[0.1,0.2,0.3,1.0]}"#,
        );
        assert_eq!(state.style, "TALL");
        assert!((state.width - 1.2).abs() < 1e-12);
        assert_eq!(state.wall_color, [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(state.columns, 2);
        assert!(SurfaceState::has_key(
            r#"{"base_uv_mode":"SURFACE"}"#,
            "base_uv_mode"
        ));
    }
}
