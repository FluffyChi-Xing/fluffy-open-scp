//! Decal projection/layout (port of `decals.frame`/`coordinates`/`cells`).

use super::dump::BundleObject;
use super::settings::SurfaceState;

#[derive(Debug, Clone)]
pub struct DecalFrame {
    pub uv: Vec<[f64; 2]>,
    pub width: f64,
    pub height: f64,
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(a: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(a, a).sqrt();
    (length > 1e-12).then(|| scale(a, 1.0 / length))
}

fn matrix_point(matrix: &[f64], point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0] * point[0] + matrix[1] * point[1] + matrix[2] * point[2] + matrix[3],
        matrix[4] * point[0] + matrix[5] * point[1] + matrix[6] * point[2] + matrix[7],
        matrix[8] * point[0] + matrix[9] * point[1] + matrix[10] * point[2] + matrix[11],
    ]
}

/// Inverse-transposed 3×3 (cofactor / determinant).
fn normal_matrix(matrix: &[f64]) -> Option<[[f64; 3]; 3]> {
    let a = [matrix[0], matrix[1], matrix[2]];
    let b = [matrix[4], matrix[5], matrix[6]];
    let c = [matrix[8], matrix[9], matrix[10]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return None;
    }
    let cofactor = [
        cross(b, c),
        cross(c, a),
        cross(a, b),
    ];
    let inv = 1.0 / det;
    // Normals transform with the inverse transpose: n' = (M^-1)^T n.  The
    // adjugate rows above are det*M^-1, so read them row-wise (not column-wise,
    // which would apply M^-1 itself and mis-rotate normals on rotated objects).
    Some([
        [cofactor[0][0] * inv, cofactor[0][1] * inv, cofactor[0][2] * inv],
        [cofactor[1][0] * inv, cofactor[1][1] * inv, cofactor[1][2] * inv],
        [cofactor[2][0] * inv, cofactor[2][1] * inv, cofactor[2][2] * inv],
    ])
}

pub fn frame(object: &BundleObject, face_index: usize) -> Result<DecalFrame, String> {
    let mesh = object.mesh.as_ref().ok_or("object has no mesh")?;
    let polygon = mesh
        .polygons
        .get(face_index)
        .ok_or_else(|| format!("face {face_index} out of range"))?;
    let normal_matrix = normal_matrix(&object.matrix).ok_or("degenerate object matrix")?;
    let normal = normalize([
        normal_matrix[0][0] * polygon.normal[0]
            + normal_matrix[0][1] * polygon.normal[1]
            + normal_matrix[0][2] * polygon.normal[2],
        normal_matrix[1][0] * polygon.normal[0]
            + normal_matrix[1][1] * polygon.normal[1]
            + normal_matrix[1][2] * polygon.normal[2],
        normal_matrix[2][0] * polygon.normal[0]
            + normal_matrix[2][1] * polygon.normal[1]
            + normal_matrix[2][2] * polygon.normal[2],
    ])
    .ok_or("degenerate face normal")?;
    let up = if normal[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let right = normalize(cross(up, normal)).ok_or("degenerate decal frame")?;
    let up = normalize(cross(normal, right)).ok_or("degenerate decal frame")?;
    let points: Vec<[f64; 3]> = polygon
        .vertices
        .iter()
        .map(|index| matrix_point(&object.matrix, mesh.vertices[*index as usize]))
        .collect();
    let origin = points[0];
    for point in &points {
        if dot(sub(*point, origin), normal).abs() > 0.01 {
            return Err("装饰贴花需要平面，请先把弯曲面拆成平面".into());
        }
    }
    let mut uv: Vec<[f64; 2]> = points
        .iter()
        .map(|point| {
            let delta = sub(*point, origin);
            [dot(delta, right), dot(delta, up)]
        })
        .collect();
    let min_x = uv.iter().map(|value| value[0]).fold(f64::INFINITY, f64::min);
    let min_y = uv.iter().map(|value| value[1]).fold(f64::INFINITY, f64::min);
    for value in &mut uv {
        value[0] -= min_x;
        value[1] -= min_y;
    }
    let width = uv.iter().map(|value| value[0]).fold(0.0, f64::max);
    let height = uv.iter().map(|value| value[1]).fold(0.0, f64::max);
    Ok(DecalFrame { uv, width, height })
}

pub fn coordinates(object: &BundleObject, face_index: usize) -> Result<Vec<[f64; 2]>, String> {
    let mesh = object.mesh.as_ref().ok_or("object has no mesh")?;
    let polygon = mesh
        .polygons
        .get(face_index)
        .ok_or_else(|| format!("face {face_index} out of range"))?;
    let schema = mesh
        .face_attributes
        .get("NY_Decal_schema")
        .and_then(|values| values.get(face_index))
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    if schema >= 2.0 {
        if let Some(coords) = mesh.corner_attributes.get("NY_DecalCoord") {
            let mut result = Vec::with_capacity(polygon.loops.len());
            for loop_index in &polygon.loops {
                let value = coords
                    .get(*loop_index as usize)
                    .ok_or("decal coordinate out of range")?;
                result.push([value[0], value[1]]);
            }
            return Ok(result);
        }
    }
    if let Some(uvs) = mesh.uv_layers.get("NY_DecalUV") {
        let mut result = Vec::with_capacity(polygon.loops.len());
        for loop_index in &polygon.loops {
            result.push(
                *uvs.get(*loop_index as usize)
                    .ok_or("decal UV out of range")?,
            );
        }
        return Ok(result);
    }
    Ok(frame(object, face_index)?.uv)
}

/// `(x, y, width, height)` cells that fit inside the authored margins.
pub fn cells(state: &SurfaceState, width: f64, height: f64) -> Result<Vec<[f64; 4]>, String> {
    let mut nc = state.decal_columns;
    let mut ng = state.decal_groups;
    let mut nr = state.decal_rows;
    if state.decal_layout == "COLUMN" {
        ng = 1;
        nc = 1;
    }
    if state.decal_layout == "ROW" {
        nr = 1;
    }
    let group_width =
        nc as f64 * state.decal_width + (nc - 1) as f64 * state.decal_col_gap;
    let group_pitch = group_width + state.decal_group_gap;
    let row_pitch = state.decal_height + state.decal_row_gap;
    if state.decal_layout == "FILL" {
        ng = ((width - 2.0 * state.decal_margin + state.decal_group_gap) / group_pitch)
            .floor()
            .max(0.0) as i32;
        nr = ((height - 2.0 * state.decal_margin + state.decal_row_gap) / row_pitch)
            .floor()
            .max(0.0) as i32;
    }
    let start_x = (width
        - (ng as f64 * group_width + (ng - 1).max(0) as f64 * state.decal_group_gap))
        / 2.0
        + state.decal_shift_x;
    let start_y = (height
        - (nr as f64 * state.decal_height + (nr - 1).max(0) as f64 * state.decal_row_gap))
        / 2.0
        + state.decal_shift_y;
    if ng * nc * nr > 20_000 {
        return Err("单面贴花超过 20000 个，请增大尺寸或间距".into());
    }
    let mut result = Vec::new();
    for row in 0..nr {
        let y = start_y + row as f64 * row_pitch;
        if y < state.decal_margin - 1e-5
            || y + state.decal_height > height - state.decal_margin + 1e-5
        {
            continue;
        }
        for group in 0..ng {
            for column in 0..nc {
                let x = start_x
                    + group as f64 * group_pitch
                    + column as f64 * (state.decal_width + state.decal_col_gap);
                if x >= state.decal_margin - 1e-5
                    && x + state.decal_width <= width - state.decal_margin + 1e-5
                {
                    result.push([
                        x,
                        y,
                        state.decal_width,
                        state.decal_height,
                    ]);
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_native::dump::{BundleMesh, BundlePolygon};

    fn rotated_quad() -> BundleObject {
        // tall_office's "051": a scale+axis-permutation matrix.  Normals need
        // (M^-1)^T here; using M^-1 instead rotates them onto the wrong axis and
        // the planarity test rejects every decal face on the building.
        BundleObject {
            name: "051".into(),
            props: Default::default(),
            layers: Vec::new(),
            matrix: vec![
                0.0, 0.0, 0.0103554, 217.01507568,
                0.0103554, 0.0, 0.0, 0.0,
                0.0, 0.0103554, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            mesh: Some(BundleMesh {
                vertices: vec![
                    [3347.52099609, 3279.53588867, 614.51556396],
                    [3347.52075195, 3279.53588867, 1063.65026855],
                    [3796.6550293, 3279.53588867, 1063.65026855],
                    [3796.6550293, 3279.53588867, 614.51556396],
                ],
                polygons: vec![BundlePolygon {
                    vertices: vec![0, 1, 2, 3],
                    loops: vec![0, 1, 2, 3],
                    normal: [0.0, 1.0, 0.0],
                    material: 0,
                    area: 0.0,
                }],
                uv_layers: Default::default(),
                face_attributes: Default::default(),
                corner_attributes: Default::default(),
                materials: vec![None],
            }),
        }
    }

    #[test]
    fn frame_transforms_normals_with_inverse_transpose() {
        let object = rotated_quad();
        let frame = frame(&object, 0).expect("rotated planar face must project");
        assert!((frame.width - 4.65097).abs() < 1e-3, "width {}", frame.width);
        assert!((frame.height - 4.65097).abs() < 1e-3, "height {}", frame.height);
    }

    #[test]
    fn frame_rejects_non_planar_faces() {
        let mut object = rotated_quad();
        // Local Y is the face normal axis under this matrix, so this bends the
        // quad by 0.41 m in world space.
        object.mesh.as_mut().unwrap().vertices[1][1] += 40.0;
        let error = frame(&object, 0).unwrap_err();
        assert!(error.contains("平面"), "{error}");
    }
}
