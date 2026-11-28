//! Independent scene reader and adapter. A failure here cannot invalidate LOTM.
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn initialize(root: &Path, source: &Path, work: &Path) -> Result<Value, String> {
    let script = work.join("extract_scene.py");
    fs::write(&script, include_str!("blender_scripts/extract_scene.py"))
        .map_err(|e| e.to_string())?;
    let output_path = work.join("scene.json");
    let output = crate::blender_service::read_scene_script(
        &script,
        &[
            crate::blender_service::path_arg(source),
            crate::blender_service::path_arg(&output_path),
        ],
    )?;
    if !output.status.success() {
        return Err(crate::blender_service::stderr_tail(&output, 12));
    }
    let raw: Value = serde_json::from_slice(&fs::read(output_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let bundle = crate::blend_native::dump::Bundle::parse(
        &fs::read(work.join("bundle.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let origin = crate::blend_native::export::bounds_origin(&bundle);
    let scene = adapt(&raw, origin, bundle.unit)?;
    let bytes = serde_json::to_vec(&scene).map_err(|e| e.to_string())?;
    let relative = format!("assets/scenes/{:x}.json", Sha256::digest(&bytes));
    let path = root.join(&relative);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    crate::atomic_fs::write_atomic(&path, &bytes, true).map_err(|e| e.to_string())?;
    Ok(json!({"asset": relative, "document": scene}))
}

fn number(v: &Value, key: &str, default: f64) -> f64 {
    v[key].as_f64().unwrap_or(default)
}
fn matrix(row: &Value, origin: [f64; 3], unit: f64) -> Result<Vec<f64>, String> {
    let m = row["matrix"].as_array().ok_or("missing matrix")?;
    if m.len() != 16 {
        return Err("invalid matrix length".into());
    }
    let m: Vec<f64> = m
        .iter()
        .map(|v| {
            v.as_f64()
                .filter(|n| n.is_finite())
                .ok_or("nonfinite matrix")
        })
        .collect::<Result<_, _>>()?;
    let mut out = vec![m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]];
    out.extend([
        (m[3] - origin[0]) * unit,
        (m[7] - origin[1]) * unit,
        (m[11] - origin[2]) * unit,
    ]);
    Ok(out)
}

pub fn adapt(raw: &Value, origin: [f64; 3], unit: f64) -> Result<Value, String> {
    if !unit.is_finite() || unit <= 0. {
        return Err("invalid scene unit".into());
    }
    let catalog: Value = serde_json::from_str(
        include_str!("blender_scripts/scene_catalog.json").trim_start_matches('\u{feff}'),
    )
    .map_err(|e| e.to_string())?;
    let mut units = Vec::new();
    let mut lot = Value::Null;
    let mut errors = raw["errors"].as_array().cloned().unwrap_or_default();
    for row in raw["objects"].as_array().ok_or("missing scene objects")? {
        let result = (|| -> Result<(), String> {
            let kind = row["kind"].as_str().ok_or("missing kind")?;
            let m = matrix(row, origin, unit)?;
            if kind == "lot" {
                if !lot.is_null() {
                    return Err("multiple lots are not supported by this scene".into());
                }
                lot = ground(row, &m, unit)?;
                return Ok(());
            }
            let index = units.iter().filter(|u: &&Value| u["kind"] == kind).count();
            let mut item = json!({"kind":kind,"index":index,"transform":{"matrix":m},"fields":[]});
            let settings = &row["settings"];
            match kind {
                "light" => {
                    // Blender lamp forward is -Z; the workbench spot axis is row 1.
                    let mut m = m;
                    let x = [m[0], m[1], m[2]];
                    let z = [m[6], m[7], m[8]];
                    let y = [m[3], m[4], m[5]];
                    let scale = (x.iter().map(|n| n * n).sum::<f64>()).sqrt();
                    if scale <= 0. {
                        return Err("zero light scale".into());
                    }
                    for axis in [&x, &y, &z] {
                        let length = axis.iter().map(|n| n * n).sum::<f64>().sqrt();
                        if (length - scale).abs() > 1e-4 {
                            return Err("nonuniform light scale".into());
                        }
                    }
                    m[..9].copy_from_slice(&[
                        x[0] / scale,
                        x[1] / scale,
                        x[2] / scale,
                        -z[0] / scale,
                        -z[1] / scale,
                        -z[2] / scale,
                        y[0] / scale,
                        y[1] / scale,
                        y[2] / scale,
                    ]);
                    item["transform"] = json!({"matrix":m});
                    item["lightType"] = json!(if row["lightType"] == "SPOT" {
                        "Spot"
                    } else {
                        "Point"
                    });
                    item["color"] = settings
                        .get("color")
                        .cloned()
                        .unwrap_or(json!([1., 1., 1.]));
                    for (target, source, default) in [
                        ("outerRadius", "radius", 8.),
                        ("innerRadius", "inner_radius", 0.),
                        ("length", "length", 12.),
                    ] {
                        item[target] = json!(number(settings, source, default) * unit * scale);
                    }
                    item["diffuse"] = json!(number(settings, "intensity", 1.));
                    item["specular"] = json!(number(settings, "specular", 0.5));
                    item["debugName"] = row["name"].clone();
                    item["cullDistance"] = json!("Far");
                    item["isVolumetric"] = json!(false);
                }
                "prop" => {
                    let mut m = m;
                    let scale = (m[..3].iter().map(|v| v * v).sum::<f64>()).sqrt();
                    if scale <= 0. {
                        return Err("zero prop scale".into());
                    }
                    for row in m[..9].chunks(3) {
                        if (row.iter().map(|v| v * v).sum::<f64>().sqrt() - scale).abs() > 1e-4 {
                            return Err("nonuniform prop scale".into());
                        }
                    }
                    for value in &mut m[..9] {
                        *value /= scale;
                    }
                    item["transform"] = json!({"matrix":m});
                    let preset = row["props"]["ny_prop_preset"]
                        .as_str()
                        .ok_or("missing prop preset")?;
                    let spec = &catalog["props"][preset];
                    let resource = spec["resourceId"]
                        .as_str()
                        .ok_or_else(|| format!("unknown prop preset: {preset}"))?;
                    item["resourceId"] = json!(
                        u32::from_str_radix(resource.trim_start_matches("0x"), 16)
                            .map_err(|e| e.to_string())?
                    );
                    item["bin"] = spec["preferredBin"].clone();
                    item["slot"] = Value::Null;
                    let mesh_unit = number(row, "meshUnit", 1.);
                    if mesh_unit <= 0. {
                        return Err("invalid prop mesh unit".into());
                    }
                    item["scale"] = json!(scale * unit / mesh_unit);
                    item["displayName"] = row["name"].clone();
                }
                "effect" => {
                    let preset = settings["preset"].as_str();
                    let name = row["name"].as_str().unwrap_or("");
                    // Stored enum ordinals aren't stable across addon releases. Resolve a
                    // string preset or its authored label, never guess an ordinal's ID.
                    let spec = preset
                        .and_then(|p| catalog["effects"].get(p))
                        .or_else(|| {
                            catalog["effects"]
                                .as_object()?
                                .values()
                                .filter(|v| {
                                    v["label"]
                                        .as_str()
                                        .is_some_and(|s| !s.is_empty() && name.contains(s))
                                })
                                .max_by_key(|v| v["label"].as_str().unwrap().len())
                        })
                        .ok_or("unresolved effect preset")?;
                    let id = u32::from_str_radix(
                        spec["effectId"]
                            .as_str()
                            .ok_or("missing effect id")?
                            .trim_start_matches("0x"),
                        16,
                    )
                    .map_err(|e| e.to_string())?;
                    item["effectId"] = json!({"typeId":0,"group":0,"instance":id});
                    item["enabled"] = json!(
                        settings["enabled"]
                            .as_bool()
                            .unwrap_or_else(|| settings["enabled"].as_i64().unwrap_or(1) != 0)
                    );
                    let size = number(settings, "size", 1.);
                    let m = item["transform"]["matrix"].as_array_mut().unwrap();
                    for v in &mut m[..9] {
                        *v = json!(v.as_f64().unwrap() * size);
                    }
                }
                _ => return Err(format!("unsupported scene kind: {kind}")),
            }
            units.push(item);
            Ok(())
        })();
        if let Err(detail) = result {
            errors.push(json!({"object":row["name"],"detail":detail}));
        }
    }
    Ok(json!({"format":"openscp.blend-scene/1","units":units,"lot":lot,"errors":errors}))
}

fn ground(row: &Value, m: &[f64], unit: f64) -> Result<Value, String> {
    let n = row["resolution"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 4096)
        .ok_or("invalid lot resolution")? as usize;
    let mask = row["mask"]
        .as_array()
        .filter(|v| v.len() == n * n)
        .ok_or("incomplete lot mask")?;
    let regions = row["regions"]
        .as_array()
        .filter(|v| v.len() == 5)
        .ok_or("lot must have five regions")?;
    let vertices = row["vertices"]
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or("lot must be rectangular")?;
    // Orthogonal world XY rectangles are resampled, including quarter-turns and mirrors.
    if m[2].abs() + m[5].abs() + m[6].abs() + m[7].abs() > 1e-5
        || (m[0].abs() > 1e-5 && m[1].abs() > 1e-5)
        || (m[3].abs() > 1e-5 && m[4].abs() > 1e-5)
    {
        return Err("lot tilt or nonorthogonal rotation".into());
    }
    let points: Vec<[f64; 2]> = vertices
        .iter()
        .map(|v| {
            Ok([
                v[0].as_f64().ok_or("invalid lot vertex")?,
                v[1].as_f64().ok_or("invalid lot vertex")?,
            ])
        })
        .collect::<Result<_, String>>()?;
    let bounds = |axis: usize| {
        (
            points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (xmin, xmax) = bounds(0);
    let (ymin, ymax) = bounds(1);
    let world: Vec<[f64; 2]> = points
        .iter()
        .map(|p| {
            [
                m[0] * p[0] * unit + m[3] * p[1] * unit + m[9],
                m[1] * p[0] * unit + m[4] * p[1] * unit + m[10],
            ]
        })
        .collect();
    let wb = |axis: usize| {
        (
            world.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min),
            world
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (lo_x, hi_x) = wb(0);
    let (lo_y, hi_y) = wb(1);
    let determinant = (m[0] * m[4] - m[1] * m[3]) * unit * unit;
    if determinant.abs() < 1e-9 || xmax <= xmin || ymax <= ymin {
        return Err("degenerate lot".into());
    }
    let mut labels = vec![0u8; n * n];
    for y in 0..n {
        for x in 0..n {
            let wx = lo_x + (x as f64 + 0.5) / n as f64 * (hi_x - lo_x) - m[9];
            let wy = lo_y + (y as f64 + 0.5) / n as f64 * (hi_y - lo_y) - m[10];
            let lx = (m[4] * unit * wx - m[3] * unit * wy) / determinant;
            let ly = (-m[1] * unit * wx + m[0] * unit * wy) / determinant;
            let ix = (((lx - xmin) / (xmax - xmin) * n as f64).floor() as isize)
                .clamp(0, n as isize - 1) as usize;
            let iy = (((ly - ymin) / (ymax - ymin) * n as f64).floor() as isize)
                .clamp(0, n as isize - 1) as usize;
            let label = mask[iy * n + ix]
                .as_u64()
                .filter(|v| *v <= 4)
                .ok_or("invalid lot mask label")? as usize;
            labels[y * n + x] = label as u8;
        }
    }
    let rgb = |r: &Value, key: &str| -> Vec<u8> {
        (0..3)
            .map(|i| {
                let linear = r[key][i].as_f64().unwrap_or(0.5).clamp(0., 1.);
                let srgb = if linear <= 0.0031308 {
                    linear * 12.92
                } else {
                    1.055 * linear.powf(1. / 2.4) - 0.055
                };
                (srgb * 255.).round() as u8
            })
            .collect()
    };
    let colors: Vec<Value> = regions[1..]
        .iter()
        .map(|r| {
            let c = rgb(r, "color");
            json!([c[0], c[1], c[2], number(r, "native_index", 0.)])
        })
        .collect();
    let borders: Vec<Value> = regions[1..]
        .iter()
        .map(|r| json!(rgb(r, "border_color")))
        .collect();
    let values = |key: &str| {
        regions[1..]
            .iter()
            .map(|r| number(r, key, 0.))
            .collect::<Vec<_>>()
    };
    let border_sizes: [f64; 4] = std::array::from_fn(|i| number(&regions[i + 1], "border", 0.15));
    let (rgba, widths) =
        crate::blend_scene_ground::selectors(&labels, n, [hi_x - lo_x, hi_y - lo_y], border_sizes);
    Ok(
        json!({"meta":{"format":"openscp.lot-ground","version":1,"name":row["name"],"size":[hi_x-lo_x,hi_y-lo_y],"width":n,"height":n,"metersPerPixel":(hi_x-lo_x)/n as f64,"baseTile":number(&regions[0],"native_index",0.) as u32,"tilePeriod":[number(&regions[0],"tile",8.),number(&regions[0],"tile",8.)],"colors":colors,"slots":[true,true,true,true],"borderColors":borders,"borderWidths":widths,"colorHeights":values("height"),"borderHeights":values("border_height"),"borderPatternIndices":values("border_index")},"mask":base64::engine::general_purpose::STANDARD.encode(rgba),"offset":[(lo_x+hi_x)/2.,(lo_y+hi_y)/2.]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broken_attachment_does_not_drop_valid_siblings() {
        let matrix = json!([1, 0, 0, 10, 0, 1, 0, 20, 0, 0, 1, 30, 0, 0, 0, 1]);
        let result=adapt(&json!({"objects":[
            {"kind":"prop","name":"bad","matrix":matrix,"props":{"ny_prop_preset":"missing"}},
            {"kind":"light","name":"lamp","matrix":matrix,"lightType":"SPOT","settings":{"radius":20,"specular":0.25}}
        ]}),[2.,3.,4.],2.).unwrap();
        assert_eq!(result["errors"].as_array().unwrap().len(), 1);
        assert_eq!(result["units"].as_array().unwrap().len(), 1);
        assert_eq!(result["units"][0]["transform"]["matrix"][9], 16.);
        assert_eq!(result["units"][0]["outerRadius"], 40.);
        assert_eq!(result["units"][0]["specular"], 0.25);
    }
    #[test]
    fn effect_enum_ordinal_is_never_guessed() {
        let result=adapt(&json!({"objects":[{"kind":"effect","name":"renamed","matrix":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],"settings":{"preset":0}}]}),[0.;3],1.).unwrap();
        assert_eq!(result["errors"].as_array().unwrap().len(), 1);
        assert!(result["units"].as_array().unwrap().is_empty());
    }
}
