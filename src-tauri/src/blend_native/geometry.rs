//! Polygon tessellation, clipping and tangent reconstruction.
//!
//! These are the geometric primitives used for body faces, window cells and
//! decals.  All functions are pure and operate on plain arrays.

pub type Point3 = [f64; 3];
pub type Uv2 = [f64; 2];
pub type Tri = [usize; 3];

/// Ear-clipping triangulation in the face plane.
pub fn tessellate(points: &[Point3], normal: Point3) -> Vec<Tri> {
    let count = points.len();
    if count < 3 {
        return Vec::new();
    }
    if count == 3 {
        return vec![[0, 1, 2]];
    }
    let axis = if normal[0].abs() >= normal[1].abs() && normal[0].abs() >= normal[2].abs() {
        0
    } else if normal[1].abs() >= normal[2].abs() {
        1
    } else {
        2
    };
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let flat: Vec<[f64; 2]> = points.iter().map(|point| [point[u], point[v]]).collect();
    let area: f64 = (0..count)
        .map(|index| {
            let a = flat[index];
            let b = flat[(index + 1) % count];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let mut indices: Vec<usize> = if area >= 0.0 {
        (0..count).collect()
    } else {
        (0..count).rev().collect()
    };
    let mut triangles = Vec::with_capacity(count - 2);
    let mut guard = 0;
    while indices.len() > 3 && guard < count * count {
        guard += 1;
        let length = indices.len();
        let mut clipped = false;
        for position in 0..length {
            let previous = indices[(position + length - 1) % length];
            let current = indices[position];
            let next = indices[(position + 1) % length];
            let a = flat[previous];
            let b = flat[current];
            let c = flat[next];
            let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            if cross <= 1e-14 {
                continue;
            }
            let contains = indices.iter().any(|candidate| {
                if *candidate == previous || *candidate == current || *candidate == next {
                    return false;
                }
                point_in_triangle(flat[*candidate], a, b, c)
            });
            if contains {
                continue;
            }
            triangles.push([previous, current, next]);
            indices.remove(position);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if indices.len() == 3 {
        triangles.push([indices[0], indices[1], indices[2]]);
    } else if indices.len() > 3 {
        // Ear clipping can stall on collinear/duplicate corners.  Tessellate
        // the remaining ring as a fan instead of dropping the rest of the face.
        for index in 1..indices.len() - 1 {
            triangles.push([indices[0], indices[index], indices[index + 1]]);
        }
    } else if triangles.is_empty() {
        for index in 1..count - 1 {
            triangles.push([0, index, index + 1]);
        }
    }
    triangles
}

fn point_in_triangle(point: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let d1 = (point[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (point[1] - b[1]);
    let d2 = (point[0] - c[0]) * (b[1] - c[1]) - (b[0] - c[0]) * (point[1] - c[1]);
    let d3 = (point[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (point[1] - a[1]);
    let has_negative = d1 < -1e-14 || d2 < -1e-14 || d3 < -1e-14;
    let has_positive = d1 > 1e-14 || d2 > 1e-14 || d3 > 1e-14;
    !(has_negative && has_positive)
}

/// Sutherland–Hodgman clip that interpolates both world position and UV.
pub fn clip_polygon(
    polygon: &[(Point3, Uv2)],
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
) -> Vec<(Point3, Uv2)> {
    let mut poly = polygon.to_vec();
    for (axis, edge, greater) in [
        (0usize, x0, true),
        (0, x1, false),
        (1, y0, true),
        (1, y1, false),
    ] {
        if poly.is_empty() {
            return poly;
        }
        let inside = |uv: Uv2| {
            if greater {
                uv[axis] >= edge - 1e-8
            } else {
                uv[axis] <= edge + 1e-8
            }
        };
        let mut result = Vec::with_capacity(poly.len() + 2);
        let mut previous = *poly.last().unwrap();
        let mut previous_inside = inside(previous.1);
        for current in poly {
            let current_inside = inside(current.1);
            if previous_inside != current_inside {
                let delta = current.1[axis] - previous.1[axis];
                let t = if delta.abs() > 1e-12 {
                    (edge - previous.1[axis]) / delta
                } else {
                    0.0
                };
                let position = [
                    previous.0[0] + (current.0[0] - previous.0[0]) * t,
                    previous.0[1] + (current.0[1] - previous.0[1]) * t,
                    previous.0[2] + (current.0[2] - previous.0[2]) * t,
                ];
                let uv = [
                    previous.1[0] + (current.1[0] - previous.1[0]) * t,
                    previous.1[1] + (current.1[1] - previous.1[1]) * t,
                ];
                result.push((position, uv));
            }
            if current_inside {
                result.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
        poly = result;
    }
    poly
}

/// Tangent + handedness from the first non-degenerate triangle.
pub fn tangent(points: &[Point3], uvs: &[Uv2], normal: Point3) -> (Point3, f64) {
    for k in 1..points.len() {
        let e1 = sub(points[k], points[0]);
        let d1 = [uvs[k][0] - uvs[0][0], uvs[k][1] - uvs[0][1]];
        for j in k + 1..points.len() {
            let e2 = sub(points[j], points[0]);
            let d2 = [uvs[j][0] - uvs[0][0], uvs[j][1] - uvs[0][1]];
            let det = d1[0] * d2[1] - d1[1] * d2[0];
            if det.abs() < 1e-10 {
                continue;
            }
            let mut value = [
                (e1[0] * d2[1] - e2[0] * d1[1]) / det,
                (e1[1] * d2[1] - e2[1] * d1[1]) / det,
                (e1[2] * d2[1] - e2[2] * d1[1]) / det,
            ];
            let projection = dot(value, normal);
            value = [
                value[0] - normal[0] * projection,
                value[1] - normal[1] * projection,
                value[2] - normal[2] * projection,
            ];
            if dot(value, value) < 1e-16 {
                continue;
            }
            let bitangent = [
                (e2[0] * d1[0] - e1[0] * d2[0]) / det,
                (e2[1] * d1[0] - e1[1] * d2[0]) / det,
                (e2[2] * d1[0] - e1[2] * d2[0]) / det,
            ];
            let normal_tangent = normalize(value).unwrap_or([1.0, 0.0, 0.0]);
            let hand = if dot(cross(normal, normal_tangent), bitangent) < 0.0 {
                -1.0
            } else {
                1.0
            };
            return (normal_tangent, hand);
        }
    }
    let axis = if normal[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    (
        normalize(cross(axis, normal)).unwrap_or([1.0, 0.0, 0.0]),
        1.0,
    )
}

fn add(a: Point3, b: Point3) -> Point3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Point3, b: Point3) -> Point3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Point3, b: Point3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Point3, b: Point3) -> Point3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(a: Point3) -> Option<Point3> {
    let length = dot(a, a).sqrt();
    (length > 1e-12).then(|| [a[0] / length, a[1] / length, a[2] / length])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_tessellates_into_two_triangles() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let triangles = tessellate(&points, [0.0, 0.0, 1.0]);
        assert_eq!(triangles.len(), 2);
    }

    #[test]
    fn clip_keeps_uv_and_position_in_sync() {
        let polygon = vec![
            ([0.0, 0.0, 0.0], [0.0, 0.0]),
            ([2.0, 0.0, 0.0], [2.0, 0.0]),
            ([2.0, 2.0, 0.0], [2.0, 2.0]),
            ([0.0, 2.0, 0.0], [0.0, 2.0]),
        ];
        let clipped = clip_polygon(&polygon, 0.5, 1.5, 0.5, 1.5);
        assert_eq!(clipped.len(), 4);
        for (position, uv) in clipped {
            assert!((position[0] - uv[0]).abs() < 1e-9);
            assert!((position[1] - uv[1]).abs() < 1e-9);
        }
    }

    #[test]
    fn tangent_is_orthogonal_to_normal() {
        let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let (tangent, hand) = tangent(&points, &uvs, [0.0, 0.0, 1.0]);
        assert!((dot(tangent, [0.0, 0.0, 1.0])).abs() < 1e-9);
        assert_eq!(hand, 1.0);
    }
}
