//! Behavior binding processor (菜单/行为拆分的行为侧).
//!
//! Owns the ONLY allowed deltas of a behavior-declared build: Unit/Lot
//! education fields plus an optional locale title row.  Menu placement
//! resources (Menu2/icon/marquee) are frozen byte-for-byte — the menu is just
//! the entry, never a behavior carrier (s23 lesson: two independent chains).
//!
//! Requires `outputs.mode: "full"`: the build re-serializes the whole
//! reference snapshot, so the package is standalone and every scene resource
//! (models, materials, ground, trees, paths) is bit-identical to the donor.

use super::menu::{
    lot_group, ELEMENTARY_TEMPLATE, EDUCATION_BEHAVIOR_CATEGORY, HASH_PARENT, HASH_STUDENT_CAPACITY,
    HASH_TITLE, HASH_UNIT_CATEGORY, ICON_GROUP, ICON_TYPE, LOCALE_GROUP, LOCALE_TYPE, MARQUEE_TYPE,
    MENU_GROUP, UNIT_GROUP, UNIT_TEMPLATE_GROUP,
};
use super::property_edit::{encode, set_array, set_scalar};
use super::stages::BuildContext;
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropertyFile, PropType, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// Unit bundle-enable union rows (cEcoUnitEntry::FillFromProps reads all four,
/// then subtracts the exclude table 0x0A0209B2).
const HASH_BUNDLE_ENABLE_A2: u32 = 0x0A02_09A2;
const HASH_BUNDLE_ENABLE_A5: u32 = 0x0A02_09A5;
/// Lot UI board: education population map (`0x6A1785B4`); park residue
/// `0xD0621548` leaves the population map dark (s19→s20 lesson).
const HASH_LOT_BOARD: u32 = 0x0D79_70ED;
const EDUCATION_LOT_BOARD: u32 = 0x6A17_85B4;
/// Signature education bundle: bins + binding tables + enrollment rules all
/// live in the base-game scripts library and are referenced by key only.
const EDUCATION_SIGNATURE_BUNDLE: u32 = 0x6C04_2367;
/// Shared vanilla tree bundle kept from the donor so the frozen scene look
/// (dynamic park trees) survives the behavior overlay (s22 double-planting
/// lesson: never append it when the scene already carries static tree bins).
const DONOR_TREE_BUNDLE: u32 = 0xE769_0450;

/// `semantics.behavior` document: bind a minimal education loop onto the
/// donor identity without touching menu placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BehaviorSpec {
    /// Processor kind; currently only `education-minimal` is implemented.
    pub kind: String,
    /// Unit leaf capacity (`0x0AD7FAC9`, desk/seat statistic).
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub student_capacity: Option<u32>,
    /// Unit behavior category (`0x0AFE27B1`); defaults to education.
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub behavior_category_instance: Option<u32>,
    /// Unit parent template (`0x00B2CCCB`); defaults to the education template.
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub simulation_template_instance: Option<u32>,
    /// Bundle-enable union additions (default: the education signature bundle).
    #[serde(default, deserialize_with = "super::menu::de_hash_vec")]
    pub ensure_bundles: Vec<u32>,
    /// Lot board instance; defaults to the education board.
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub lot_board_instance: Option<u32>,
    /// Locale title row override — the single declared menu-layer drift.
    #[serde(default)]
    pub title_override: Option<String>,
}

impl BehaviorSpec {
    /// Parse from `SemanticsIr.behavior` (string-keyed JSON map).
    pub fn from_values(values: &BTreeMap<String, serde_json::Value>) -> Result<Self, String> {
        let value =
            serde_json::Value::Object(values.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let spec: BehaviorSpec = serde_json::from_value(value)
            .map_err(|error| format!("invalid behavior binding: {error}"))?;
        if spec.kind != "education-minimal" {
            return Err(format!(
                "unsupported behavior kind {:?} (only \"education-minimal\")",
                spec.kind
            ));
        }
        Ok(spec)
    }

    fn bundles(&self) -> Vec<u32> {
        if self.ensure_bundles.is_empty() {
            vec![EDUCATION_SIGNATURE_BUNDLE]
        } else {
            self.ensure_bundles.clone()
        }
    }
}

/// Behavior-declared build stage: patch Unit/Lot education fields and the
/// optional locale title row, then prove the menu layer is frozen.
pub(crate) fn apply(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let Some(spec) = ctx.plan.behavior.clone() else {
        return Ok(serde_json::Value::Null);
    };
    // Dalian identity convention: Menu2/Unit/Lot share the reference instance.
    let lot_ref = ctx
        .plan
        .intermediate
        .references
        .lot
        .as_ref()
        .ok_or("behavior binding requires reference.lot")?;
    let instance = lot_ref.instance;
    let unit_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: UNIT_GROUP,
        instance,
    };
    let lot_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: lot_group(),
        instance,
    };
    let menu_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: MENU_GROUP,
        instance,
    };

    // Menu-layer freeze baseline: capture the bytes the output must preserve.
    let frozen = freeze_baseline(ctx.resources.all(), &menu_id, instance)?;

    let mut changed = Vec::new();
    let unit = patch_unit(ctx, &unit_id, &spec, &mut changed)?;
    let lot = patch_lot(ctx, &lot_id, spec.lot_board_instance, &mut changed)?;
    let locale = match (&spec.title_override, &frozen.locale) {
        (Some(title), Some((locale_id, _))) => {
            let (id, row) = override_locale_title(ctx, locale_id, title)?;
            changed.push(row);
            Some(id)
        }
        (Some(_), None) => {
            return Err(
                "behavior titleOverride requires a locale table reachable from the donor Menu2"
                    .into(),
            )
        }
        (None, _) => None,
    };

    // Invariants, fail-closed: everything outside Unit/Lot/locale is frozen.
    frozen.verify(ctx.resources.all(), &locale)?;
    ensure_education_invariants(ctx, &unit_id, &spec)?;

    Ok(serde_json::json!({
        "kind": spec.kind,
        "instance": format!("{instance:08X}"),
        "changed": changed,
        "frozen": frozen.summary(),
        "bundles": spec.bundles().iter().map(|b| format!("{b:08X}")).collect::<Vec<_>>(),
    }))
}

/// The menu-layer bytes whose equality the stage guarantees.
struct FreezeBaseline {
    menu_id: ResourceId,
    menu_bytes: Vec<u8>,
    extras: Vec<(ResourceId, Vec<u8>)>,
    /// `(resource, original bytes)` of the locale table reachable from Menu2.
    locale: Option<(ResourceId, Vec<u8>)>,
}

fn freeze_baseline(
    resources: &super::connector::Resources,
    menu_id: &ResourceId,
    instance: u32,
) -> Result<FreezeBaseline, String> {
    let menu_bytes = resources
        .get(menu_id)
        .ok_or_else(|| format!("behavior binding requires donor Menu2 {menu_id} in the reference snapshot"))?
        .to_vec();
    let mut extras = Vec::new();
    for (type_id, group) in [(ICON_TYPE, ICON_GROUP), (MARQUEE_TYPE, ICON_GROUP)] {
        let id = ResourceId {
            type_id,
            group,
            instance,
        };
        if let Some(bytes) = resources.get(&id) {
            extras.push((id, bytes.to_vec()));
        }
    }
    let locale = donor_locale_id(resources, &menu_bytes)
        .map(|id| {
            let bytes = resources
                .get(&id)
                .ok_or_else(|| format!("donor Menu2 references locale {id} but it is absent"))?
                .to_vec();
            Ok::<_, String>((id, bytes))
        })
        .transpose()?;
    Ok(FreezeBaseline {
        menu_id: *menu_id,
        menu_bytes,
        extras,
        locale,
    })
}

impl FreezeBaseline {
    /// Fail if any menu-layer resource drifted.  The locale table is the one
    /// resource allowed to change, and only on the title row.
    fn verify(
        &self,
        resources: &super::connector::Resources,
        patched_locale: &Option<ResourceId>,
    ) -> Result<(), String> {
        let byte_equal = |id: &ResourceId, frozen: &[u8]| -> Result<(), String> {
            let current = resources
                .get(id)
                .ok_or_else(|| format!("frozen resource {id} disappeared during behavior bind"))?;
            if current.as_slice() != frozen {
                return Err(format!(
                    "invariant MENU_FREEZE violated: {id} changed bytes (only Unit/Lot/locale title may drift)"
                ));
            }
            Ok(())
        };
        byte_equal(&self.menu_id, &self.menu_bytes)?;
        for (id, bytes) in &self.extras {
            byte_equal(id, bytes)?;
        }
        if let Some((locale_id, original)) = &self.locale {
            let current = resources
                .get(locale_id)
                .ok_or_else(|| format!("locale table {locale_id} disappeared during behavior bind"))?;
            if Some(locale_id) != patched_locale.as_ref() {
                if current.as_slice() != original.as_slice() {
                    return Err(format!(
                        "invariant MENU_FREEZE violated: locale {locale_id} changed without titleOverride"
                    ));
                }
                return Ok(());
            }
            let before = locale_rows(original)?;
            let after = locale_rows(current)?;
            for (row, value) in &before {
                if row == "0x00000001" {
                    continue;
                }
                if after.get(row) != Some(value) {
                    return Err(format!(
                        "invariant MENU_FREEZE violated: locale row {row} drifted (only 0x00000001 may change)"
                    ));
                }
            }
            if after.len() != before.len() {
                return Err("invariant MENU_FREEZE violated: locale row set changed".into());
            }
        }
        Ok(())
    }

    fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "menu2": self.menu_id.to_string(),
            "frozenResources": 1 + self.extras.len() + usize::from(self.locale.is_some()),
        })
    }
}

/// The locale table the donor Menu2 title Text points at (`table_id` carries
/// the table instance; `instance_id` is the row).
fn donor_locale_id(resources: &super::connector::Resources, menu_bytes: &[u8]) -> Option<ResourceId> {
    let menu = PropertyFile::parse(menu_bytes).ok()?;
    let text = menu.get(HASH_TITLE)?;
    let Kind::Array(values) = &text.kind else {
        return None;
    };
    let Value::Text(text) = values.first()? else {
        return None;
    };
    let id = ResourceId {
        type_id: LOCALE_TYPE,
        group: LOCALE_GROUP,
        instance: text.table_id,
    };
    resources.contains_key(&id).then_some(id)
}

/// Parse a locale table (BOM + CRLF JSON) into row-keyed strings.
fn locale_rows(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let text = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .ok_or("locale table is missing the UTF-8 BOM")?;
    let value: serde_json::Value =
        serde_json::from_slice(text).map_err(|error| format!("locale table parse: {error}"))?;
    let object = value
        .as_object()
        .ok_or("locale table is not a JSON object")?;
    Ok(object
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_default()))
        .collect())
}

/// Replace ONLY the title row inside the locale table, keeping every other
/// byte (line layout, CRLF, rows) identical.
fn override_locale_title(
    ctx: &mut BuildContext,
    locale_id: &ResourceId,
    title: &str,
) -> Result<(ResourceId, serde_json::Value), String> {
    let bytes = ctx
        .resources
        .get(locale_id)
        .ok_or("locale table vanished")?
        .to_vec();
    let text = std::str::from_utf8(
        bytes
            .strip_prefix(&[0xEF, 0xBB, 0xBF])
            .ok_or("locale table is missing the UTF-8 BOM")?,
    )
    .map_err(|error| format!("locale table is not UTF-8: {error}"))?;
    let needle = "    \"0x00000001\" :";
    let line = format!(
        "{needle} {}",
        serde_json::to_string(title.trim()).unwrap_or_default()
    );
    let mut replaced = false;
    let updated = text
        .lines()
        .map(|row| {
            if row.starts_with(needle) {
                replaced = true;
                // Preserve the row's own syntax: a trailing comma must survive
                // or the following rows fail JSON parsing.
                match row.trim_end().ends_with(',') {
                    true => format!("{line},"),
                    false => line.clone(),
                }
            } else {
                row.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    if !replaced {
        return Err("locale table has no title row 0x00000001".into());
    }
    let mut out = Vec::with_capacity(updated.len() + 3);
    out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    out.extend_from_slice(updated.as_bytes());
    ctx.resources.insert(*locale_id, out);
    Ok((
        *locale_id,
        serde_json::json!({
            "resource": locale_id.to_string(),
            "row": "0x00000001",
            "title": title.trim(),
        }),
    ))
}

fn patch_unit(
    ctx: &mut BuildContext,
    unit_id: &ResourceId,
    spec: &BehaviorSpec,
    changed: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    let bytes = ctx.resources.get(unit_id).ok_or_else(|| {
        format!("behavior binding requires donor Unit {unit_id} in the reference snapshot")
    })?;
    let mut unit = PropertyFile::parse(bytes).map_err(|error| format!("donor Unit parse: {error}"))?;

    // Parent template (0x00B2CCCB): the education template chain supplies the
    // utility bundles.  Vanilla encoding is TYPE-FUL (`00B1B104:48E0C000:<tpl>`,
    // fire-station dissection 2026-10-20); s14 carried group 40E0C000, which
    // the template walker cannot resolve — this overlay repairs it.
    let template = spec.simulation_template_instance.unwrap_or(ELEMENTARY_TEMPLATE);
    set_scalar(
        &mut unit,
        HASH_PARENT,
        PropType::Key,
        Value::Key(Key {
            type_id: PROPERTY_TYPE,
            group: UNIT_TEMPLATE_GROUP,
            instance: template,
        }),
    );
    changed.push(serde_json::json!({
        "resource": unit_id.to_string(),
        "field": format!("{HASH_PARENT:08X}"),
        "value": format!("{template:08X}"),
    }));

    // Behavior category (0x0AFE27B1): engine-read education selector.
    let category = spec
        .behavior_category_instance
        .unwrap_or(EDUCATION_BEHAVIOR_CATEGORY);
    set_scalar(
        &mut unit,
        HASH_UNIT_CATEGORY,
        PropType::Key,
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: category,
        }),
    );
    changed.push(serde_json::json!({
        "resource": unit_id.to_string(),
        "field": format!("{HASH_UNIT_CATEGORY:08X}"),
        "value": format!("{category:08X}"),
    }));

    // Bundle-enable union: add the signature education bundle without touching
    // the donor's existing entries (E7690450 park trees stay = frozen look).
    for bundle in spec.bundles() {
        let already = [HASH_BUNDLE_ENABLE_A2, HASH_BUNDLE_ENABLE_A5].into_iter().any(|hash| {
            unit.get(hash).is_some_and(|property| {
                matches!(
                    &property.kind,
                    Kind::Array(values) if values.iter().any(|v| matches!(v,
                        Value::Key(k) if k.instance == bundle
                    ))
                )
            })
        });
        if already {
            changed.push(serde_json::json!({
                "resource": unit_id.to_string(),
                "field": "bundle-union",
                "value": format!("{bundle:08X}"),
                "note": "already enabled",
            }));
            continue;
        }
        match unit.values.iter_mut().find(|p| p.hash == HASH_BUNDLE_ENABLE_A5) {
            Some(property) => {
                if let Kind::Array(values) = &mut property.kind {
                    values.push(Value::Key(Key {
                        type_id: 0,
                        group: 0,
                        instance: bundle,
                    }));
                }
            }
            None => set_array(
                &mut unit,
                HASH_BUNDLE_ENABLE_A5,
                PropType::Key,
                vec![Value::Key(Key {
                    type_id: 0,
                    group: 0,
                    instance: bundle,
                })],
            ),
        }
        changed.push(serde_json::json!({
            "resource": unit_id.to_string(),
            "field": format!("{HASH_BUNDLE_ENABLE_A5:08X}"),
            "value": format!("{bundle:08X}"),
        }));
    }

    if let Some(capacity) = spec.student_capacity {
        set_scalar(
            &mut unit,
            HASH_STUDENT_CAPACITY,
            PropType::Float,
            Value::Float(capacity as f32),
        );
        changed.push(serde_json::json!({
            "resource": unit_id.to_string(),
            "field": format!("{HASH_STUDENT_CAPACITY:08X}"),
            "value": capacity,
        }));
    }
    unit.claimed_count = unit.values.len() as u32;
    let encoded = encode(&unit)?;
    ctx.resources.insert(*unit_id, encoded);
    Ok(())
}

fn patch_lot(
    ctx: &mut BuildContext,
    lot_id: &ResourceId,
    board: Option<u32>,
    changed: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    let bytes = ctx.resources.get(lot_id).ok_or_else(|| {
        format!("behavior binding requires donor Lot {lot_id} in the reference snapshot")
    })?;
    let mut lot = PropertyFile::parse(bytes).map_err(|error| format!("donor Lot parse: {error}"))?;
    let board = board.unwrap_or(EDUCATION_LOT_BOARD);
    set_scalar(
        &mut lot,
        HASH_LOT_BOARD,
        PropType::Key,
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: board,
        }),
    );
    changed.push(serde_json::json!({
        "resource": lot_id.to_string(),
        "field": format!("{HASH_LOT_BOARD:08X}"),
        "value": format!("{board:08X}"),
    }));
    lot.claimed_count = lot.values.len() as u32;
    let encoded = encode(&lot)?;
    ctx.resources.insert(*lot_id, encoded);
    Ok(())
}

/// Post-patch invariants: the education chain must resolve, fail-closed.
fn ensure_education_invariants(
    ctx: &BuildContext,
    unit_id: &ResourceId,
    spec: &BehaviorSpec,
) -> Result<(), String> {
    let bytes = ctx
        .resources
        .get(unit_id)
        .ok_or("unit vanished during behavior bind")?;
    let unit = PropertyFile::parse(bytes).map_err(|error| format!("unit reparse: {error}"))?;
    let template = spec.simulation_template_instance.unwrap_or(ELEMENTARY_TEMPLATE);
    let category = spec
        .behavior_category_instance
        .unwrap_or(EDUCATION_BEHAVIOR_CATEGORY);
    let bundle_enabled = |bundle: u32| {
        [HASH_BUNDLE_ENABLE_A2, HASH_BUNDLE_ENABLE_A5].into_iter().any(|hash| {
            unit.get(hash).is_some_and(|property| {
                matches!(
                    &property.kind,
                    Kind::Array(values) if values.iter().any(|v| matches!(v,
                        Value::Key(k) if k.instance == bundle
                    ))
                )
            })
        })
    };
    let parent_ok = unit.get(HASH_PARENT).is_some_and(|property| {
        matches!(
            &property.kind,
            Kind::Scalar(Value::Key(key))
                if key.type_id == PROPERTY_TYPE
                    && key.group == UNIT_TEMPLATE_GROUP
                    && key.instance == template
        )
    });
    if !parent_ok {
        return Err(format!(
            "invariant EDUCATION_PARENT violated: Unit {HASH_PARENT:08X} is not {template:08X}"
        ));
    }
    let category_ok = unit.get(HASH_UNIT_CATEGORY).is_some_and(|property| {
        matches!(
            &property.kind,
            Kind::Scalar(Value::Key(key)) if key.instance == category
        )
    });
    if !category_ok {
        return Err(format!(
            "invariant EDUCATION_CATEGORY violated: Unit {HASH_UNIT_CATEGORY:08X} is not {category:08X}"
        ));
    }
    if !spec.bundles().iter().all(|bundle| bundle_enabled(*bundle)) {
        return Err(
            "invariant EDUCATION_BUNDLE violated: signature bundle missing from the enable union"
                .into(),
        );
    }
    // The donor tree bundle must survive: losing it loses the frozen look.
    if !bundle_enabled(DONOR_TREE_BUNDLE) {
        return Err(
            "invariant DONOR_TREE_BUNDLE violated: E7690450 disappeared from the enable union"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_from_json(json: &str) -> BehaviorSpec {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let map: BTreeMap<String, serde_json::Value> = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        BehaviorSpec::from_values(&map).unwrap()
    }

    #[test]
    fn spec_defaults_carry_the_education_chain() {
        let spec = spec_from_json(r#"{"kind":"education-minimal","studentCapacity":250}"#);
        assert_eq!(spec.bundles(), vec![EDUCATION_SIGNATURE_BUNDLE]);
        assert_eq!(spec.student_capacity, Some(250));
        assert_eq!(spec.title_override, None);
    }

    #[test]
    fn spec_rejects_unknown_kinds_and_fields() {
        let value: serde_json::Value = serde_json::from_str(r#"{"kind":"power-plant"}"#).unwrap();
        let map: BTreeMap<String, serde_json::Value> = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert!(BehaviorSpec::from_values(&map).is_err());
        assert!(serde_json::from_value::<BehaviorSpec>(serde_json::json!({
            "kind": "education-minimal",
            "unknownField": 1
        }))
        .is_err());
    }
}
