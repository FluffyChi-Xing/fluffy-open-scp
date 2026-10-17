//! Literal city introduction fields from the shipped JS registry (never evaluated).
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CityInfo {
    pub name_entry: String,
    pub description_entry: Option<String>,
    pub description: Option<String>,
    pub description_en: Option<String>,
    pub buildable: Option<u32>,
    pub wind: Option<u32>,
    pub water: Option<u32>,
    pub coal: Option<u32>,
    pub ore: Option<u32>,
    pub oil: Option<u32>,
    pub rail: Option<bool>,
    pub shipping: Option<bool>,
}

fn literal<'a>(record: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("{key}:\"");
    let tail = record.split_once(&marker)?.1;
    Some(tail.split_once('"')?.0)
}

fn parse(
    js: &str,
    names: &HashMap<u32, String>,
    english: &HashMap<u32, String>,
) -> HashMap<u32, CityInfo> {
    let mut out = HashMap::new();
    for tail in js
        .split("cityName:\"AutomatedRegionTemplates.json!0x")
        .skip(1)
    {
        let Some((id, record)) = tail.split_once('"') else {
            continue;
        };
        let Ok(id) = u32::from_str_radix(id, 16) else {
            continue;
        };
        // Each city object contains scalar introduction fields followed by resources.
        let record = record.split('}').next().unwrap_or("");
        let description_id = literal(record, "cityDescription")
            .and_then(|s| s.split_once("!0x"))
            .and_then(|(_, s)| u32::from_str_radix(s, 16).ok());
        let number = |key| literal(record, key).and_then(|s| s.parse::<u32>().ok());
        let boolean = |key| literal(record, key).and_then(|s| s.parse::<bool>().ok());
        out.insert(
            id,
            CityInfo {
                name_entry: format!("{id:08X}"),
                description_entry: description_id.map(|id| format!("{id:08X}")),
                description: description_id.and_then(|id| names.get(&id).cloned()),
                description_en: description_id.and_then(|id| english.get(&id).cloned()),
                buildable: number("percentageBuildable").filter(|n| *n <= 100),
                wind: number("wind"),
                water: number("water"),
                coal: number("coal"),
                ore: number("rawOre"),
                oil: number("crudeOil"),
                rail: boolean("rail"),
                shipping: boolean("shipping"),
            },
        );
    }
    out
}

pub fn registry(
    game: &dbpf::Package,
    locale: Option<&dbpf::Package>,
    english: Option<&dbpf::Package>,
) -> HashMap<u32, CityInfo> {
    let names = locale
        .map(crate::region_3d::parse_locale_names)
        .unwrap_or_default();
    let english = english
        .map(crate::region_3d::parse_locale_names)
        .unwrap_or_default();
    let mut out = HashMap::new();
    for entry in game.entries().iter().filter(|e| e.id.type_id == 0x67771F5C) {
        if let Ok(bytes) = game.read(entry) {
            if let Ok(js) = std::str::from_utf8(&bytes) {
                out.extend(parse(js, &names, &english));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn introduction_fields_do_not_bleed_between_cities() {
        let js = r#"1026:{cityName:"AutomatedRegionTemplates.json!0x00000001",cityDescription:"AutomatedRegionTemplates.json!0x00000002",percentageBuildable:"90",coal:"0",rail:"false",resources:[]},1027:{cityName:"AutomatedRegionTemplates.json!0x00000003",coal:"3"}"#;
        let parsed = parse(
            js,
            &HashMap::from([(2, "介绍".into())]),
            &HashMap::from([(2, "Description".into())]),
        );
        assert_eq!(parsed[&1].description.as_deref(), Some("介绍"));
        assert_eq!(parsed[&1].description_en.as_deref(), Some("Description"));
        assert_eq!(parsed[&1].buildable, Some(90));
        assert_eq!(parsed[&1].coal, Some(0));
        assert_eq!(parsed[&1].rail, Some(false));
        assert_eq!(parsed[&3].rail, None);
        assert_eq!(parsed[&3].coal, Some(3));
    }
}
