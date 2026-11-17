use super::{GRAPHICS, Recipe, property, property_id};
use dbpf::Package;
use sc_properties::{
    Value,
    map_assets::{RoadAssets, composed_road_appearance, road_appearance},
};

const APPEARANCE: u32 = 0x0955_8B7E;
const NORMAL_AVENUE: u32 = 0xE5CE_83A6;
const TRACK_GROUP: u32 = 0x355D_03EA;

fn ground(
    game: &Package,
    dependencies: &[&Package],
    recipe: Recipe,
    future: bool,
) -> Result<(u32, RoadAssets), String> {
    let (_, path) = recipe.donor();
    let graphics = property(game, property_id(GRAPHICS, path))?;
    let field = if future { 0x107E_2B61 } else { 0x0953_2375 };
    let Some(Value::Key(key)) = graphics.get(field).and_then(|prop| prop.scalar()) else {
        return Err("road_recipe_appearance_missing".into());
    };
    let assets = road_appearance(game, key.group, key.instance, dependencies)
        .ok_or("road_recipe_appearance_missing")?;
    Ok((key.instance, assets))
}

pub(crate) fn append(
    game: &Package,
    dependencies: &[&Package],
    future: bool,
    assets: &mut RoadAssets,
) -> Result<(), String> {
    // Component banks are preview-only keys, distinct from native path IDs.
    let (id, mut median) = ground(game, dependencies, Recipe::MediumAvenue, future)?;
    let dividers = median
        .ribbons
        .remove(&id)
        .unwrap_or_default()
        .into_iter()
        .filter(|ribbon| {
            ribbon.offset[0] == 0.0
                && ribbon.offset[2] >= 0.0
                && ribbon.rotation[1] == 0.0
                && ribbon.rotation[2] == 0.0
        })
        .collect();
    assets.ribbons.insert(1, dividers);
    assets.textures.extend(median.textures);

    let (id, mut rails) = ground(game, dependencies, Recipe::Streetcar, future)?;
    let base = rails
        .ribbons
        .remove(&id)
        .unwrap_or_default()
        .into_iter()
        .find(|ribbon| {
            ribbon.offset[0] == 0.0
                && ribbon.offset[2] >= 0.0
                && ribbon.rotation[1] == 0.0
                && ribbon.rotation[2] == 0.0
        });
    let tracks = property(game, property_id(0x0D94_2043, TRACK_GROUP))?;
    let mut track_assets = composed_road_appearance(game, 2, tracks, dependencies);
    let mut ribbons = base.into_iter().collect::<Vec<_>>();
    ribbons.extend(track_assets.ribbons.remove(&2).unwrap_or_default());
    assets.ribbons.insert(2, ribbons);
    assets
        .sweeps
        .insert(2, track_assets.sweeps.remove(&2).unwrap_or_default());
    assets.textures.extend(rails.textures);
    assets.textures.extend(track_assets.textures);
    assets.models.extend(track_assets.models);

    // E5CE83A6 contains the two native median trees. Retain their models and
    // placement cadence for user-authored tree strips.
    let trees = property(game, property_id(APPEARANCE, NORMAL_AVENUE))?;
    let mut tree_assets = composed_road_appearance(game, 3, trees, dependencies);
    let sweeps = tree_assets
        .sweeps
        .remove(&3)
        .unwrap_or_default()
        .into_iter()
        .filter(|sweep| matches!(sweep.model, 0x4DF4_3690 | 0x1113_B131))
        .collect();
    assets.sweeps.insert(3, sweeps);
    assets.models.extend(tree_assets.models);
    Ok(())
}
