//! Building count semantics. Read inherited simulation properties, then patch full
//! typed resources; capacity arrays are joined by resource ID, never by a UI index.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

type Id = (u32, u32); // group, instance; all entries are property resources
const MODULE_BIN: u32 = 0x0DF1467C;
const SHARED_BIN: u32 = 0x0E51C90F;
const INDIVIDUAL_BIN: u32 = 0x0EC5D62E;
const UNLOCK_BIN: u32 = 0x0BF5D2E3;
const BIN_IDS: u32 = 0x0A8DF590;
const CAPACITIES: u32 = 0x0A8DF591;
// Retail 0x0066ABE1: property = 0x0CC8BE51 + index in kModuleResourceIDs.
const MODULE_BINS: [u32; 16] = [
    0x7D11699A, 0x7D116999, 0x7D116998, 0x7D11699F, 0x7D11699E, 0x7D11699D, 0x7D11699C, 0x7D116993,
    0x7D116992, 0x7C693D5E, 0x7C693D5F, 0x7C693D5C, 0x7C693D5D, 0x7C693D5A, 0x7C693D5B, 0x7C693D58,
];

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("game_count_limits_invalid", message.into())
}

#[derive(Default)]
struct Catalog {
    files: BTreeMap<Id, PropertyFile>,
    menus: BTreeMap<u32, Id>,
    units: BTreeMap<u32, Vec<Id>>,
    capacities: BTreeMap<u32, Vec<(Id, usize)>>,
}

fn key(file: &PropertyFile, hash: u32) -> Option<sc_properties::Key> {
    file.get(hash)?.keys().next().copied()
}

impl Catalog {
    fn load(game_data: &Path) -> Result<Self, CommandError> {
        let mut latest = BTreeMap::<Id, (PathBuf, dbpf::IndexEntry)>::new();
        for path in package_paths(game_data) {
            let package = dbpf::Package::open(&path).map_err(|e| invalid(e.to_string()))?;
            let simulation = path.components().any(|c| c.as_os_str() == "EcoGame");
            for entry in package
                .entries()
                .iter()
                .filter(|e| e.id.type_id == TYPE_PROPERTY)
            {
                let family = entry.id.group & 0xffff;
                if !matches!(family, 0xC900 | 0x8A01 | 0xEB00)
                    && !(simulation && matches!(family, 0xC000 | 0xC700))
                {
                    continue;
                }
                let id = (entry.id.group, entry.id.instance);
                if latest
                    .get(&id)
                    .is_none_or(|(old, _)| candidate_score(&path) > candidate_score(old))
                {
                    latest.insert(id, (path.clone(), entry.clone()));
                }
            }
        }
        let mut result = Self::default();
        let mut packages = HashMap::new();
        for (id, (path, entry)) in latest {
            if !packages.contains_key(&path) {
                packages.insert(
                    path.clone(),
                    dbpf::Package::open(&path).map_err(|e| invalid(e.to_string()))?,
                );
            }
            let pkg = &packages[&path];
            let bytes = pkg.read(&entry).map_err(|e| invalid(e.to_string()))?;
            let file = PropertyFile::parse(&bytes).map_err(|e| invalid(e.to_string()))?;
            match id.0 & 0xffff {
                0xC900 | 0x8A01 => {
                    result.menus.insert(id.1, id);
                }
                0xC000 => {
                    result.units.entry(id.1).or_default().push(id);
                }
                _ => {}
            }
            result.files.insert(id, file);
        }
        result.index_capacities()?;
        Ok(result)
    }

    fn index_capacities(&mut self) -> Result<(), CommandError> {
        for (id, file) in &self.files {
            let Some(ids) = file.get(BIN_IDS).and_then(|p| p.array()) else {
                continue;
            };
            let Some(caps) = file.get(CAPACITIES).and_then(|p| p.array()) else {
                continue;
            };
            if ids.len() != caps.len() {
                return Err(invalid("capacity array length mismatch"));
            }
            for (index, value) in ids.iter().enumerate() {
                if let (Value::Key(k), Value::Int32(_)) = (value, &caps[index]) {
                    self.capacities
                        .entry(k.instance)
                        .or_default()
                        .push((*id, index));
                }
            }
        }
        Ok(())
    }

    fn unit(&self, instance: u32) -> Option<Id> {
        let ids = self.units.get(&instance)?;
        (ids.len() == 1).then(|| ids[0])
    }

    fn inherited(&self, mut id: Id, hash: u32) -> Option<&sc_properties::Property> {
        let mut visited = BTreeSet::new();
        while visited.len() < 32 && visited.insert(id) {
            let file = self.files.get(&id)?;
            if let Some(value) = file.get(hash) {
                return Some(value);
            }
            let parent = key(file, 0x00B2CCCB)?;
            id = if parent.group != 0 {
                (parent.group, parent.instance)
            } else {
                self.unit(parent.instance)?
            };
        }
        None
    }

    fn reference(&self, id: Id, hash: u32) -> Option<u32> {
        self.inherited(id, hash)?.keys().next().map(|k| k.instance)
    }

    fn capacity(&self, resource: u32) -> Option<(Id, usize, i32)> {
        let locations = self.capacities.get(&resource)?;
        if locations.len() != 1 {
            return None;
        }
        let (id, index) = locations[0];
        match self.files.get(&id)?.get(CAPACITIES)?.array()?.get(index)? {
            Value::Int32(value) if *value > 0 => Some((id, index, *value)),
            _ => None,
        }
    }

    fn resolve(&self, source: u32) -> Option<Resolved> {
        let menu = *self.menus.get(&source)?;
        if self.inherited(menu, 0x0F7FA2A8).is_some() {
            return None;
        }
        let unit = self.unit(self.reference(menu, 0x0975695F)?)?;
        let module = self.reference(unit, MODULE_BIN)?;
        let hash = 0x0CC8BE51 + MODULE_BINS.iter().position(|v| *v == module)? as u32;
        let individual = match self.inherited(unit, hash)?.scalar()? {
            Value::Float(v) if v.is_finite() && *v >= 1.0 && v.fract() == 0.0 => *v as i32,
            _ => return None,
        };
        let shared_bin = self.reference(unit, SHARED_BIN);
        let shared =
            shared_bin.and_then(|bin| self.capacity(bin).map(|(_, _, value)| (bin, value)));
        // Missing/ambiguous shared capacity is unsupported, not an unlimited building.
        if shared_bin.is_some() && shared.is_none() {
            return None;
        }
        let individual_bin = self.reference(unit, INDIVIDUAL_BIN).or_else(|| {
            shared
                .is_none()
                .then(|| self.reference(unit, UNLOCK_BIN))
                .flatten()
        });
        if individual_bin.is_some_and(|bin| self.capacity(bin).is_none()) {
            return None;
        }
        Some(Resolved {
            unit,
            hash,
            individual,
            individual_bin,
            shared,
        })
    }
}

struct Resolved {
    unit: Id,
    hash: u32,
    individual: i32,
    individual_bin: Option<u32>,
    shared: Option<(u32, i32)>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CountLimits {
    pub individual: Option<i32>,
    pub shared: Option<i32>,
    pub shared_key: Option<String>,
}

impl Resolved {
    fn info(&self) -> CountLimits {
        CountLimits {
            individual: Some(self.individual),
            shared: self.shared.map(|(_, value)| value),
            shared_key: self.shared.map(|(bin, _)| format!("{bin:08X}")),
        }
    }
}

static CATALOG: Mutex<Option<(PathBuf, Arc<Catalog>)>> = Mutex::new(None);
fn catalog(path: &Path) -> Result<Arc<Catalog>, CommandError> {
    let mut cache = CATALOG
        .lock()
        .map_err(|_| invalid("count catalog unavailable"))?;
    if let Some((root, value)) = cache.as_ref() {
        if root == path {
            return Ok(value.clone());
        }
    }
    let value = Arc::new(Catalog::load(path)?);
    *cache = Some((path.to_path_buf(), value.clone()));
    Ok(value)
}

fn game_path(state: &State<'_, crate::activity::AppState>) -> Result<PathBuf, CommandError> {
    state
        .store
        .app_settings()
        .map_err(CommandError::from)?
        .and_then(|s| s.game_data_path)
        .map(PathBuf::from)
        .ok_or_else(|| {
            CommandError::new(
                "game_directory_not_configured",
                "Game data directory is not configured",
            )
        })
}

#[tauri::command]
pub async fn game_count_limits_read(
    state: State<'_, crate::activity::AppState>,
    source_instance: u32,
) -> Result<CountLimits, CommandError> {
    let path = game_path(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(catalog(&path)?
            .resolve(source_instance)
            .map(|r| r.info())
            .unwrap_or_default())
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountChange {
    pub source_instance: u32,
    pub individual: Option<i32>,
    pub shared: Option<i32>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseResource {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
    pub data_base64: String,
}

fn compile(
    catalog: &Catalog,
    changes: &[CountChange],
    base: &[BaseResource],
) -> Result<Vec<PropertyPatchResult>, CommandError> {
    // Reject conflicting requests instead of letting the final writer silently win.
    let mut scalars = BTreeMap::<(Id, u32), i32>::new();
    let mut bins = BTreeMap::<u32, i32>::new();
    fn insert<K: Ord>(map: &mut BTreeMap<K, i32>, key: K, value: i32) -> Result<(), CommandError> {
        // Float-backed integer limits must remain exactly representable.
        if !(1..=16_777_216).contains(&value) {
            return Err(invalid("limit must be an integer from 1 to 16777216"));
        }
        if map.insert(key, value).is_some_and(|old| old != value) {
            return Err(invalid("conflicting count limits"));
        }
        Ok(())
    }
    for change in changes {
        let r = catalog
            .resolve(change.source_instance)
            .ok_or_else(|| invalid("building has no supported count limit"))?;
        if let Some(value) = change.individual {
            insert(&mut scalars, (r.unit, r.hash), value)?;
            if let Some(bin) = r.individual_bin {
                insert(&mut bins, bin, value)?;
            }
        }
        if let Some(value) = change.shared {
            let (bin, _) = r
                .shared
                .ok_or_else(|| invalid("building has no shared count limit"))?;
            insert(&mut bins, bin, value)?;
            // Landmark unlock budget and built counter are paired global resources.
            if bin == 0x4B427C97 {
                insert(&mut bins, 0x3B6116BF, value)?;
            }
        }
    }
    let mut files = BTreeMap::<Id, PropertyFile>::new();
    let mut load = |id: Id| -> Result<(), CommandError> {
        if files.contains_key(&id) {
            return Ok(());
        }
        let matches: Vec<_> = base
            .iter()
            .filter(|v| v.type_id == TYPE_PROPERTY && (v.group, v.instance) == id)
            .collect();
        if matches.len() > 1 {
            return Err(invalid("duplicate input property TGI"));
        }
        let file = if let Some(input) = matches.first() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&input.data_base64)
                .map_err(|e| invalid(e.to_string()))?;
            PropertyFile::parse(&bytes).map_err(|e| invalid(e.to_string()))?
        } else {
            catalog
                .files
                .get(&id)
                .ok_or_else(|| invalid("missing source property"))?
                .clone()
        };
        files.insert(id, file);
        Ok(())
    };
    for (id, _) in scalars.keys() {
        load(*id)?;
    }
    for bin in bins.keys() {
        load(
            catalog
                .capacity(*bin)
                .ok_or_else(|| invalid("missing capacity definition"))?
                .0,
        )?;
    }
    for ((id, hash), value) in scalars {
        // Override on the leaf unit, never on a template shared by other buildings.
        set_scalar_value(
            files.get_mut(&id).unwrap(),
            hash,
            PropType::Float,
            Value::Float(value as f32),
        );
    }
    for (bin, value) in bins {
        let (id, index, _) = catalog.capacity(bin).unwrap();
        let file = files.get_mut(&id).unwrap();
        let ids = file
            .get(BIN_IDS)
            .and_then(|p| p.array())
            .ok_or_else(|| invalid("missing resource ID array"))?;
        if !matches!(ids.get(index), Some(Value::Key(k)) if k.instance == bin) {
            return Err(invalid("resource ID array changed"));
        }
        let count = ids.len();
        let prop = file
            .values
            .iter_mut()
            .find(|p| p.hash == CAPACITIES)
            .ok_or_else(|| invalid("missing capacity array"))?;
        let Kind::Array(values) = &mut prop.kind else {
            return Err(invalid("capacity must remain an array"));
        };
        if prop.prop_type != PropType::Int32
            || values.len() != count
            || !matches!(values.get(index), Some(Value::Int32(_)))
        {
            return Err(invalid("invalid capacity array type or length"));
        }
        values[index] = Value::Int32(value);
    }
    files
        .into_iter()
        .map(|((group, instance), mut file)| {
            normalize_array_item_sizes(&mut file);
            let bytes = file
                .encode_canonical()
                .map_err(|e| invalid(e.to_string()))?;
            PropertyFile::parse(&bytes).map_err(|e| invalid(e.to_string()))?;
            Ok(PropertyPatchResult {
                type_id: TYPE_PROPERTY,
                group,
                instance,
                data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn game_count_limits_patch(
    state: State<'_, crate::activity::AppState>,
    changes: Vec<CountChange>,
    base_resources: Vec<BaseResource>,
) -> Result<Vec<PropertyPatchResult>, CommandError> {
    let path = game_path(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        compile(catalog(&path)?.as_ref(), &changes, &base_resources)
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Catalog {
        let mut c = Catalog::default();
        let mut unit = PropertyFile {
            values: vec![],
            claimed_count: 0,
        };
        set_scalar_value(
            &mut unit,
            MODULE_BIN,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: MODULE_BINS[0],
            }),
        );
        set_scalar_value(
            &mut unit,
            SHARED_BIN,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: 0x4B427C97,
            }),
        );
        set_scalar_value(&mut unit, 0x0CC8BE51, PropType::Float, Value::Float(1.0));
        c.files.insert((1, 10), unit);
        c.units.insert(10, vec![(1, 10)]);
        let mut menu = PropertyFile {
            values: vec![],
            claimed_count: 0,
        };
        set_scalar_value(
            &mut menu,
            0x0975695F,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: 10,
            }),
        );
        c.files.insert((2, 20), menu);
        c.menus.insert(20, (2, 20));
        let mut capacities = PropertyFile {
            values: vec![],
            claimed_count: 0,
        };
        set_array_values(
            &mut capacities,
            BIN_IDS,
            PropType::Key,
            [0x4B427C97, 0x3B6116BF, 99]
                .map(|instance| {
                    Value::Key(sc_properties::Key {
                        type_id: 0,
                        group: 0,
                        instance,
                    })
                })
                .to_vec(),
        );
        set_array_values(
            &mut capacities,
            CAPACITIES,
            PropType::Int32,
            vec![Value::Int32(3), Value::Int32(3), Value::Int32(42)],
        );
        c.files.insert((3, 30), capacities);
        c.index_capacities().unwrap();
        c
    }

    fn decode(result: &PropertyPatchResult) -> PropertyFile {
        PropertyFile::parse(
            &base64::engine::general_purpose::STANDARD
                .decode(&result.data_base64)
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn count_limits_keep_array_shape_merge_shared_edits_and_preserve_other_properties() {
        let c = fixture();
        let mut base = c.files[&(1, 10)].clone();
        set_scalar_value(&mut base, 0x0A8A41DA, PropType::Float, Value::Float(123.0));
        let base = BaseResource {
            type_id: TYPE_PROPERTY,
            group: 1,
            instance: 10,
            data_base64: base64::engine::general_purpose::STANDARD
                .encode(base.encode_canonical().unwrap()),
        };
        let result = compile(
            &c,
            &[CountChange {
                source_instance: 20,
                individual: Some(2),
                shared: Some(5),
            }],
            &[base],
        )
        .unwrap();
        assert_eq!(result.len(), 2);
        let unit = decode(result.iter().find(|r| r.instance == 10).unwrap());
        assert!(matches!(
            unit.get(0x0CC8BE51).unwrap().scalar(),
            Some(Value::Float(2.0))
        ));
        assert!(matches!(
            unit.get(0x0A8A41DA).unwrap().scalar(),
            Some(Value::Float(123.0))
        ));
        let bins = decode(result.iter().find(|r| r.instance == 30).unwrap());
        let p = bins.get(CAPACITIES).unwrap();
        assert_eq!(p.encoding.array_item_size, Some(4));
        assert!(matches!(
            p.array().unwrap(),
            [Value::Int32(5), Value::Int32(5), Value::Int32(42)]
        ));
    }

    #[test]
    fn count_limits_reject_conflicts_invalid_numbers_and_unsupported_tools() {
        let c = fixture();
        for value in [0, -1, 16_777_217] {
            assert!(
                compile(
                    &c,
                    &[CountChange {
                        source_instance: 20,
                        individual: Some(value),
                        shared: None
                    }],
                    &[]
                )
                .is_err()
            );
        }
        assert!(
            compile(
                &c,
                &[CountChange {
                    source_instance: 99,
                    individual: Some(2),
                    shared: None
                }],
                &[]
            )
            .is_err()
        );
        assert!(
            compile(
                &c,
                &[
                    CountChange {
                        source_instance: 20,
                        individual: None,
                        shared: Some(4)
                    },
                    CountChange {
                        source_instance: 20,
                        individual: None,
                        shared: Some(5)
                    },
                ],
                &[]
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires SIMCITY_DATA; optionally writes browser metadata to COUNT_LIMITS_MANIFEST"]
    fn count_limits_installed_game_roundtrip() {
        let path = PathBuf::from(std::env::var("SIMCITY_DATA").expect("SIMCITY_DATA"));
        let c = Catalog::load(&path).unwrap();
        let vu = c.resolve(0xBC45E41B).expect("V doctor");
        assert_eq!(vu.individual, 1);
        assert_eq!(vu.shared, None);
        let empire = c.resolve(0xC0E5BE77).expect("Empire State");
        assert_eq!(empire.individual, 1);
        assert_eq!(empire.shared, Some((0x4B427C97, 3)));
        let results = compile(
            &c,
            &[
                CountChange {
                    source_instance: 0xBC45E41B,
                    individual: Some(2),
                    shared: None,
                },
                CountChange {
                    source_instance: 0xC0E5BE77,
                    individual: Some(2),
                    shared: Some(5),
                },
            ],
            &[],
        )
        .unwrap();
        for resource in &results {
            decode(resource);
        }
        let bins = decode(results.iter().find(|r| r.instance == 0x318E8443).unwrap());
        assert!(matches!(
            bins.get(CAPACITIES).unwrap().array().unwrap()[0..2],
            [Value::Int32(5), Value::Int32(5)]
        ));
        let single = c.capacity(0x4A73D81E).unwrap().1;
        assert!(matches!(
            bins.get(CAPACITIES).unwrap().array().unwrap()[single],
            Value::Int32(2)
        ));
        if let Ok(output) = std::env::var("COUNT_LIMITS_MANIFEST") {
            let infos: BTreeMap<_, _> = c
                .menus
                .keys()
                .filter_map(|id| c.resolve(*id).map(|r| (format!("{id:08X}"), r.info())))
                .collect();
            std::fs::write(output, serde_json::to_string_pretty(&infos).unwrap()).unwrap();
        }
    }
}
