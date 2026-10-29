//! Read-only diagnostics for asset-mod samples.
//!
//! This module deliberately stops before the asset workbench and before any
//! package writer. It turns an existing project directory into a deterministic
//! fixture report that can be used to decide whether an M1 schema is possible.

use dbpf::{Package, ResourceId};
use rw4::RasterImage;
use sc_properties::{Key, PropertyFile, Value as PropertyValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleInspection {
    pub project: ProjectSummary,
    pub files: Vec<FileSummary>,
    pub diagnostics: Vec<SampleDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<RasterDependencySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceDependencySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceDependencySummary {
    pub requested_unit_resources: usize,
    pub resolved_unit_resources: usize,
    pub missing_unit_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RasterDependencySummary {
    pub lot_mask_key: String,
    pub lot_mask_resolved_tgi: Option<String>,
    pub lot_mask_source_package: Option<String>,
    pub pixel_format: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mip_count: Option<u32>,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
    pub decode_supported: bool,
    pub roundtrip_equal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummary {
    pub root_name: String,
    pub manifest_name: Option<String>,
    pub manifest_format: Option<String>,
    pub manifest_version: Option<u64>,
    pub asset_schema_version: Option<u64>,
    pub unit_count: Option<usize>,
    pub model_lod_count: Option<usize>,
    pub source_tgi: Option<String>,
    pub referenced_tgis: Vec<String>,
    pub node_count: usize,
    pub connected_texture_slots: usize,
    pub has_reference_package: bool,
    pub lot_unit_count: usize,
    pub prop_unit_count: usize,
    pub bin_unit_count: usize,
    pub path_point_count: usize,
    pub path_count: usize,
    pub path_entry_count: usize,
    pub missing_path_entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileSummary {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

pub fn inspect_project(root: &Path) -> Result<SampleInspection, String> {
    if !root.is_dir() {
        return Err(format!(
            "sample root is not a directory: {}",
            root.display()
        ));
    }
    let manifest_path = root.join("package.json");
    let manifest = read_json(&manifest_path)?;
    let asset_path = root.join("asset.lot.json");
    let asset = if asset_path.is_file() {
        Some(read_json(&asset_path)?)
    } else {
        None
    };

    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let nodes = manifest["nodes"].as_array().cloned().unwrap_or_default();
    let mut diagnostics = Vec::new();
    let mut connected_texture_slots = 0;
    let mut has_reference_package = false;
    let units = asset
        .as_ref()
        .and_then(|value| value.get("units"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let count_kind = |kind: &str| {
        units
            .iter()
            .filter(|unit| unit["kind"].as_str() == Some(kind))
            .count()
    };
    let bin_unit_count = units
        .iter()
        .filter(|unit| unit.get("bin").is_some_and(|bin| !bin.is_null()))
        .count();
    let paths = asset
        .as_ref()
        .and_then(|value| value.pointer("/editor/workbench/paths"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let path_entry_count = paths
        .iter()
        .filter(|path| {
            path.get("entry")
                .and_then(Value::as_str)
                .is_some_and(|entry| !entry.trim().is_empty())
        })
        .count();
    let missing_path_entry_count = paths.len().saturating_sub(path_entry_count);
    for (index, path) in paths.iter().enumerate() {
        if path
            .get("entry")
            .and_then(Value::as_str)
            .is_none_or(|entry| entry.trim().is_empty())
        {
            diagnostics.push(error(
                "missing-path-entry",
                Some(format!("editor.workbench.paths[{index}].entry")),
            ));
        } else if path["entry"]
            .as_str()
            .is_some_and(|entry| parse_hex_u32(entry).is_none())
        {
            diagnostics.push(error(
                "invalid-path-entry",
                Some(format!("editor.workbench.paths[{index}].entry")),
            ));
        }
        if path.get("source").is_none() {
            diagnostics.push(error(
                "missing-path-source",
                Some(format!("editor.workbench.paths[{index}].source")),
            ));
        }
    }
    for node in &nodes {
        let node_path = node["schema"].as_str().unwrap_or("");
        if node_path.is_empty() || !root.join(node_path).is_file() {
            diagnostics.push(error("missing-node-schema", Some(node_path.to_string())));
        }
        if node["kind"].as_str() == Some("building-asset") {
            let schema = if node_path.is_empty() {
                None
            } else {
                read_json(&root.join(node_path)).ok()
            };
            let config = schema.as_ref().and_then(|v| v.get("config"));
            for key in ["slot0", "slot1", "slot2", "slot3", "slot4", "slot5"] {
                if config
                    .and_then(|v| v.get(key))
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
                {
                    connected_texture_slots += 1;
                }
            }
            if config
                .and_then(|v| v.get("asset"))
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                diagnostics.push(error("empty-building-asset", Some(node_path.to_string())));
            }
            if config
                .and_then(|v| v.get("property"))
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                diagnostics.push(error(
                    "missing-property-binding",
                    Some(node_path.to_string()),
                ));
            }
        }
    }
    if !asset_path.is_file() {
        diagnostics.push(error(
            "missing-asset-document",
            Some("asset.lot.json".into()),
        ));
    }
    if connected_texture_slots < 6 {
        diagnostics.push(error(
            "missing-six-slot-inputs",
            Some("nodes/building-asset".into()),
        ));
    }
    if let Some(reference) = asset.as_ref().and_then(|v| v.get("reference")) {
        has_reference_package = reference
            .get("package")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
    }
    if !has_reference_package {
        diagnostics.push(error(
            "missing-reference-package",
            Some("reference.package".into()),
        ));
    }

    let source_tgi = asset.as_ref().and_then(|v| {
        let tgi = v.pointer("/meta/source/tgi")?;
        Some(format!(
            "{}:{}:{}",
            scalar_text(&tgi["typeId"]),
            scalar_text(&tgi["group"]),
            scalar_text(&tgi["instance"])
        ))
    });
    let referenced_tgis = asset
        .as_ref()
        .map(collect_referenced_tgis)
        .unwrap_or_default();
    let project = ProjectSummary {
        root_name: root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string(),
        manifest_name: manifest["name"].as_str().map(str::to_string),
        manifest_format: manifest["format"].as_str().map(str::to_string),
        manifest_version: manifest["manifest_version"].as_u64(),
        asset_schema_version: asset.as_ref().and_then(|v| v["version"].as_u64()),
        unit_count: asset
            .as_ref()
            .and_then(|v| v["units"].as_array().map(Vec::len)),
        model_lod_count: asset.as_ref().and_then(|v| {
            v.pointer("/model/lods")
                .and_then(Value::as_array)
                .map(Vec::len)
        }),
        source_tgi,
        referenced_tgis,
        node_count: nodes.len(),
        connected_texture_slots,
        has_reference_package,
        lot_unit_count: usize::from(
            asset
                .as_ref()
                .is_some_and(|value| value.pointer("/editor/workbench/lotStyle").is_some()),
        ),
        prop_unit_count: count_kind("prop"),
        bin_unit_count,
        path_point_count: count_kind("pathPoint"),
        path_count: paths.len(),
        path_entry_count,
        missing_path_entry_count,
    };
    Ok(SampleInspection {
        project,
        files,
        diagnostics,
        raster: None,
        resources: None,
    })
}

/// Enrich a read-only sample report with the LotMask raster resolved from a
/// game package. The package is only memory-mapped and is never written.
pub fn inspect_project_with_raster(
    root: &Path,
    package_path: &Path,
) -> Result<SampleInspection, String> {
    inspect_project_with_packages(root, &[package_path.to_path_buf()])
}

pub fn inspect_project_with_packages(
    root: &Path,
    package_paths: &[PathBuf],
) -> Result<SampleInspection, String> {
    let mut report = inspect_project(root)?;
    let source_from_document = read_json(&root.join("asset.lot.json"))
        .ok()
        .and_then(|asset| {
            asset
                .pointer("/editor/workbench/lotStyle/sourceLot/tgi")
                .map(format_tgi)
        });
    let source = source_from_document.or_else(|| report.project.source_tgi.clone());
    let Some(source) = source else {
        report
            .diagnostics
            .push(error("missing-source-tgi", Some("meta.source.tgi".into())));
        return Ok(report);
    };
    let property_tgi = parse_tgi(&source)?;
    let packages: Vec<Package> = package_paths
        .iter()
        .map(|path| Package::open(path).map_err(|e| format!("{}: {e}", path.display())))
        .collect::<Result<_, _>>()?;
    validate_path_semantics(root, &mut report, &packages);
    report.resources = Some(validate_unit_resources(root, &mut report, &packages));
    let package = packages
        .first()
        .ok_or("at least one reference package is required")?;
    let property_entry = package.entry(property_tgi).or_else(|| {
        package.entries().iter().find(|entry| {
            entry.id.type_id == property_tgi.type_id && entry.id.instance == property_tgi.instance
        })
    });
    let Some(property_entry) = property_entry else {
        report
            .diagnostics
            .push(error("source-property-missing", Some(source.clone())));
        return Ok(report);
    };
    let property = PropertyFile::parse(&package.read(property_entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let Some(mask_key) = property.get(0x0CCB_7FD5).and_then(property_key) else {
        report
            .diagnostics
            .push(error("lot-mask-property-missing", Some(source.into())));
        return Ok(report);
    };
    let raster_id = ResourceId {
        type_id: 0x2F4E_681C,
        group: mask_key.group,
        instance: mask_key.instance,
    };
    let raster_entry = package.entry(raster_id).or_else(|| {
        package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x2F4E_681C && entry.id.instance == mask_key.instance)
    });
    let Some(raster_entry) = raster_entry else {
        report.raster = Some(RasterDependencySummary {
            lot_mask_key: format_key(mask_key),
            lot_mask_resolved_tgi: None,
            lot_mask_source_package: None,
            pixel_format: None,
            width: None,
            height: None,
            mip_count: None,
            bytes: None,
            sha256: None,
            decode_supported: false,
            roundtrip_equal: false,
        });
        report
            .diagnostics
            .push(error("lot-mask-raster-missing", Some(format_key(mask_key))));
        return Ok(report);
    };
    let bytes = package.read(raster_entry).map_err(|e| e.to_string())?;
    let digest = Sha256::digest(&bytes);
    let image = RasterImage::parse(&bytes).map_err(|e| e.to_string())?;
    let decode_supported = image.is_raw_rgba();
    let roundtrip_equal = decode_supported
        && image
            .decode_top_mip_rgba()
            .ok()
            .and_then(|rgba| {
                RasterImage::build_raw_bgra(image.width, image.height, &rgba, image.mip_count > 1)
                    .ok()
            })
            .is_some_and(|rebuilt| {
                rebuilt.decode_top_mip_rgba().ok() == image.decode_top_mip_rgba().ok()
            });
    report.raster = Some(RasterDependencySummary {
        lot_mask_key: format_key(mask_key),
        lot_mask_resolved_tgi: Some(format!(
            "{:08X}:{:08X}:{:08X}",
            raster_entry.id.type_id, raster_entry.id.group, raster_entry.id.instance
        )),
        lot_mask_source_package: Some(package.path().to_string_lossy().to_string()),
        pixel_format: Some(image.pixel_format),
        width: Some(image.width),
        height: Some(image.height),
        mip_count: Some(image.mip_count),
        bytes: Some(bytes.len() as u64),
        sha256: Some(format!("{digest:x}")),
        decode_supported,
        roundtrip_equal,
    });
    if !decode_supported {
        report.diagnostics.push(error(
            "lot-mask-raster-unsupported-format",
            Some(format_key(mask_key)),
        ));
    }
    if !roundtrip_equal {
        report.diagnostics.push(error(
            "lot-mask-raster-roundtrip-failed",
            Some(format_key(mask_key)),
        ));
    }
    Ok(report)
}

fn validate_unit_resources(
    root: &Path,
    report: &mut SampleInspection,
    packages: &[Package],
) -> ResourceDependencySummary {
    let Ok(asset) = read_json(&root.join("asset.lot.json")) else {
        return ResourceDependencySummary {
            requested_unit_resources: 0,
            resolved_unit_resources: 0,
            missing_unit_resources: vec![],
        };
    };
    let units = asset
        .pointer("/units")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut requested = Vec::new();
    for unit in units {
        let kind = unit.get("kind").and_then(Value::as_str).unwrap_or_default();
        if !matches!(kind, "prop" | "light" | "bin") {
            continue;
        }
        let Some(tgi) = unit.get("modelTgi").and_then(parse_json_resource_id) else {
            continue;
        };
        requested.push(tgi);
    }
    requested.sort_by_key(|id| (id.type_id, id.group, id.instance));
    requested.dedup();
    let mut missing = Vec::new();
    for id in &requested {
        let found = packages
            .iter()
            .any(|package| package.entries().iter().any(|entry| entry.id == *id));
        if !found {
            missing.push(format!("{id}"));
            report
                .diagnostics
                .push(error("unit-resource-missing", Some(format!("{id}"))));
        }
    }
    ResourceDependencySummary {
        requested_unit_resources: requested.len(),
        resolved_unit_resources: requested.len() - missing.len(),
        missing_unit_resources: missing,
    }
}

fn parse_json_resource_id(value: &Value) -> Option<ResourceId> {
    Some(ResourceId {
        type_id: parse_json_u32(value.get("typeId")?)?,
        group: parse_json_u32(value.get("group")?)?,
        instance: parse_json_u32(value.get("instance")?)?,
    })
}

fn parse_json_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.parse().ok().or_else(|| parse_hex_u32(text)))
        })
}

fn validate_path_semantics(root: &Path, report: &mut SampleInspection, packages: &[Package]) {
    let Ok(asset) = read_json(&root.join("asset.lot.json")) else {
        return;
    };
    let Some(paths) = asset
        .pointer("/editor/workbench/paths")
        .and_then(Value::as_array)
    else {
        return;
    };
    for (index, path) in paths.iter().enumerate() {
        let Some(entry_text) = path.get("entry").and_then(Value::as_str) else {
            continue;
        };
        let Some(entry_instance) = parse_hex_u32(entry_text) else {
            continue;
        };
        let Some(source_value) = path
            .get("source")
            .and_then(|source| source.get("tgi"))
            .or_else(|| path.get("source"))
        else {
            continue;
        };
        let source_text = format_tgi(source_value);
        let Ok(source_tgi) = parse_tgi(&source_text) else {
            report.diagnostics.push(error(
                "invalid-path-source",
                Some(format!("editor.workbench.paths[{index}].source.tgi")),
            ));
            continue;
        };
        let source_entry = packages.iter().find_map(|package| {
            package.entry(source_tgi).or_else(|| {
                package.entries().iter().find(|candidate| {
                    candidate.id.type_id == source_tgi.type_id
                        && candidate.id.instance == source_tgi.instance
                })
            })
        });
        let Some(source_entry) = source_entry else {
            report
                .diagnostics
                .push(error("path-source-property-missing", Some(source_text)));
            continue;
        };
        let source_package = packages
            .iter()
            .find(|package| package.entry(source_entry.id).is_some());
        let Some(source_package) = source_package else {
            continue;
        };
        let Ok(file) = source_package
            .read(source_entry)
            .ok()
            .and_then(|bytes| PropertyFile::parse(&bytes).ok())
            .ok_or(())
        else {
            continue;
        };
        let bound = file.get(0x0CC8_FE7A).is_some_and(|property| match &property.kind {
            sc_properties::Kind::Scalar(PropertyValue::Key(key)) => key.instance == entry_instance,
            sc_properties::Kind::Array(values) => values.iter().any(|value| matches!(value, PropertyValue::Key(key) if key.instance == entry_instance)),
            sc_properties::Kind::Empty => false,
            _ => false,
        });
        if !bound {
            report.diagnostics.push(error(
                "path-entry-not-bound",
                Some(format!("editor.workbench.paths[{index}].entry")),
            ));
        }
        let resource_exists = packages.iter().any(|package| {
            package.entries().iter().any(|candidate| {
                candidate.id.type_id == 0x00B1_B104
                    && (candidate.id.group & 0xFFFF) == 0xC400
                    && candidate.id.instance == entry_instance
            })
        });
        if !resource_exists {
            report.diagnostics.push(error(
                "path-entry-resource-missing",
                Some(format!("0x40E0C400:{entry_instance:08X}")),
            ));
        }
    }
}

fn parse_tgi(value: &str) -> Result<ResourceId, String> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("invalid TGI: {value}"));
    }
    let parse = |part: &str| {
        let trimmed = part.trim();
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            u32::from_str_radix(hex, 16).map_err(|e| e.to_string())
        } else {
            trimmed
                .parse::<u32>()
                .or_else(|_| u32::from_str_radix(trimmed, 16))
                .map_err(|e| e.to_string())
        }
    };
    Ok(ResourceId {
        type_id: parse(parts[0])?,
        group: parse(parts[1])?,
        instance: parse(parts[2])?,
    })
}

fn property_key(property: &sc_properties::Property) -> Option<Key> {
    match &property.kind {
        sc_properties::Kind::Scalar(PropertyValue::Key(key)) => Some(*key),
        _ => None,
    }
}

fn format_key(key: Key) -> String {
    format!("{:08X}:{:08X}:{:08X}", key.type_id, key.group, key.instance)
}

fn parse_hex_u32(value: &str) -> Option<u32> {
    u32::from_str_radix(
        value
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
        16,
    )
    .ok()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn collect_files(root: &Path, current: &Path, output: &mut Vec<FileSummary>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output)?;
        } else if path.is_file() {
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let digest = Sha256::digest(&bytes);
            output.push(FileSummary {
                path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                bytes: bytes.len() as u64,
                sha256: format!("{digest:x}"),
            });
        }
    }
    Ok(())
}

fn error(code: &str, path: Option<String>) -> SampleDiagnostic {
    SampleDiagnostic {
        severity: DiagnosticSeverity::Error,
        code: code.into(),
        path,
    }
}

fn scalar_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn collect_referenced_tgis(asset: &Value) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(tgi) = asset.pointer("/editor/workbench/lotStyle/sourceLot/tgi") {
        result.push(format_tgi(tgi));
    }
    if let Some(paths) = asset
        .pointer("/editor/workbench/paths")
        .and_then(Value::as_array)
    {
        for path in paths {
            if let Some(tgi) = path.get("source") {
                result.push(format_tgi(tgi));
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

fn format_tgi(tgi: &Value) -> String {
    format!(
        "{}:{}:{}",
        scalar_text(&tgi["typeId"]),
        scalar_text(&tgi["group"]),
        scalar_text(&tgi["instance"])
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbpf::{OverlayEntry, write_uncompressed_overlay};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str, building_config: &str, asset: Option<&str>) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openscp-sample-{name}-{id}"));
        fs::create_dir_all(root.join("schemas/assets")).unwrap();
        fs::write(root.join("package.json"), r#"{"name":"sample","format":"openscp.mod","manifest_version":2,"nodes":[{"id":"building-asset-7","kind":"building-asset","schema":"schemas/assets/building-asset-7.json"}]}"#).unwrap();
        fs::write(
            root.join("schemas/assets/building-asset-7.json"),
            building_config,
        )
        .unwrap();
        if let Some(asset) = asset {
            fs::write(root.join("asset.lot.json"), asset).unwrap();
        }
        root
    }

    #[test]
    fn university_shape_reports_empty_bindings_without_mutating_files() {
        let root = fixture(
            "university",
            r#"{"config":{"asset":"","property":"","slot0":"","slot1":"","slot2":"","slot3":"","slot4":"","slot5":""}}"#,
            Some(
                r#"{"version":1,"units":[{},{}],"model":{"lods":[]},"meta":{"source":{"tgi":{"typeId":"0x2F4E681B","group":"0x0","instance":"0x1"}}}}"#,
            ),
        );
        let before = fs::read(root.join("package.json")).unwrap();
        let report = inspect_project(&root).unwrap();
        let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"empty-building-asset"));
        assert!(codes.contains(&"missing-six-slot-inputs"));
        assert_eq!(report.project.unit_count, Some(2));
        assert_eq!(before, fs::read(root.join("package.json")).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn commercial_shape_counts_connected_slots_and_still_blocks_without_reference() {
        let root = fixture(
            "commercial",
            r#"{"config":{"asset":"assets/models/building.lotm","property":"","slot0":"texture-input-1","slot1":"texture-input-2","slot2":"texture-input-3","slot3":"texture-input-4","slot4":"texture-input-5","slot5":"texture-input-6"}}"#,
            Some(r#"{"version":1,"units":[{}, {}, {}],"model":{"lods":[]}}"#),
        );
        let report = inspect_project(&root).unwrap();
        assert_eq!(report.project.connected_texture_slots, 6);
        assert_eq!(report.project.unit_count, Some(3));
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "missing-property-binding")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "missing-reference-package")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn path_without_entry_is_a_blocking_diagnostic() {
        let root = fixture(
            "invalid-path",
            r#"{"config":{"asset":"asset.lotm","property":"lot"}}"#,
            Some(
                r#"{
                "version":1,
                "units":[{"kind":"pathPoint"}],
                "editor":{"workbench":{"paths":[{"id":"path:1","pointIds":["pathPoint:0"]}]}}
            }"#,
            ),
        );
        let report = inspect_project(&root).unwrap();
        assert_eq!(report.project.path_count, 1);
        assert_eq!(report.project.path_entry_count, 0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|item| item.code == "missing-path-entry")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the local AppData mods root"]
    fn real_appdata_samples_write_read_only_reports() {
        let root =
            PathBuf::from(std::env::var("OPEN_SCP_MODS_ROOT").expect("set OPEN_SCP_MODS_ROOT"));
        let projects = ["大学-综合实验楼-资产项目", "商业地标资产开发"];
        let mut reports = Vec::new();
        for name in projects {
            reports.push(inspect_project(&root.join(name)).unwrap());
        }
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tmp")
            .join("asset-build-sample-reports.json");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(output, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
        assert_eq!(reports.len(), 2);
        assert!(
            reports
                .iter()
                .all(|report| report.project.asset_schema_version == Some(1))
        );
    }

    #[test]
    #[ignore = "requires AppData samples and a local game package"]
    fn real_appdata_samples_resolve_lot_mask_rasters_without_writing() {
        let root =
            PathBuf::from(std::env::var("OPEN_SCP_MODS_ROOT").expect("set OPEN_SCP_MODS_ROOT"));
        let package = PathBuf::from(
            std::env::var("OPEN_SCP_GAME_PACKAGE").expect("set OPEN_SCP_GAME_PACKAGE"),
        );
        let projects = ["大学-综合实验楼-资产项目", "商业地标资产开发"];
        for name in projects {
            let report = inspect_project_with_raster(&root.join(name), &package).unwrap();
            let raster = report.raster.as_ref().unwrap_or_else(|| {
                panic!("LotMask raster summary missing: {:?}", report.diagnostics)
            });
            assert_eq!(
                raster.lot_mask_resolved_tgi.as_deref().map(|v| &v[..8]),
                Some("2F4E681C")
            );
            assert_eq!(raster.pixel_format, Some(21));
            assert!(raster.decode_supported);
            assert!(raster.roundtrip_equal);
        }
    }

    #[test]
    #[ignore = "requires AppData samples and the local game data directory"]
    fn real_appdata_samples_validate_path_entry_closure_across_packages() {
        let root =
            PathBuf::from(std::env::var("OPEN_SCP_MODS_ROOT").expect("set OPEN_SCP_MODS_ROOT"));
        let game_root =
            PathBuf::from(std::env::var("OPEN_SCP_GAME_DATA").expect("set OPEN_SCP_GAME_DATA"));
        let packages: Vec<PathBuf> = fs::read_dir(&game_root)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension().and_then(|ext| ext.to_str()) == Some("package")).then_some(path)
            })
            .collect();
        for name in ["大学-综合实验楼-资产项目", "商业地标资产开发"] {
            let report = inspect_project_with_packages(&root.join(name), &packages).unwrap();
            assert_eq!(report.project.path_count, 1);
            assert_eq!(report.project.missing_path_entry_count, 0);
            if name == "大学-综合实验楼-资产项目" {
                assert!(
                    !report.diagnostics.iter().any(|item| matches!(
                        item.code.as_str(),
                        "path-source-property-missing"
                            | "path-entry-not-bound"
                            | "path-entry-resource-missing"
                    )),
                    "{}: {:?}",
                    name,
                    report.diagnostics
                );
                let resources = report.resources.as_ref().unwrap();
                assert!(resources.requested_unit_resources > 0);
                assert!(
                    resources.missing_unit_resources.is_empty(),
                    "{}: {:?}",
                    name,
                    resources
                );
            } else {
                assert!(
                    report.diagnostics.iter().any(|item| matches!(
                        item.code.as_str(),
                        "path-source-property-missing"
                            | "path-entry-not-bound"
                            | "path-entry-resource-missing"
                    )),
                    "{}: {:?}",
                    name,
                    report.diagnostics
                );
            }
        }
    }

    #[test]
    #[ignore = "requires a local game package"]
    fn real_lot_mask_raster_overlay_fixture_roundtrips_without_touching_source() {
        let package_path = PathBuf::from(
            std::env::var("OPEN_SCP_GAME_PACKAGE").expect("set OPEN_SCP_GAME_PACKAGE"),
        );
        let package = Package::open(&package_path).unwrap();
        let source_bytes = fs::read(&package_path).unwrap();
        let cases = [
            ResourceId {
                type_id: 0x00B1_B104,
                group: 0x58F0_C000,
                instance: 0x2063_CCDD,
            },
            ResourceId {
                type_id: 0x00B1_B104,
                group: 0x41F0_C000,
                instance: 0x0483_239E,
            },
        ];
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tmp")
            .join("raster-overlay-fixtures");
        fs::create_dir_all(&fixture_root).unwrap();
        for lot_tgi in cases {
            let lot_entry = package.entry(lot_tgi).unwrap();
            let mut property = PropertyFile::parse(&package.read(lot_entry).unwrap()).unwrap();
            let mask = property_key(property.get(0x0CCB_7FD5).unwrap()).unwrap();
            let source_raster = ResourceId {
                type_id: 0x2F4E_681C,
                group: mask.group,
                instance: mask.instance,
            };
            let raster_entry = package.entry(source_raster).unwrap();
            let new_raster = ResourceId {
                type_id: 0x2F4E_681C,
                group: 0,
                instance: mask.instance ^ 0x4000_0000,
            };
            crate::asset_build_m1::raster::ensure_target_free(
                package.entries().iter().map(|entry| entry.id),
                new_raster,
            )
            .unwrap();
            let artifact = crate::asset_build_m1::raster::encode_pixel_delta(
                &package.read(raster_entry).unwrap(),
                new_raster,
                0,
            )
            .unwrap();
            property
                .values
                .iter_mut()
                .find(|item| item.hash == 0x0CCB_7FD5)
                .unwrap()
                .kind = sc_properties::Kind::Scalar(PropertyValue::Key(Key {
                type_id: new_raster.type_id,
                group: new_raster.group,
                instance: new_raster.instance,
            }));
            let overlay = fixture_root.join(format!("{:08X}.package", lot_tgi.instance));
            fs::write(
                &overlay,
                write_uncompressed_overlay(&[
                    OverlayEntry::new(new_raster, artifact.bytes.clone()),
                    OverlayEntry::new(lot_tgi, property.encode_canonical().unwrap()),
                ])
                .unwrap(),
            )
            .unwrap();
            let output = Package::open(&overlay).unwrap();
            let patched =
                PropertyFile::parse(&output.read(output.entry(lot_tgi).unwrap()).unwrap()).unwrap();
            assert_eq!(
                property_key(patched.get(0x0CCB_7FD5).unwrap())
                    .unwrap()
                    .instance,
                new_raster.instance
            );
            let output_image =
                RasterImage::parse(&output.read(output.entry(new_raster).unwrap()).unwrap())
                    .unwrap();
            assert_eq!(
                output_image.decode_top_mip_rgba().unwrap(),
                RasterImage::parse(&artifact.bytes)
                    .unwrap()
                    .decode_top_mip_rgba()
                    .unwrap()
            );
        }
        assert_eq!(
            Sha256::digest(&source_bytes),
            Sha256::digest(fs::read(&package_path).unwrap())
        );
    }
}
