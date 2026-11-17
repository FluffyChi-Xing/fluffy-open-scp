use super::Recipe;
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum StripKind {
    Forward,
    Backward,
    Walk,
    Margin,
    Median,
    Trees,
    Rail,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Strip {
    id: String,
    kind: StripKind,
    width: f64,
}

pub(super) fn deserialize_section<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<Strip>>, D::Error> {
    Vec::deserialize(deserializer).map(Some)
}

pub(super) fn valid(section: &[Strip]) -> bool {
    let mut ids = HashSet::new();
    section.iter().all(|strip| {
        !strip.id.is_empty()
            && ids.insert(&strip.id)
            && strip.width.is_finite()
            && strip.width > 0.0
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    lanes: usize,
    width: f64,
    offset: f64,
    walk_width: f64,
    walk_offset: f64,
}

impl Template {
    fn strips(&self, streetcar: bool) -> Vec<(StripKind, f64)> {
        use StripKind::*;
        let gap = (self.walk_offset - self.offset - self.lanes as f64 * self.width).max(0.0);
        let mut section = vec![(Walk, self.walk_width)];
        if gap > 0.0 {
            section.push((Margin, gap));
        }
        section.extend((0..self.lanes).map(|_| (Backward, self.width)));
        if self.offset > 0.0 {
            section.push((if streetcar { Rail } else { Median }, self.offset * 2.0));
        }
        section.extend((0..self.lanes).map(|_| (Forward, self.width)));
        if gap > 0.0 {
            section.push((Margin, gap));
        }
        section.push((Walk, self.walk_width));
        section
    }
}

pub(super) fn resolve(section: &[Strip]) -> Result<Recipe, String> {
    if section.is_empty() {
        return Err("road_draft_empty".into());
    }
    let recipes: BTreeMap<String, Template> = serde_json::from_str(include_str!(
        "../../../src/pages/studio/components/road/roadRecipes.json"
    ))
    .map_err(|error| error.to_string())?;
    let name = recipes
        .iter()
        .find(|(name, template)| {
            let expected = template.strips(name.as_str() == "streetcar");
            expected.len() == section.len()
                && expected.iter().zip(section).all(|((kind, width), strip)| {
                    *kind == strip.kind && (width - strip.width).abs() < 1e-8
                })
        })
        .map(|(name, _)| name)
        .ok_or("road_draft_layout_unsupported")?;
    serde_json::from_value(serde_json::Value::String(name.clone()))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_native_geometry_builds_but_a_second_median_does_not() {
        let recipes: BTreeMap<String, Template> = serde_json::from_str(include_str!(
            "../../../src/pages/studio/components/road/roadRecipes.json"
        ))
        .unwrap();
        let mut section: Vec<_> = recipes["mediumAvenue"]
            .strips(false)
            .into_iter()
            .enumerate()
            .map(|(index, (kind, width))| Strip {
                id: index.to_string(),
                kind,
                width,
            })
            .collect();
        assert!(matches!(resolve(&section), Ok(Recipe::MediumAvenue)));
        section.push(Strip {
            id: "extra".into(),
            kind: StripKind::Median,
            width: 2.0,
        });
        assert_eq!(
            resolve(&section).err().as_deref(),
            Some("road_draft_layout_unsupported")
        );
        assert_eq!(resolve(&[]).err().as_deref(), Some("road_draft_empty"));
    }
}
