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
    lot_group, ELEMENTARY_TEMPLATE, EDUCATION_BEHAVIOR_CATEGORY, HASH_MENU_UNIT, HASH_PARENT,
    HASH_STUDENT_CAPACITY, HASH_TITLE, HASH_UNIT_CATEGORY, ICON_GROUP, ICON_TYPE, LOCALE_GROUP,
    LOCALE_TYPE, MARQUEE_TYPE, MENU_GROUP, UNIT_GROUP, UNIT_TEMPLATE_GROUP,
};
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use super::stages::BuildContext;
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropertyFile, PropType, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// Menu2 info-panel stat rows (school schedule/capacity vs park visitors).
const HASH_STAT_TEMPLATE: u32 = 0x0EB1_FC05;
const HASH_STAT_PARAMS: u32 = 0x0EB1_FC22;
/// Menu2 uiCategories: the container membership edges (park + education).
const HASH_UI_CATEGORIES: u32 = 0x0DB9_FC63;
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
/// Vanilla Pedestrians PathEntry (filter E0CD3122): the walking-department
/// path type.  The park-donor lot ships ONE walkway typed 03106C2E (Cars And
/// Crap, shadowed by the scripts copy), so student agents have no walkable
/// route - the exact "zero arrivals" defect.
const PEDESTRIAN_PATH_ENTRY: u32 = 0x418A_18B1;
/// Lot path six-piece hashes (CreatePathsFromProps.c).
const HASH_PATH_ENTRIES: u32 = 0x0CC8_FE7A;
const HASH_PATH_SEGMENTS: u32 = 0x0CAA_6841;
const HASH_PATH_INDICES: u32 = 0x0CAA_6832;
const HASH_PATH_SCOPE: u32 = 0x0D9A_4E54;
const HASH_PATH_ROAD_CONNECT: u32 = 0x0D9A_A2BB;
/// Vanilla Cars-And-Crap PathEntry: the garbage/sewage service network
/// (carriers EB91A84C et al. declare filter 979CC22E).  The vanilla college
/// pairs it with a Pedestrians path on the same lot.
const SERVICE_PATH_ENTRY: u32 = 0x0310_6C2E;
const PATH_SCOPE_LOCAL: u32 = 0xF44F_784A;
const PATH_ROAD_CONNECT_NONE: u32 = 0xC186_DA37;
/// Unit leaf: internal path-entry reference (SC/GetUnitInternalPathEntries.c);
/// the vanilla school carries Key 2C7C7986, the park donor does not.
const HASH_INTERNAL_PATH_REF: u32 = 0x0DA6_9D6F;
const VANILLA_INTERNAL_PATH_REF: u32 = 0x2C7C_7986;
/// Unit leaf bundle-exclude table (cEcoUnitEntry::FillFromProps subtracts it
/// from the enable union; the park donor excludes the economy bundle).
const HASH_EXCLUDE_TABLE: u32 = 0x0A02_09B2;

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
    /// Bundles to REMOVE from the enable union (all four rows).  Identity
    /// swaps need this: the community-college signature bundle is 6C042366,
    /// the grade-school one 6C042367 must come off or both sink sets attach.
    #[serde(default, deserialize_with = "super::menu::de_hash_vec")]
    pub remove_bundles: Vec<u32>,
    /// Unit leaf fields to remove entirely (e.g. the seat-capacity UI value
    /// 0x0AD7FAC9, which vanilla colleges do not carry).
    #[serde(default, deserialize_with = "super::menu::de_hash_vec")]
    pub remove_fields: Vec<u32>,
    /// Lot board instance; defaults to the education board.
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub lot_board_instance: Option<u32>,
    /// Locale title row override — a declared menu-layer drift.
    #[serde(default)]
    pub title_override: Option<String>,
    /// Presentation donor package (project-relative): a vanilla education
    /// menu/unit whose stat rows, uiCategories and tool icon are cloned onto
    /// the frozen identity so the building presents as education.
    #[serde(default)]
    pub presentation_package: Option<String>,
    /// The donor unit instance; its menu is found via the menuUnit edge.
    #[serde(default, deserialize_with = "super::menu::de_hash")]
    pub presentation_unit_instance: Option<u32>,
    /// Extend uiCategories (0x0DB9FC63) with the donor's entries (union; the
    /// existing park container entry stays — the park menu remains the entry).
    #[serde(default)]
    pub adopt_ui_categories: bool,
    /// Replace the info-panel stat rows (0x0EB1FC05/0x0EB1FC22) with the
    /// donor's (school schedule + student capacity instead of park visitors).
    #[serde(default)]
    pub adopt_stat_rows: bool,
    /// Replace the tool icon bytes with the donor unit's icon.
    #[serde(default)]
    pub adopt_icon: bool,
    /// Retype every lot path segment to the vanilla Pedestrians PathEntry so
    /// student agents gain a walkable route onto the lot (walking-department
    /// minimal recipe: "仅 Pedestrians 路").  Fails closed when the lot's
    /// path six-piece structure is not internally consistent.
    #[serde(default)]
    pub ensure_walk_path: bool,
    /// Ensure the Unit leaf's internal path-entry reference (0x0DA69D6F =
    /// 2C7C7986), present on vanilla schools and absent on the park donor.
    #[serde(default)]
    pub ensure_internal_path_ref: bool,
    /// Append a second lot path typed Cars-And-Crap (03106C2E, the garbage/
    /// sewage service network) over the walkway's endpoints.  The vanilla
    /// college ships exactly this pair: a short service path + a pedestrian
    /// path.  Without it, garbage trucks have no route onto the lot even
    /// though trash props and garbage production exist.
    #[serde(default)]
    pub ensure_service_path: bool,
    /// Bundle ids to remove from the leaf exclude table (0x0A0209B2).  The
    /// park donor excludes the economy bundle 30645625, which carries the
    /// commuter-car bins (0AB4DFE1/E2/E3) that drive the percent-fill parking
    /// draw — excluding it keeps the parking lot permanently empty.
    #[serde(default, deserialize_with = "super::menu::de_hash_vec")]
    pub remove_excluded_bundles: Vec<u32>,
    /// Unit leaf float fields to set (hex hash → value).  Carries the college
    /// leaf's per-bin initial amounts (0x0B9880C6 family) and sink capacity
    /// (0x0AD93A77) that vanilla colleges ship and park donors lack.
    #[serde(default)]
    pub set_float_fields: BTreeMap<String, f64>,
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
    let lot = patch_lot(
        ctx,
        &lot_id,
        spec.lot_board_instance,
        spec.ensure_walk_path,
        spec.ensure_service_path,
        &mut changed,
    )?;
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

    // Presentation layer: clone donor stat rows / uiCategories / icon so the
    // building presents as education.  These are DECLARED menu-layer drifts;
    // the freeze below enforces that nothing beyond them moved.
    let menu2_patched = if presentation_requested(&spec) {
        let presentation = ctx.presentation_resources.clone().ok_or(
            "behavior presentation fields require presentation_package to be loadable",
        )?;
        let (menu2_declared, icon_override) =
            apply_presentation(ctx, &presentation, &spec, instance, &mut changed)?;
        Some((menu2_declared, icon_override))
    } else {
        None
    };

    // Invariants, fail-closed: everything outside the declared set is frozen.
    frozen.verify(ctx.resources.all(), &locale, menu2_patched.as_ref())?;
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

fn presentation_requested(spec: &BehaviorSpec) -> bool {
    spec.presentation_package.is_some()
        && (spec.adopt_ui_categories || spec.adopt_stat_rows || spec.adopt_icon)
}

/// Clone the declared presentation pieces from the vanilla education donor.
/// Returns the set of Menu2 hashes allowed to drift plus whether the icon
/// resource may differ.
fn apply_presentation(
    ctx: &mut BuildContext,
    presentation: &super::connector::Resources,
    spec: &BehaviorSpec,
    instance: u32,
    changed: &mut Vec<serde_json::Value>,
) -> Result<(Vec<u32>, bool), String> {
    let unit_instance = spec.presentation_unit_instance.ok_or(
        "presentation_package requires presentationUnitInstance",
    )?;
    // Find the donor menu whose menuUnit edge points at the donor unit.
    let donor_menu = presentation
        .iter()
        .find(|(id, bytes)| {
            id.type_id == PROPERTY_TYPE
                && id.group == MENU_GROUP
                && PropertyFile::parse(bytes).ok().is_some_and(|menu| {
                    matches!(
                        menu.get(HASH_MENU_UNIT).map(|p| &p.kind),
                        Some(Kind::Scalar(Value::Key(k))) if k.instance == unit_instance
                    )
                })
        })
        .ok_or_else(|| {
            format!(
                "presentation donor has no menu pointing at unit {unit_instance:08X}"
            )
        })?;
    let donor = PropertyFile::parse(donor_menu.1)
        .map_err(|error| format!("donor menu parse: {error}"))?;
    let mut declared = Vec::new();
    let mut icon_override = false;

    let menu_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: MENU_GROUP,
        instance,
    };
    let bytes = ctx
        .resources
        .get(&menu_id)
        .ok_or_else(|| format!("donor Menu2 {menu_id} missing from the snapshot"))?
        .to_vec();
    let mut menu = PropertyFile::parse(&bytes)
        .map_err(|error| format!("Menu2 parse: {error}"))?;

    // a) stat rows: school schedule + capacity panel replaces park visitors.
    if spec.adopt_stat_rows {
        for hash in [HASH_STAT_TEMPLATE, HASH_STAT_PARAMS] {
            let Some(donor_property) = donor.get(hash) else {
                return Err(format!(
                    "donor menu has no stat field {hash:08X} to adopt"
                ));
            };
            if let Kind::Array(values) = &donor_property.kind {
                set_array_with_flags(
                    &mut menu,
                    hash,
                    donor_property.prop_type,
                    values.clone(),
                    donor_property.encoding.flags,
                );
                declared.push(hash);
                changed.push(serde_json::json!({
                    "resource": menu_id.to_string(),
                    "field": format!("{hash:08X}"),
                    "value": "donor stat rows",
                }));
            }
        }
    }

    // b) uiCategories union: education board + category join; the park
    // container entry stays so the park menu remains the entry.
    if spec.adopt_ui_categories {
        let Some(Kind::Array(donor_values)) =
            donor.get(HASH_UI_CATEGORIES).map(|p| &p.kind)
        else {
            return Err("donor menu has no uiCategories".into());
        };
        let existing = menu.get(HASH_UI_CATEGORIES).map(|p| &p.kind);
        let mut values = match existing {
            Some(Kind::Array(values)) => values.clone(),
            _ => Vec::new(),
        };
        let mut added = Vec::new();
        for value in donor_values {
            if !values.contains(value) {
                added.push(render_key(value));
                values.push(value.clone());
            }
        }
        let flags = menu
            .get(HASH_UI_CATEGORIES)
            .map(|p| p.encoding.flags)
            .unwrap_or(0x809C);
        set_array_with_flags(&mut menu, HASH_UI_CATEGORIES, PropType::Key, values, flags);
        declared.push(HASH_UI_CATEGORIES);
        changed.push(serde_json::json!({
            "resource": menu_id.to_string(),
            "field": format!("{HASH_UI_CATEGORIES:08X}"),
            "value": format!("+{}", added.join("+")),
        }));
    }

    menu.claimed_count = menu.values.len() as u32;
    let encoded = encode(&menu)?;
    ctx.resources.insert(menu_id, encoded);

    // c) icon bytes: donor keeps its icon at instance == unit instance.
    if spec.adopt_icon {
        let donor_icon_id = ResourceId {
            type_id: ICON_TYPE,
            group: ICON_GROUP,
            instance: unit_instance,
        };
        let icon_bytes = presentation
            .get(&donor_icon_id)
            .ok_or_else(|| format!("donor icon {donor_icon_id} missing"))?
            .to_vec();
        let our_icon = ResourceId {
            type_id: ICON_TYPE,
            group: ICON_GROUP,
            instance,
        };
        if ctx.resources.get(&our_icon).is_none() {
            return Err(format!("our icon {our_icon} missing from the snapshot"));
        }
        ctx.resources.insert(our_icon, icon_bytes);
        icon_override = true;
        changed.push(serde_json::json!({
            "resource": our_icon.to_string(),
            "field": "icon-bytes",
            "value": format!("donor {donor_icon_id}"),
        }));
    }
    Ok((declared, icon_override))
}

fn render_key(value: &Value) -> String {
    match value {
        Value::Key(key) => format!("{:08X}", key.instance),
        other => format!("{other:?}"),
    }
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
    /// Fail if any menu-layer resource drifted beyond the declared set.  The
    /// locale table may change on the title row only; Menu2 only on the
    /// declared stat/uiCategories fields (field-level, canonical re-encoding
    /// of untouched fields must stay value-identical); the icon only when
    /// `adopt_icon` cloned donor bytes onto it.
    fn verify(
        &self,
        resources: &super::connector::Resources,
        patched_locale: &Option<ResourceId>,
        menu2_drift: Option<&(Vec<u32>, bool)>,
    ) -> Result<(), String> {
        let byte_equal = |id: &ResourceId, frozen: &[u8]| -> Result<(), String> {
            let current = resources
                .get(id)
                .ok_or_else(|| format!("frozen resource {id} disappeared during behavior bind"))?;
            if current.as_slice() != frozen {
                return Err(format!(
                    "invariant MENU_FREEZE violated: {id} changed bytes (only declared drift may touch it)"
                ));
            }
            Ok(())
        };
        match menu2_drift {
            Some((declared_fields, _)) => {
                let baseline =
                    PropertyFile::parse(&self.menu_bytes).map_err(|e| e.to_string())?;
                let current = resources
                    .get(&self.menu_id)
                    .ok_or_else(|| format!("frozen resource {} disappeared", self.menu_id))?;
                let current = PropertyFile::parse(current).map_err(|e| e.to_string())?;
                menu_fields_undeclared_drift(&baseline, &current, declared_fields)?;
            }
            None => byte_equal(&self.menu_id, &self.menu_bytes)?,
        }
        let icon_exempt =
            menu2_drift.map(|(_, icon)| *icon).unwrap_or(false);
        for (id, bytes) in &self.extras {
            if icon_exempt && id.type_id == ICON_TYPE {
                continue;
            }
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

/// Field-level menu freeze: every property outside `declared` must keep its
/// type, encoding and values (canonical encoding may move bytes, not values).
fn menu_fields_undeclared_drift(
    baseline: &PropertyFile,
    current: &PropertyFile,
    declared: &[u32],
) -> Result<(), String> {
    let render = |file: &PropertyFile, hash: u32| -> Option<String> {
        let property = file.get(hash)?;
        Some(format!(
            "{:?}|{:?}|{:?}",
            property.prop_type, property.encoding, property.kind
        ))
    };
    let mut hashes: Vec<u32> = baseline
        .values
        .iter()
        .map(|p| p.hash)
        .chain(current.values.iter().map(|p| p.hash))
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    for hash in hashes {
        if declared.contains(&hash) {
            continue;
        }
        let before = render(baseline, hash);
        let after = render(current, hash);
        if before != after {
            return Err(format!(
                "invariant MENU_FREEZE violated: Menu2 field {hash:08X} drifted outside the declared set"
            ));
        }
    }
    Ok(())
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

    // Identity swap removals run FIRST: drop whole leaf fields and pull the
    // named bundles out of all four enable rows before anything is added.
    if !spec.remove_fields.is_empty() {
        let removed: Vec<String> = spec
            .remove_fields
            .iter()
            .map(|hash| format!("{hash:08X}"))
            .collect();
        unit.values
            .retain(|property| !spec.remove_fields.contains(&property.hash));
        changed.push(serde_json::json!({
            "resource": unit_id.to_string(),
            "field": "remove-fields",
            "value": removed,
        }));
    }
    if !spec.remove_excluded_bundles.is_empty() {
        let excluded = unit.values.iter_mut().find(|p| p.hash == HASH_EXCLUDE_TABLE);
        if let Some(property) = excluded {
            if let Kind::Array(values) = &mut property.kind {
                let before = values.len();
                values.retain(|v| !matches!(v, Value::Key(k) if spec.remove_excluded_bundles.contains(&k.instance)));
                changed.push(serde_json::json!({
                    "resource": unit_id.to_string(),
                    "field": format!("{HASH_EXCLUDE_TABLE:08X}"),
                    "value": format!("{} -> {}", before, values.len()),
                }));
            }
        }
    }
    for (hash_text, value) in &spec.set_float_fields {
        let Ok(hash) = u32::from_str_radix(hash_text.trim_start_matches("0x"), 16) else {
            return Err(format!("setFloatFields key {hash_text:?} is not hex"));
        };
        set_scalar(&mut unit, hash, PropType::Float, Value::Float(*value as f32));
        changed.push(serde_json::json!({
            "resource": unit_id.to_string(),
            "field": format!("{hash:08X}"),
            "value": value,
        }));
    }
    for bundle in &spec.remove_bundles {
        let mut removed_from = Vec::new();
        for hash in [HASH_BUNDLE_ENABLE_A2, 0x0A02_09A3, 0x0A02_09A4, HASH_BUNDLE_ENABLE_A5] {
            if let Some(property) = unit.values.iter_mut().find(|p| p.hash == hash) {
                if let Kind::Array(values) = &mut property.kind {
                    let before = values.len();
                    values.retain(|v| !matches!(v, Value::Key(k) if k.instance == *bundle));
                    if values.len() != before {
                        removed_from.push(format!("{hash:08X}"));
                    }
                }
            }
        }
        if !removed_from.is_empty() {
            changed.push(serde_json::json!({
                "resource": unit_id.to_string(),
                "field": "remove-bundles",
                "value": format!("{bundle:08X}"),
                "rows": removed_from,
            }));
        }
    }

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
    // Internal path-entry reference: vanilla schools carry it (door finding).
    if spec.ensure_internal_path_ref {
        set_scalar(
            &mut unit,
            HASH_INTERNAL_PATH_REF,
            PropType::Key,
            Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: VANILLA_INTERNAL_PATH_REF,
            }),
        );
        changed.push(serde_json::json!({
            "resource": unit_id.to_string(),
            "field": format!("{HASH_INTERNAL_PATH_REF:08X}"),
            "value": format!("{VANILLA_INTERNAL_PATH_REF:08X}"),
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
    ensure_walk_path: bool,
    ensure_service_path: bool,
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
    // Walking department: every path segment must be typed Pedestrians or
    // student agents have no arrival route.  Fail closed unless the six-piece
    // path structure is internally consistent (counts must line up:
    // entries == segments/2 == scope == roadConnect, at least one path).
    if ensure_walk_path {
        let count = |hash: u32| match lot.get(hash).map(|p| &p.kind) {
            Some(Kind::Array(values)) => values.len(),
            _ => 0,
        };
        let (entries, segments, scope, connect) = (
            count(HASH_PATH_ENTRIES),
            count(HASH_PATH_SEGMENTS),
            count(HASH_PATH_SCOPE),
            count(HASH_PATH_ROAD_CONNECT),
        );
        let paths = if segments % 2 == 0
            && entries == segments / 2
            && scope == entries
            && connect == entries
            && entries > 0
        {
            entries
        } else {
            return Err(
                "invariant WALK_PATH violated: lot path six-piece counts are inconsistent \
                 (entries/segments/scope/connect do not line up)"
                    .into(),
            );
        };
        let flags = lot
            .get(HASH_PATH_ENTRIES)
            .map(|p| p.encoding.flags)
            .unwrap_or(0x809C);
        set_array_with_flags(
            &mut lot,
            HASH_PATH_ENTRIES,
            PropType::Key,
            vec![
                Value::Key(Key {
                    type_id: PROPERTY_TYPE,
                    group: 0x40E0_C400,
                    instance: PEDESTRIAN_PATH_ENTRY,
                });
                paths
            ],
            flags,
        );
        changed.push(serde_json::json!({
            "resource": lot_id.to_string(),
            "field": format!("{HASH_PATH_ENTRIES:08X}"),
            "value": format!("{paths} path(s) -> Pedestrians {PEDESTRIAN_PATH_ENTRY:08X}"),
        }));
    }
    // Service department: append a Cars-And-Crap path over the walkway's
    // endpoints (the vanilla-college pattern: short service path + pedestrian
    // path) so garbage/sewage trucks gain a route onto the lot.
    if ensure_service_path {
        let array = |hash: u32| -> Option<Vec<Value>> {
            match lot.get(hash).map(|p| &p.kind) {
                Some(Kind::Array(values)) => Some(values.clone()),
                _ => None,
            }
        };
        let (indices, segments, mut entries, mut scope, mut connect) = (
            array(HASH_PATH_INDICES).ok_or("invariant SERVICE_PATH violated: no path indices")?,
            array(HASH_PATH_SEGMENTS).ok_or("invariant SERVICE_PATH violated: no path segments")?,
            array(HASH_PATH_ENTRIES).ok_or("invariant SERVICE_PATH violated: no path entries")?,
            array(HASH_PATH_SCOPE).ok_or("invariant SERVICE_PATH violated: no path scope")?,
            array(HASH_PATH_ROAD_CONNECT)
                .ok_or("invariant SERVICE_PATH violated: no path road connect")?,
        );
        let walk_points = match (indices.first(), indices.last()) {
            (Some(first), Some(last)) => (first.clone(), last.clone()),
            _ => return Err("invariant SERVICE_PATH violated: empty path indices".into()),
        };
        let paths = segments.len() / 2;
        if entries.len() != paths || scope.len() != paths || connect.len() != paths {
            return Err(
                "invariant SERVICE_PATH violated: path counts do not line up before appending"
                    .into(),
            );
        }
        // New path = points[0] -> points[last]: two fresh index slots reuse the
        // existing point pool, so no new geometry is authored.
        let first_slot = indices.len() as i32;
        let service_indices = [walk_points.0, walk_points.1];
        let mut next_indices = indices;
        next_indices.extend(service_indices.iter().cloned());
        let mut next_segments = segments;
        next_segments.push(Value::Int32(first_slot));
        next_segments.push(Value::Int32(first_slot + 1));
        entries.push(Value::Key(Key {
            type_id: PROPERTY_TYPE,
            group: 0x40E0_C400,
            instance: SERVICE_PATH_ENTRY,
        }));
        scope.push(Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: PATH_SCOPE_LOCAL,
        }));
        connect.push(Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: PATH_ROAD_CONNECT_NONE,
        }));
        // Precompute flags before the mutable passes (borrow-order hygiene).
        let flags_indices = lot.get(HASH_PATH_INDICES).map(|p| p.encoding.flags).unwrap_or(0x809C);
        let flags_segments = lot.get(HASH_PATH_SEGMENTS).map(|p| p.encoding.flags).unwrap_or(0x809C);
        let flags_entries = lot.get(HASH_PATH_ENTRIES).map(|p| p.encoding.flags).unwrap_or(0x809C);
        let flags_scope = lot.get(HASH_PATH_SCOPE).map(|p| p.encoding.flags).unwrap_or(0x809C);
        let flags_connect = lot.get(HASH_PATH_ROAD_CONNECT).map(|p| p.encoding.flags).unwrap_or(0x809C);
        set_array_with_flags(&mut lot, HASH_PATH_INDICES, PropType::Int32, next_indices, flags_indices);
        set_array_with_flags(&mut lot, HASH_PATH_SEGMENTS, PropType::Int32, next_segments, flags_segments);
        set_array_with_flags(&mut lot, HASH_PATH_ENTRIES, PropType::Key, entries, flags_entries);
        set_array_with_flags(&mut lot, HASH_PATH_SCOPE, PropType::Key, scope, flags_scope);
        set_array_with_flags(&mut lot, HASH_PATH_ROAD_CONNECT, PropType::Key, connect, flags_connect);
        changed.push(serde_json::json!({
            "resource": lot_id.to_string(),
            "field": "service-path",
            "value": format!("+Cars-And-Crap {SERVICE_PATH_ENTRY:08X} (points {first_slot}->{})", first_slot + 1),
        }));
    }
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
    // Removed bundles must be gone from every row, or both sink sets attach.
    for bundle in &spec.remove_bundles {
        if bundle_enabled(*bundle) {
            return Err(format!(
                "invariant BUNDLE_REMOVAL violated: {bundle:08X} still enabled after removal"
            ));
        }
    }
    // Removed leaf fields must not survive the swap.
    for hash in &spec.remove_fields {
        if unit.get(*hash).is_some() {
            return Err(format!(
                "invariant FIELD_REMOVAL violated: {hash:08X} still present after removal"
            ));
        }
    }
    // Excluded bundles must actually leave the exclude table (the commuter-car
    // bins live behind this gate).
    for bundle in &spec.remove_excluded_bundles {
        let still_excluded = unit.get(HASH_EXCLUDE_TABLE).is_some_and(|property| {
            matches!(
                &property.kind,
                Kind::Array(values) if values.iter().any(|v| matches!(v,
                    Value::Key(k) if k.instance == *bundle
                ))
            )
        });
        if still_excluded {
            return Err(format!(
                "invariant EXCLUDE_REMOVAL violated: {bundle:08X} still excluded"
            ));
        }
    }
    // Set fields must exist with the requested value (fail on silent drops).
    for (hash_text, value) in &spec.set_float_fields {
        let hash = u32::from_str_radix(hash_text.trim_start_matches("0x"), 16).unwrap();
        let ok = unit.get(hash).is_some_and(|property| {
            matches!(
                &property.kind,
                Kind::Scalar(Value::Float(v)) if (*v - *value as f32).abs() < 1e-4
            )
        });
        if !ok {
            return Err(format!(
                "invariant SET_FIELD violated: {hash:08X} is not {value}"
            ));
        }
    }
    // The donor tree bundle must survive: losing it loses the frozen look.
    if !bundle_enabled(DONOR_TREE_BUNDLE) {
        return Err(
            "invariant DONOR_TREE_BUNDLE violated: E7690450 disappeared from the enable union"
                .into(),
        );
    }
    // Walking department: the walkway must be Pedestrians-typed or students
    // cannot arrive (the park donor ships a Cars-typed, scripts-shadowed path).
    if spec.ensure_walk_path {
        let lot_id = ResourceId {
            type_id: PROPERTY_TYPE,
            group: lot_group(),
            instance: unit_id.instance,
        };
        let walk_ok = ctx.resources.get(&lot_id).is_some_and(|bytes| {
            PropertyFile::parse(bytes).is_ok_and(|lot| {
                matches!(
                    lot.get(HASH_PATH_ENTRIES).map(|p| &p.kind),
                    Some(Kind::Array(values)) if values.iter().any(|v| matches!(v,
                        Value::Key(k) if k.instance == PEDESTRIAN_PATH_ENTRY
                    ))
                )
            })
        });
        if !walk_ok {
            return Err(
                "invariant WALK_PATH violated: lot path entries are not Pedestrians-typed".into(),
            );
        }
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
