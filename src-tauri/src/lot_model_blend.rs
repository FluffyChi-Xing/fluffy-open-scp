//! Export a game LOTM payload as an editable Blender file.
//!
//! The back end stages the payload into a work directory (one GLB per mesh,
//! the six-slot PNGs and parameter table per material, plus a manifest), runs
//! `blender_scripts/export_lotm_blend.py` in background mode, and moves the
//! resulting `.blend` to the caller's path. Blender is the only side effect
//! boundary: staging is plain file I/O and every step names its failure.
//!
//! Slot numbering follows the game's container order, not the PNG indices in
//! [`super::asset_build_m1::lotm`]: slot1 = tint, slot2 = normal,
//! slot3 = shader, slot4 = palette, slot5 = interior. That is the same
//! contract the source-file page's six-slot export uses, so both entry points
//! produce the same files.

use crate::asset_build_m1::lotm::{self, LotmDocument};
use serde::Serialize;
use std::path::{Path, PathBuf};

const EXPORT_SCRIPT: &str = include_str!("blender_scripts/export_lotm_blend.py");

/// Container PNG index → game slot number.
const SLOT_SOURCES: [(u8, usize); 5] = [
    (1, lotm::LotmMaterial::PNG_TINT),
    (2, lotm::LotmMaterial::PNG_NORMAL),
    (3, lotm::LotmMaterial::PNG_SHADER),
    (4, lotm::LotmMaterial::PNG_PALETTE),
    (5, lotm::LotmMaterial::PNG_INTERIOR),
];

const UV_LAYER_NAMES: [&str; 4] = [
    "UVMap",
    "TEXCOORD_1_material",
    "TEXCOORD_2",
    "TEXCOORD_3_facade",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshEntry {
    file: String,
    material: u32,
    uv_kind: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialEntry {
    index: u32,
    param_cols: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    params_f32: Option<Vec<f32>>,
    /// slot number (as string) → bundle-relative PNG path.
    slots: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    source: String,
    asset_name: String,
    meshes: Vec<MeshEntry>,
    materials: Vec<MaterialEntry>,
    uv_layer_names: [&'static str; 4],
    readme: String,
}

/// Write the payload into `dir`; returns the manifest file path.
pub(crate) fn stage(
    dir: &Path,
    payload: &[u8],
    source_label: &str,
    asset_name: &str,
) -> Result<PathBuf, String> {
    let document = lotm::parse(payload)?;
    if document.meshes.is_empty() {
        return Err("model payload carries no meshes".into());
    }
    std::fs::create_dir_all(dir).map_err(|error| format!("blend work dir: {error}"))?;
    let mesh_dir = dir.join("meshes");
    std::fs::create_dir_all(&mesh_dir).map_err(|error| format!("mesh dir: {error}"))?;

    let mut meshes = Vec::with_capacity(document.meshes.len());
    for (index, bytes) in document.meshes.iter().enumerate() {
        let file = format!("meshes/{index:03}.glb");
        std::fs::write(dir.join(&file), bytes).map_err(|error| format!("mesh {index}: {error}"))?;
        let binding = document.mesh_material.get(index).copied();
        meshes.push(MeshEntry {
            file,
            material: binding.map(|value| value.material).unwrap_or(0),
            uv_kind: binding.map(|value| value.uv_kind).unwrap_or(0),
        });
    }

    let mut materials = Vec::with_capacity(document.materials.len());
    for (index, material) in document.materials.iter().enumerate() {
        let material_dir = dir.join("materials").join(format!("{index:03}"));
        std::fs::create_dir_all(&material_dir)
            .map_err(|error| format!("material dir {index}: {error}"))?;
        let mut slots = std::collections::BTreeMap::new();
        for (slot, png_index) in SLOT_SOURCES {
            let Some(Some(bytes)) = material.pngs.get(png_index) else {
                continue;
            };
            let file = format!("materials/{index:03}/slot{slot}.png");
            std::fs::write(dir.join(&file), bytes)
                .map_err(|error| format!("slot {slot} of material {index}: {error}"))?;
            slots.insert(slot.to_string(), file);
        }
        materials.push(MaterialEntry {
            index: index as u32,
            param_cols: material.param_cols,
            params_f32: material
                .params_f32
                .as_deref()
                .map(decode_f32_table)
                .filter(|values| !values.is_empty()),
            slots,
        });
    }

    let manifest = Manifest {
        source: source_label.to_string(),
        asset_name: asset_name.to_string(),
        meshes,
        materials,
        uv_layer_names: UV_LAYER_NAMES,
        readme: readme_text(source_label, asset_name, &document),
    };
    let path = dir.join("manifest.json");
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("manifest encode: {error}"))?;
    std::fs::write(&path, bytes).map_err(|error| format!("manifest write: {error}"))?;
    Ok(path)
}

fn decode_f32_table(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn readme_text(source: &str, asset_name: &str, document: &LotmDocument) -> String {
    format!(
        "OpenSCP game asset export\n\
         =========================\n\
         asset:    {asset_name}\n\
         source:   {source}\n\
         payload:  LOTM v{} / {} meshes / {} materials\n\
         \n\
         Materials are named OpenSCP_<index>_<asset>. Each carries the game's\n\
         six texture slots as labelled image nodes:\n\
         \n\
           slot1_tint      tint / colour-control map        -> Base Color\n\
           slot2_normal    normal map                       -> Normal (via Normal Map)\n\
           slot3_shader    relief + specular + interior mask   (unconnected)\n\
           slot4_palette   16x512 palette lookup table         (unconnected)\n\
           slot5_interior  interior / night emissive map    -> Emission Color\n\
         \n\
         slot0 parameter tables are embedded as text blocks\n\
         (params_matNN.json: {{ paramCols, values }}).\n\
         \n\
         UV layers: UVMap = TEXCOORD_0, TEXCOORD_1_material = material\n\
         index/seed, TEXCOORD_2 = spare, TEXCOORD_3_facade = facade world\n\
         projection. All textures are packed into this .blend.\n",
        document.version,
        document.meshes.len(),
        document.materials.len(),
    )
}

/// Run Blender to turn a staged bundle into `target`.
pub(crate) fn run_blender(bundle: &Path, target: &Path) -> Result<(), String> {
    let script = bundle.join("export_lotm_blend.py");
    std::fs::write(&script, EXPORT_SCRIPT).map_err(|error| format!("export script: {error}"))?;
    let output = crate::blender_service::run_script(
        &script,
        &[
            crate::blender_service::path_arg(bundle),
            crate::blender_service::path_arg(target),
        ],
    )?;
    if target.is_file() {
        return Ok(());
    }
    let tail = crate::blender_service::stderr_tail(&output, 16);
    Err(if tail.is_empty() {
        "Blender did not produce the .blend file".into()
    } else {
        format!("Blender could not export the .blend: {tail}")
    })
}

/// Full export: stage the payload next to `target` and publish the `.blend`
/// there. The work directory never outlives the call.
///
/// The staging root sits beside the destination rather than in `%TEMP%`: the
/// user already proved the destination writable with the save dialog, and
/// sandboxed/redirected temp directories otherwise fail with access denied.
pub(crate) fn export_to(
    payload: &[u8],
    target: &Path,
    source_label: &str,
    asset_name: &str,
) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| format!("output directory: {error}"))?;
    let bundle = parent.join(format!(
        ".openscp-blend-export-{}-{stamp}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&bundle);
    let result = (|| {
        stage(&bundle, payload, source_label, asset_name)?;
        run_blender(&bundle, target)
    })();
    let _ = std::fs::remove_dir_all(&bundle);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal but structurally valid GLB (header + empty JSON chunk).
    fn tiny_glb() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"}}   "#;
        let total = 12 + 8 + json.len();
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        out.extend_from_slice(json);
        out
    }

    /// Minimal v9 container: 2 meshes, 1 material with tint/palette only.
    fn sample_payload() -> Vec<u8> {
        let mut out = b"LOTM".to_vec();
        out.extend_from_slice(&9u32.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes());
        for _ in 0..2 {
            let mesh = tiny_glb();
            out.extend_from_slice(&(mesh.len() as u32).to_le_bytes());
            out.extend_from_slice(&mesh);
        }
        out.extend_from_slice(&1u32.to_le_bytes()); // material count
        // v9: 8 PNG slots — only tint (4) and palette (5) present.
        let mut pngs: Vec<Option<&[u8]>> = vec![None; 8];
        pngs[lotm::LotmMaterial::PNG_TINT] = Some(b"tint-png");
        pngs[lotm::LotmMaterial::PNG_PALETTE] = Some(b"palette-png");
        for png in pngs {
            let bytes = png.unwrap_or(b"");
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        let params: Vec<f32> = (0..8).map(|value| value as f32).collect();
        out.extend_from_slice(&(params.len() as u32 * 4).to_le_bytes());
        for value in &params {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&2u32.to_le_bytes()); // param_cols
        for material in [0u32, 0u32] {
            out.extend_from_slice(&material.to_le_bytes());
            out.push(2); // uv kind
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // diagnostics
        out
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = crate::test_scratch::scratch_base()
            .join(format!("openscp-lotm-blend-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stage_writes_meshes_slots_and_manifest() {
        let root = scratch("stage");
        let manifest_path = stage(&root, &sample_payload(), "00B1B104:0:1", "willis").unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

        assert_eq!(manifest["meshes"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["materials"].as_array().unwrap().len(), 1);
        assert_eq!(manifest["source"], "00B1B104:0:1");
        assert_eq!(manifest["uvLayerNames"].as_array().unwrap().len(), 4);

        assert!(root.join("meshes/000.glb").is_file());
        assert!(root.join("meshes/001.glb").is_file());
        let slots = &manifest["materials"][0]["slots"];
        assert_eq!(slots["1"], "materials/000/slot1.png");
        assert_eq!(slots["4"], "materials/000/slot4.png");
        assert!(slots.get("2").is_none(), "absent slots are not declared");
        assert_eq!(
            std::fs::read(root.join("materials/000/slot1.png")).unwrap(),
            b"tint-png"
        );
        assert_eq!(manifest["materials"][0]["paramCols"], 2);
        assert_eq!(
            manifest["materials"][0]["paramsF32"].as_array().unwrap().len(),
            8
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// Real payload → real Blender → read back the .blend.
    /// Run: `SC_LOTM_FIXTURE=<path to a LOTM container> cargo test
    /// -p fluffy-open-scp --lib real_lotm_exports_editable_blend -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a local LOTM fixture and Blender"]
    fn real_lotm_exports_editable_blend() {
        let source = std::env::var("SC_LOTM_FIXTURE").expect("set SC_LOTM_FIXTURE");
        let payload = std::fs::read(&source).expect("read LOTM fixture");
        let dir = scratch("real");
        let target = dir.join("asset.blend");
        export_to(&payload, &target, "fixture", "fixture").expect("blend export");
        let size = std::fs::metadata(&target).unwrap().len();
        assert!(size > 4096, "blend file looks empty: {size} bytes");
        assert!(
            !dir.read_dir()
                .unwrap()
                .any(|entry| entry
                    .map(|entry| entry.file_name().to_string_lossy().starts_with(".openscp-blend-export-"))
                    .unwrap_or(false)),
            "staging directory must not survive the call"
        );
        println!("blend bytes: {size}");
        // Manual verification hook: copy the artifact somewhere inspectable.
        if let Ok(keep) = std::env::var("SC_BLEND_KEEP") {
            std::fs::copy(&target, &keep).expect("copy blend artifact");
            println!("kept at {keep}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stage_rejects_empty_mesh_list() {
        let root = scratch("empty");
        let mut out = b"LOTM".to_vec();
        out.extend_from_slice(&9u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // mesh count
        out.extend_from_slice(&0u32.to_le_bytes()); // material count
        out.extend_from_slice(&0u32.to_le_bytes()); // diagnostics length
        let error = stage(&root, &out, "x", "y").unwrap_err();
        assert!(error.contains("no meshes"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }
}
