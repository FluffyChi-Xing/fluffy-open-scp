//! Dump one Swarm effect collection (`0xEA5118B0`) to JSON for format study.
//!
//! cargo run -p probes --release --example effect_swarm_dump -- \
//!   <package> <group_hex> <instance_hex> <out.json>

use std::collections::BTreeMap;

use dbpf::Package;
use sc_properties::swarm::{EFFECT_COLLECTION_TYPE, SwarmCollection};
use sc_properties::swarm_material::decode_effect_materials;
use sc_properties::swarm_particle::{
    decode_meta_particle_components, decode_particle_components_prefix,
};
use serde_json::{Value, json};

fn hex(value: u32) -> String {
    format!("{value:08X}")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let group = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("group");
    let instance = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).expect("instance");
    let entry = package
        .entries()
        .iter()
        .find(|entry| {
            entry.id.type_id == EFFECT_COLLECTION_TYPE
                && entry.id.group == group
                && entry.id.instance == instance
        })
        .expect("collection entry");
    let bytes = package.read(entry).expect("read collection");
    let collection = SwarmCollection::parse(&bytes).expect("parse collection");

    let mut histogram = BTreeMap::<u8, usize>::new();
    for visual in &collection.visual_effects {
        for block in &visual.blocks {
            *histogram.entry(block.type_code).or_default() += 1;
        }
    }

    let mut particles = Vec::new();
    let (decoded, particle_error) = decode_particle_components_prefix(&collection, &bytes);
    for particle in decoded {
        particles.push(serde_json::to_value(&particle).expect("serialize particle"));
    }
    let meta = decode_meta_particle_components(&collection, &bytes).unwrap_or_default();
    let mut meta_particles = Vec::new();
    for item in &meta {
        meta_particles.push(json!({
            "index": item.index,
            "child": item.child.as_ref().map(|child| json!({
                "typeCode": child.type_code,
                "index": child.index,
            })),
        }));
    }

    let materials = decode_effect_materials(&collection, &bytes).unwrap_or_default();
    let mut material_values: Vec<Value> = materials
        .values()
        .map(|material| {
            json!({
                "id": hex(material.instance),
                "group": hex(material.group),
                "shaderGroup": hex(material.shader_group),
                "shaderInstance": hex(material.shader_instance),
                "format": material.format,
                "textures": material.textures.iter().map(|(slot, texture)| json!({
                    "slot": slot,
                    "group": hex(texture.group),
                    "instance": hex(texture.instance),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    material_values.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));

    let sections: Vec<Value> = collection
        .components
        .iter()
        .map(|section| {
            json!({
                "kind": "component",
                "type": section.type_code,
                "version": section.version,
                "count": section.count,
                "offset": section.payload_offset,
                "size": section.payload_size,
            })
        })
        .chain(collection.resources.iter().map(|section| {
            json!({
                "kind": "auxiliary",
                "type": section.type_code,
                "version": section.version,
                "count": section.count,
                "offset": section.payload_offset,
                "size": section.payload_size,
            })
        }))
        .collect();

    let visuals: Vec<Value> = collection
        .visual_effects
        .iter()
        .enumerate()
        .map(|(index, visual)| {
            json!({
                "index": index,
                "flags": visual.flags,
                "blocks": visual.blocks.iter().map(|block| json!({
                    "type": block.type_code,
                    "flags": block.flags,
                    "component": block.component_index,
                    "scale": block.transform.scale,
                    "rotation": block.transform.rotation,
                    "offset": block.transform.offset,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    let exports: Vec<Value> = collection
        .exports
        .iter()
        .map(|export| {
            json!({
                "index": export.effect_index,
                "import": export.import_index,
                "group": hex(export.group_id),
                "hash": hex(export.name_hash),
            })
        })
        .collect();

    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    for (kind, count) in &histogram {
        by_type.insert(format!("{kind}"), *count);
    }

    let document = json!({
        "source": {
            "package": args[0],
            "type": hex(EFFECT_COLLECTION_TYPE),
            "group": hex(group),
            "instance": hex(instance),
            "bytes": bytes.len(),
        },
        "sections": sections,
        "visualVersion": collection.visual_version,
        "visualEffectCount": collection.visual_effects.len(),
        "exportCount": collection.exports.len(),
        "blockHistogram": by_type,
        "visualEffects": visuals,
        "exports": exports,
        "particles": particles,
        "metaParticles": meta_particles,
        "materials": material_values,
        "particleError": particle_error.map(|error| format!("{error}")),
    });
    std::fs::write(&args[3], serde_json::to_vec_pretty(&document).expect("json"))
        .expect("write json");
    println!(
        "wrote {} ({} bytes, {} visuals, {} particles, {} materials) -> {}",
        args[3],
        bytes.len(),
        collection.visual_effects.len(),
        particles.len(),
        materials.len(),
        &args[3]
    );
}
