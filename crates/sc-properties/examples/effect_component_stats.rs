//! Summarise Swarm component usage for render-preview comparison.
//!
//! `cargo run -p sc-properties --release --example effect_component_stats -- <package>...`

use std::collections::BTreeMap;

use dbpf::Package;
use sc_properties::swarm::{EFFECT_COLLECTION_TYPE, SwarmCollection};
use sc_properties::swarm_particle::{decode_meta_particle_components, decode_particle_components_prefix};

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(package) = Package::open(&path) else {
            eprintln!("skip {path}");
            continue;
        };
        let mut collections = 0usize;
        let mut visuals = 0usize;
        let mut blocks = BTreeMap::<u8, usize>::new();
        let mut child_types = BTreeMap::<u16, usize>::new();
        let mut nested_visuals = 0usize;
        let mut nested_visuals_with_particles = 0usize;
        let mut invalid_nested_visuals = 0usize;
        let mut direct_particles = 0usize;
        let mut meta_particles = 0usize;
        let mut fast_blocks = 0usize;
        let mut zero_rate = 0usize;
        let mut nonzero_rate = 0usize;
        for entry in package
            .entries()
            .iter()
            .filter(|entry| entry.id.type_id == EFFECT_COLLECTION_TYPE)
        {
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(collection) = SwarmCollection::parse(&bytes) else { continue };
            collections += 1;
            visuals += collection.visual_effects.len();
            for visual in &collection.visual_effects {
                for block in &visual.blocks {
                    *blocks.entry(block.type_code).or_default() += 1;
                    match block.type_code {
                        1 => direct_particles += 1,
                        2 => meta_particles += 1,
                        12 => fast_blocks += 1,
                        _ => {}
                    }
                }
            }
            let (particles, _) = decode_particle_components_prefix(&collection, &bytes);
            for particle in particles {
                if particle.rate.iter().any(|rate| *rate > 0.0) {
                    nonzero_rate += 1;
                } else {
                    zero_rate += 1;
                }
            }
            let Ok(metas) = decode_meta_particle_components(&collection, &bytes) else { continue };
            for meta in metas {
                if let Some(child) = meta.child {
                    *child_types.entry(child.type_code).or_default() += 1;
                    if child.type_code == 0 {
                        if let Some(visual) = collection.visual_effects.get(child.index) {
                            nested_visuals += 1;
                            nested_visuals_with_particles += usize::from(visual.blocks.iter().any(|block| matches!(block.type_code, 1 | 2 | 12)));
                        } else {
                            invalid_nested_visuals += 1;
                        }
                    }
                }
            }
        }
        let name = path.rsplit(['/', '\\']).next().unwrap_or(&path);
        println!(
            "{name}: collections={collections} visuals={visuals} blocks={blocks:?} "
        );
        println!(
            "  direct_particles={direct_particles} meta_blocks={meta_particles} "
        );
        println!(
            "  fast_blocks={fast_blocks} child_types={child_types:?} "
        );
        println!("  particle_rate_nonzero={nonzero_rate} zero={zero_rate}");
        println!("  nested_visuals={nested_visuals} with_particle_blocks={nested_visuals_with_particles} invalid={invalid_nested_visuals}");
    }
}
