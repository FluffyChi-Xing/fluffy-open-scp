//! Native palette subcategories: parent -> category group -> category instances.
use super::*;

const CHILD_GROUP: u32 = 0x0C2585B1;
const LAYOUT: u32 = 0x0C22F4DE;
const NAME: u32 = 0x0C2BF463;
const ORDER: u32 = 0xCDCEF1AF;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuGroup {
    pub instance: u32,
    pub title_string_id: u32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuGroupsRequest {
    pub parent_instance: u32,
    pub parent_group: u32,
    pub new_group: u32,
    pub locale_table: u32,
    pub default_title_string_id: u32,
    pub children: Vec<MenuGroup>,
}

fn fail(message: impl Into<String>) -> CommandError {
    CommandError::new("game_menu_groups_invalid", message.into())
}

fn read(index: &AssetIndex, group: u32, instance: u32) -> Result<PropertyFile, CommandError> {
    let candidates = index.entries.get(&instance).ok_or_else(|| fail("category not found"))?;
    let (path, entry) = pick_property_candidate(candidates, Some(group))
        .ok_or_else(|| fail("category group not found"))?;
    let package = dbpf::Package::open(path).map_err(|e| fail(e.to_string()))?;
    PropertyFile::parse(&package.read(entry).map_err(|e| fail(e.to_string()))?)
        .map_err(|e| fail(e.to_string()))
}

fn encode(file: &PropertyFile, group: u32, instance: u32) -> Result<PropertyPatchResult, CommandError> {
    Ok(PropertyPatchResult {
        type_id: TYPE_PROPERTY, group, instance,
        data_base64: base64::engine::general_purpose::STANDARD
            .encode(file.encode_canonical().map_err(|e| fail(e.to_string()))?),
    })
}

fn category(group: &MenuGroup, table: u32, order: i32) -> PropertyFile {
    let mut file = PropertyFile { values: vec![], claimed_count: 0 };
    set_array_values(&mut file, NAME, PropType::Text,
        vec![Value::Text(sc_properties::Text { table_id: table, instance_id: group.title_string_id })]);
    set_scalar_value(&mut file, ORDER, PropType::Int32, Value::Int32(order));
    file
}

fn compile(index: &AssetIndex, request: &MenuGroupsRequest) -> Result<Vec<PropertyPatchResult>, CommandError> {
    if !matches!(request.parent_group, 0x09A4BA03 | 0x0C298BDD) || request.children.is_empty()
        || request.parent_instance == 0x3193C0BE {
        return Err(fail("expected a root palette and at least one subcategory"));
    }
    let mut parent = read(index, request.parent_group, request.parent_instance)?;
    let existing = parent.get(CHILD_GROUP).and_then(|p| match &p.kind {
        Kind::Scalar(Value::UInt32(value)) => Some(*value), _ => None,
    });
    let group = existing.unwrap_or(request.new_group);
    let mut output = Vec::new();
    if existing.is_none() || existing == Some(request.new_group) {
        // Retain direct members under a default tab with the SAME container ID.
        if existing.is_none() && index.entries.values().flatten().any(|(_, e)| e.id.type_id == TYPE_PROPERTY && e.id.group == group) {
            return Err(fail("new category group collides with installed resources"));
        }
        let default = category(&MenuGroup { instance: request.parent_instance,
            title_string_id: request.default_title_string_id }, request.locale_table, 0);
        output.push(encode(&default, group, request.parent_instance)?);
        // The retail culture palette contains the native subcategory button host.
        let template = read(index, 0x0C298BDD, 0x94E412ED)?;
        let layout = template.get(LAYOUT).and_then(|p| p.keys().next()).copied()
            .ok_or_else(|| fail("subcategory palette layout missing"))?;
        set_scalar_key(&mut parent, LAYOUT, Value::Key(layout));
        set_scalar_value(&mut parent, CHILD_GROUP, PropType::UInt32, Value::UInt32(group));
        output.push(encode(&parent, request.parent_group, request.parent_instance)?);
    }
    let mut seen = std::collections::HashSet::new();
    for (position, child) in request.children.iter().enumerate() {
        if child.instance == request.parent_instance || !seen.insert(child.instance)
            || index.entries.get(&child.instance).is_some_and(|entries|
                entries.iter().any(|(_, entry)| entry.id.type_id != TYPE_PROPERTY || entry.id.group != group)) {
            return Err(fail("subcategory identity collides with installed resources"));
        }
        output.push(encode(&category(child, request.locale_table, 10000 + position as i32), group, child.instance)?);
    }
    Ok(output)
}

#[tauri::command]
pub async fn game_menu_groups_compile(
    state: State<'_, crate::activity::AppState>, request: MenuGroupsRequest,
) -> Result<Vec<PropertyPatchResult>, CommandError> {
    let path = state.store.app_settings().map_err(CommandError::from)?
        .and_then(|s| s.game_data_path).ok_or_else(|| fail("game directory not configured"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let guard = cached_index(Path::new(&path));
        compile(guard.as_ref().ok_or_else(|| fail("game index unavailable"))?, &request)
    }).await.map_err(|e| fail(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires OPENSCP_GAME_PACKAGE pointing to a retail SimCity_Game.package"]
    fn retail_category_resources_compile_without_modifying_installation() {
        let path = PathBuf::from(std::env::var("OPENSCP_GAME_PACKAGE").unwrap());
        let package = dbpf::Package::open(&path).unwrap();
        let mut index = AssetIndex { entries: HashMap::new() };
        for entry in package.entries() {
            index.entries.entry(entry.id.instance).or_default().push((path.clone(), entry.clone()));
        }
        for (parent_group, parent_instance, expected_count) in [
            (0x09A4BA03, 0xAC12C904, 3), (0x0C298BDD, 0x94E412ED, 1),
        ] {
            let request = MenuGroupsRequest { parent_instance, parent_group,
                new_group: 0xF1897C01, locale_table: 77, default_title_string_id: 88,
                children: vec![MenuGroup { instance: 0xF2897C01, title_string_id: 99 }] };
            let output = compile(&index, &request).unwrap();
            assert_eq!(output.len(), expected_count);
            for result in &output {
                let bytes = base64::engine::general_purpose::STANDARD.decode(&result.data_base64).unwrap();
                let decoded = PropertyFile::parse(&bytes).unwrap();
                assert_eq!(decoded.encode_canonical().unwrap(), bytes);
            }
            if let Some(patched) = output.iter().find(|entry| entry.group == parent_group) {
                let original = read(&index, parent_group, parent_instance).unwrap();
                let decoded = PropertyFile::parse(&base64::engine::general_purpose::STANDARD.decode(&patched.data_base64).unwrap()).unwrap();
                for property in original.values.iter().filter(|p| p.hash != LAYOUT && p.hash != CHILD_GROUP) {
                    assert_eq!(decoded.get(property.hash), Some(property));
                }
                let layout = decoded.get(LAYOUT).unwrap().keys().next().unwrap();
                assert!(package.entries().iter().any(|entry| entry.id.type_id == layout.type_id
                    && entry.id.group == layout.group && entry.id.instance == layout.instance));
            }
        }
    }
    #[test]
    fn converts_direct_root_and_extends_existing_group_without_replacing_it() {
        let mut root = PropertyFile { values: vec![], claimed_count: 0 };
        set_scalar_value(&mut root, 123, PropType::Int32, Value::Int32(456));
        let mut culture = PropertyFile { values: vec![], claimed_count: 0 };
        set_scalar_value(&mut culture, CHILD_GROUP, PropType::UInt32, Value::UInt32(0x0F978DAC));
        set_scalar_key(&mut culture, LAYOUT, Value::Key(sc_properties::Key {
            type_id: 0x67771F5C, group: 0x0B074E5A, instance: 0xE99FFF56,
        }));
        let entries = vec![
            dbpf::OverlayEntry { id: dbpf::ResourceId { type_id: TYPE_PROPERTY, group: 0x09A4BA03, instance: 0xAC12C904 }, data: root.encode_canonical().unwrap() },
            dbpf::OverlayEntry { id: dbpf::ResourceId { type_id: TYPE_PROPERTY, group: 0x0C298BDD, instance: 0x94E412ED }, data: culture.encode_canonical().unwrap() },
        ];
        let path = std::env::temp_dir().join(format!("openscp-menu-groups-{}.package", std::process::id()));
        std::fs::write(&path, dbpf::write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let package = dbpf::Package::open(&path).unwrap();
        let mut index = AssetIndex { entries: HashMap::new() };
        for entry in package.entries() {
            index.entries.entry(entry.id.instance).or_default().push((path.clone(), entry.clone()));
        }
        let mut request = MenuGroupsRequest { parent_instance: 0xAC12C904, parent_group: 0x09A4BA03,
            new_group: 0xF1000001, locale_table: 77, default_title_string_id: 88,
            children: vec![MenuGroup { instance: 0xF2000001, title_string_id: 99 }] };
        let output = compile(&index, &request).unwrap();
        assert_eq!(output.len(), 3);
        let parent = output.iter().find(|entry| entry.group == 0x09A4BA03).unwrap();
        let file = PropertyFile::parse(&base64::engine::general_purpose::STANDARD.decode(&parent.data_base64).unwrap()).unwrap();
        assert!(matches!(&file.get(123).unwrap().kind, Kind::Scalar(Value::Int32(456))));
        assert!(matches!(&file.get(CHILD_GROUP).unwrap().kind, Kind::Scalar(Value::UInt32(0xF1000001))));
        assert!(output.iter().any(|entry| entry.group == 0xF1000001 && entry.instance == 0xAC12C904));
        request.parent_instance = 0x94E412ED;
        request.parent_group = 0x0C298BDD;
        let output = compile(&index, &request).unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].group, 0x0F978DAC);
        request.children[0].instance = 0xAC12C904;
        assert!(compile(&index, &request).is_err());
        drop(package);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn category_name_is_native_text_array_and_order_is_int() {
        let file = category(&MenuGroup { instance: 42, title_string_id: 99 }, 77, 10);
        let bytes = file.encode_canonical().unwrap();
        let decoded = PropertyFile::parse(&bytes).unwrap();
        let name = decoded.get(NAME).unwrap();
        assert_eq!(name.encoding.array_item_size, Some(sc_properties::TEXT_ARRAY_ITEM_SIZE));
        assert!(matches!(&name.kind, Kind::Array(values) if matches!(&values[0], Value::Text(t) if t.table_id == 77 && t.instance_id == 99)));
        assert!(matches!(&decoded.get(ORDER).unwrap().kind, Kind::Scalar(Value::Int32(10))));
    }
}
