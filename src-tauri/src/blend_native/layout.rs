//! Window grid geometry derived from `SurfaceState` (port of `layout_values`).
//!
//! Pure: every value is a function of the facade size, the authored settings
//! and whether the decor style names a catalog entry.

use super::settings::SurfaceState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutValues {
    pub width: f64,
    pub height: f64,
    pub col_pitch: f64,
    pub group_pitch: f64,
    pub row_pitch: f64,
    pub nc: i32,
    pub ng: i32,
    pub nr: i32,
    pub start_x: f64,
    pub start_y: f64,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub panel_height: f64,
    pub crossbar: i32,
    pub frame_x: f64,
    pub frame_y: f64,
    pub preserve: i32,
    pub decor_cols: i32,
    pub decor_rows: i32,
    pub decor_inset: i32,
}

pub fn layout_values(
    width: f64,
    height: f64,
    state: &SurfaceState,
    decor_is_catalog_id: bool,
) -> LayoutValues {
    let nc = if state.layout == "COLUMN" {
        1
    } else {
        state.columns
    };
    let mut ng = if state.layout == "COLUMN" {
        1
    } else {
        state.groups
    };
    let mut nr = if state.layout == "ROW" { 1 } else { state.rows };
    let group_width = nc as f64 * state.width + (nc - 1) as f64 * state.col_gap;
    let fixed = state.vertical_mode == "FIXED";
    let window_height = if fixed {
        state.height.min(state.row_pitch - 0.02)
    } else {
        state.height
    };
    let pitch = if fixed {
        state.row_pitch
    } else {
        state.height + state.row_gap
    };
    if state.layout == "FILL" {
        ng = (((width - 2.0 * state.margin + state.group_gap) / (group_width + state.group_gap))
            .floor()) as i32;
        ng = ng.max(0);
        let rows = if fixed {
            (height - 2.0 * state.margin) / pitch
        } else {
            (height - 2.0 * state.margin + state.row_gap) / pitch
        };
        nr = (rows.floor() as i32).max(0);
    }
    let total_width = ng as f64 * group_width + (ng - 1).max(0) as f64 * state.group_gap;
    let total_height = if fixed {
        nr as f64 * pitch
    } else {
        nr as f64 * window_height + (nr - 1).max(0) as f64 * state.row_gap
    };
    let offset = if fixed {
        (pitch - window_height) / 2.0
    } else {
        0.0
    };
    let panel_height = if state.panel && state.decor_style != "NONE" {
        (state.panel_height.min(pitch - window_height - 0.02)).max(0.0)
    } else {
        0.0
    };
    LayoutValues {
        width: state.width,
        height: window_height,
        col_pitch: state.width + state.col_gap,
        group_pitch: group_width + state.group_gap,
        row_pitch: pitch,
        nc,
        ng,
        nr,
        start_x: (width - total_width) / 2.0 + state.shift_x,
        start_y: (height - total_height) / 2.0 + state.shift_y + offset,
        x_min: state.margin,
        x_max: width - state.margin,
        y_min: state.margin,
        y_max: height - state.margin,
        panel_height,
        crossbar: i32::from(state.style == "PANEL"),
        frame_x: state.frame_border.min(state.width * 0.24),
        frame_y: state.frame_border.min(window_height * 0.24),
        preserve: i32::from(state.preserve_frame),
        decor_cols: if matches!(state.decor_style.as_str(), "TWIN" | "BRICKS") {
            2
        } else {
            1
        },
        decor_rows: if state.decor_style == "BRICKS" { 2 } else { 1 },
        decor_inset: i32::from(
            matches!(state.decor_style.as_str(), "RECESSED" | "TWIN" | "BRICKS")
                || decor_is_catalog_id,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_layout_matches_plugin_math() {
        let state = SurfaceState::default();
        let values = layout_values(20.0, 30.0, &state, false);
        assert_eq!(values.nc, 2);
        assert_eq!(values.ng, 4);
        assert_eq!(values.nr, 7);
        assert!((values.group_pitch - 4.55).abs() < 1e-9);
        assert!((values.start_x - 1.45).abs() < 1e-9);
        assert!((values.start_y - 2.45).abs() < 1e-9);
        assert!((values.height - 2.3).abs() < 1e-9);
    }

    #[test]
    fn gap_layout_uses_height_plus_row_gap() {
        let mut state = SurfaceState::default();
        state.vertical_mode = "GAP".into();
        state.layout = "GRID".into();
        let values = layout_values(20.0, 30.0, &state, false);
        assert!((values.row_pitch - 3.35).abs() < 1e-9);
        assert_eq!(values.nr, 6);
    }

    #[test]
    fn decor_catalog_ids_enable_inset() {
        let mut state = SurfaceState::default();
        state.decor_style = "Commercial_decor_000".into();
        assert_eq!(layout_values(10.0, 10.0, &state, true).decor_inset, 1);
        assert_eq!(layout_values(10.0, 10.0, &state, false).decor_inset, 0);
    }
}
