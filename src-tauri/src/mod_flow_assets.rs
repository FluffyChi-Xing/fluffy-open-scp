//! Project-local asset inputs. Native package passthrough preserves every TGI;
//! edited/foreign geometry stays a draft until the RW4 compiler is available.
use super::*;
use base64::Engine;
use std::process::Command;

pub(super) fn import(root: &Path, payload: &Value) -> Result<Value, String> {
    let source = PathBuf::from(payload["path"].as_str().ok_or("Missing source path")?);
    if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("Asset exceeds 128 MiB".into());
    }
    let extension = source.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if extension == "blend" {
        return import_blend(root, &source);
    }
    if !["package", "glb", "lotm", "png", "jpg", "jpeg", "json", "rw4", "dds"].contains(&extension.as_str()) {
        return Err("Unsupported asset format".into());
    }
    let bytes = fs::read(&source).map_err(|e| e.to_string())?;
    if extension == "package" { dbpf::Package::open(&source).map_err(|e| e.to_string())?; }
    let folder = if ["png", "jpg", "jpeg", "dds", "json"].contains(&extension.as_str()) { "textures" } else { "models" };
    let relative = format!("assets/{folder}/{}.{}", digest(&bytes), extension);
    let target = safe_path(root, &relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    if !target.exists() { crate::atomic_fs::write_atomic(&target, &bytes, false).map_err(|e| e.to_string())?; }
    describe(root, &relative)
}

/// Use Blender's headless Python API to turn a .blend into a project GLB and
/// recover the six-slot image mapping from material node names. Blender is an
/// optional runtime dependency; the error is explicit when it is unavailable.
fn import_blend(root: &Path, source: &Path) -> Result<Value, String> {
    let temp = root.join(".openscp-blend-import");
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let out_glb = temp.join("model.glb");
    let out_map = temp.join("slots.json");
let script = r#"import bpy, json, os, sys, struct
src, out_glb, out_map, out_dir = sys.argv[-4:]
bpy.ops.wm.open_mainfile(filepath=src)
# Community blends (e.g. NY Facade) keep live six-slot engine node groups that
# reference the slot atlases INSIDE the material graph. Legacy leftovers of the
# same slot images elsewhere in the file must not disable evaluation, so the
# decision is per material graph, not per file. Materials without six-slot
# semantics that the glTF exporter cannot translate are baked to a final
# color image instead; the source node graph stays the authority either way.
def tree_images(tree, seen=None):
    if not tree:
        return []
    seen = seen or set()
    if tree.name in seen:
        return []
    seen.add(tree.name)
    result = []
    for node in tree.nodes:
        if node.type == 'TEX_IMAGE' and node.image:
            result.append(node.image)
        elif node.type == 'GROUP' and node.node_tree:
            result.extend(tree_images(node.node_tree, seen))
    return result

def image_haystack(image):
    name = (image.name or '').lower()
    path = os.path.basename(bpy.path.abspath(image.filepath) if image.filepath else '').lower()
    return name + ' ' + path

def material_slot_keys(mat):
    keys = set()
    for image in tree_images(mat.node_tree):
        haystack = image_haystack(image)
        for key in ('slot1', 'slot2', 'slot3', 'slot4', 'slot5'):
            if key in haystack:
                keys.add(key)
    return keys

def principled_of(tree):
    return next((n for n in tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)

def is_six_slot_material(mat):
    return {'slot1', 'slot4'} <= material_slot_keys(mat)

def is_directly_exportable(mat):
    # The glTF exporter translates a Principled BSDF whose base color is an
    # unlinked value or a direct image texture; anything else needs a bake.
    if not mat or not mat.use_nodes or not mat.node_tree:
        return True
    bsdf = principled_of(mat.node_tree)
    output = next((n for n in mat.node_tree.nodes if n.type == 'OUTPUT_MATERIAL'), None)
    if not bsdf or not output or not output.inputs['Surface'].is_linked:
        return False
    if output.inputs['Surface'].links[0].from_node != bsdf:
        return False
    base = bsdf.inputs['Base Color']
    if not base.is_linked:
        return True
    return base.links[0].from_node.type == 'TEX_IMAGE'

mesh_objects = [o for o in bpy.data.objects if o.type == 'MESH']
six_slot_materials = {
    mat.name for mat in bpy.data.materials if is_six_slot_material(mat)
}
bake_materials = []
seen_names = set()
for obj in mesh_objects:
    for mat in obj.data.materials:
        if not mat or not mat.use_nodes or mat.name in seen_names:
            continue
        seen_names.add(mat.name)
        if mat.name in six_slot_materials or is_directly_exportable(mat):
            continue
        bake_materials.append(mat)

# --- UV domain normalization (six-slot path) ---------------------------
# The refined shader reads the base facade domain from TEXCOORD_1 and the
# Top/facade domain from TEXCOORD_3. The glTF exporter orders TEXCOORD_n by
# the mesh's UV layer order, so move the two domain layers into place. The
# domain layers are identified the same way the engine groups consume them:
# UVMAP nodes feeding the group's base/top facade inputs (the official B4
# pipeline names them UV Base / UV Top; community facade plugins name them
# Base UV / Surface UV).
def find_domain_layer_names():
    base_name = None
    surface_name = None
    for obj in mesh_objects:
        for mat in obj.data.materials:
            if not mat or not mat.use_nodes or not mat.node_tree:
                continue
            for node in mat.node_tree.nodes:
                if node.type != 'GROUP':
                    continue
                for inp in node.inputs:
                    if not inp.is_linked:
                        continue
                    feed = inp.links[0].from_node
                    if feed.type != 'UVMAP':
                        continue
                    if inp.name in ('Base UV', 'UV Base'):
                        base_name = feed.uv_map
                    elif inp.name in ('Surface UV', 'UV Top'):
                        surface_name = feed.uv_map
    return base_name or 'NY_UV', surface_name or 'NY_SurfaceUV'

if six_slot_materials:
    try:
        import numpy as np
        base_name, surface_name = find_domain_layer_names()
        for obj in mesh_objects:
            me = obj.data
            names = [ul.name for ul in me.uv_layers]
            if base_name not in names or surface_name not in names:
                continue
            order = [n for n in names if n not in (base_name, surface_name)]
            order.insert(min(1, len(order)), base_name)
            order.insert(min(3, len(order)), surface_name)
            if order == names:
                continue
            loop_count = len(me.loops)
            captured = {}
            for ul in me.uv_layers:
                buf = np.empty(loop_count * 2, dtype=np.float32)
                ul.data.foreach_get('uv', buf)
                captured[ul.name] = buf
            for ul in list(me.uv_layers):
                me.uv_layers.remove(ul)
            for name in order:
                layer = me.uv_layers.new(name=name)
                if name in captured:
                    layer.data.foreach_set('uv', captured[name])
            me.update()
    except Exception as error:
        print('OpenSCP UV normalization skipped:', repr(error))

# --- bake non-exportable materials (no six-slot semantics) --------------
if bake_materials:
    scene = bpy.context.scene
    try:
        scene.render.engine = 'CYCLES'
        scene.cycles.samples = 1
        scene.cycles.use_denoising = False
        scene.cycles.device = 'CPU'
        for mat in bake_materials:
            tree = mat.node_tree
            bsdf = principled_of(tree)
            if not bsdf:
                continue
            bake = tree.nodes.get('__OpenSCP_Bake')
            if not bake:
                bake = tree.nodes.new('ShaderNodeTexImage')
                bake.name = '__OpenSCP_Bake'
            if not bake.image:
                bake.image = bpy.data.images.new(
                    '__OpenSCP_Bake_' + ''.join(ch if ch.isalnum() else '_' for ch in mat.name)[:60],
                    width=2048, height=2048, alpha=True)
            tree.nodes.active = bake
            emission = bsdf.inputs.get('Emission Color') or bsdf.inputs.get('Emission')
            base = bsdf.inputs['Base Color']
            for link in list(emission.links):
                tree.links.remove(link)
            if base.is_linked:
                tree.links.new(base.links[0].from_socket, emission)
            else:
                emission.default_value = tuple(base.default_value)
        for obj in mesh_objects:
            mats = list(obj.data.materials)
            wanted = [(index, mat) for index, mat in enumerate(mats)
                      if mat in bake_materials and
                      '__OpenSCP_Bake' in (mat.node_tree.nodes if mat and mat.use_nodes else {})]
            if not wanted:
                continue
            bpy.ops.object.select_all(action='DESELECT')
            obj.select_set(True)
            bpy.context.view_layer.objects.active = obj
            for index, mat in wanted:
                for poly in obj.data.polygons:
                    poly.select = (poly.material_index == index)
                bpy.ops.object.bake(type='EMIT', margin=8, use_clear=True)
        for mat in bake_materials:
            bake = mat.node_tree.nodes.get('__OpenSCP_Bake')
            if not bake or not bake.image:
                continue
            image = bake.image
            tree = mat.node_tree
            tree.nodes.clear()
            output = tree.nodes.new('ShaderNodeOutputMaterial')
            bsdf = tree.nodes.new('ShaderNodeBsdfPrincipled')
            tex = tree.nodes.new('ShaderNodeTexImage')
            tex.image = image
            tree.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
            if 'Alpha' in tex.outputs and 'Alpha' in bsdf.inputs:
                tree.links.new(tex.outputs['Alpha'], bsdf.inputs['Alpha'])
            tree.links.new(bsdf.outputs['BSDF'], output.inputs['Surface'])
            bsdf.inputs['Roughness'].default_value = 0.8
            try:
                mat.surface_render_method = 'DITHERED'
            except Exception:
                pass
    except Exception as error:
        # A blend may use a renderer that cannot bake in headless mode.
        # Exporting the original graph is still useful, and the caller
        # receives a valid GLB.
        print('OpenSCP bake skipped:', repr(error))

# --- GLB export (standard Y-up glTF convention) -------------------------
bpy.ops.object.select_all(action='SELECT')
exporter = getattr(getattr(bpy.ops, 'export_scene', None), 'gltf', None)
fallback = getattr(getattr(bpy.ops, 'wm', None), 'gltf_export', None)
if exporter is None and fallback is None:
    raise RuntimeError('This Blender build does not provide a glTF exporter')
try:
    result = exporter(filepath=out_glb, export_format='GLB', use_selection=False, export_yup=True) if exporter else fallback(filepath=out_glb, export_format='GLB', use_selection=False, export_yup=True)
except RuntimeError:
    if fallback is None:
        raise
    result = fallback(filepath=out_glb, export_format='GLB', use_selection=False, export_yup=True)
if 'FINISHED' not in result:
    raise RuntimeError('The glTF exporter did not finish')

def inject_up_axis(path):
    # Declare the file's axis contract so the editor does not have to guess
    # from bounds (a low, deep building would misdetect). export_yup=True
    # guarantees a spec Y-up file regardless of the source objects' own
    # rotations, so the declaration is always "y" for blend conversions.
    try:
        with open(path, 'rb') as f:
            data = bytearray(f.read())
        if len(data) < 20 or struct.unpack_from('<I', data, 0)[0] != 0x46546c67:
            return
        json_length = struct.unpack_from('<I', data, 12)[0]
        doc = json.loads(bytes(data[20:20 + json_length]))
        openscp = doc.setdefault('asset', {}).setdefault('extras', {}).setdefault('openscp', {})
        if not isinstance(openscp, dict):
            openscp = {}
            doc['asset']['extras']['openscp'] = openscp
        openscp.setdefault('upAxis', 'y')
        openscp.setdefault('unit', 'meter')
        openscp['source'] = 'blend'
        payload = json.dumps(doc, separators=(',', ':')).encode('utf-8')
        payload += b' ' * ((4 - (len(payload) % 4)) % 4)
        bin_start = 20 + json_length
        total = 12 + 8 + len(payload) + len(data[bin_start:])
        struct.pack_into('<I', data, 8, total)
        struct.pack_into('<I', data, 12, len(payload))
        with open(path, 'wb') as f:
            f.write(bytes(data[:20] + payload + data[bin_start:]))
    except Exception as error:
        print('OpenSCP extras injection skipped:', repr(error))

inject_up_axis(out_glb)

# --- slot extraction (crash-proof: partial results still land) ----------
slots = {}
material_slots = {}
material_params = {}
os.makedirs(out_dir, exist_ok=True)
try:
    for key in ('slot1', 'slot2', 'slot3', 'slot4', 'slot5'):
        image = bpy.data.images.get(key + '.png')
        if image:
            path = os.path.join(out_dir, key + '.png')
            try:
                image.save_render(path)
                slots[key] = path
            except Exception:
                pass
    params = bpy.data.images.get('slot0_params.exr')
    if params:
        path = os.path.join(out_dir, 'params.json')
        try:
            with open(path, 'w', encoding='utf-8') as f:
                json.dump({'width': params.size[0], 'values': [float(value) for value in params.pixels]}, f)
            slots['params'] = path
        except Exception:
            pass

    def find_parameter_image(tree, seen=None):
        if not tree:
            return None
        seen = seen or set()
        if tree.name in seen:
            return None
        seen.add(tree.name)
        for node in tree.nodes:
            if node.type == 'TEX_IMAGE' and node.image and 'Parameters' in node.image.name:
                return node.image
            if node.type == 'GROUP':
                image = find_parameter_image(node.node_tree, seen)
                if image:
                    return image
        return None

    def all_images(tree, seen=None):
        if not tree:
            return []
        seen = seen or set()
        if tree.name in seen:
            return []
        seen.add(tree.name)
        result = []
        for node in tree.nodes:
            if node.type == 'TEX_IMAGE' and node.image:
                result.append(node.image)
            if node.type == 'GROUP':
                result.extend(all_images(node.node_tree, seen))
        return result

    for mat in bpy.data.materials:
        if not mat.use_nodes:
            continue
        parameter_image = find_parameter_image(mat.node_tree)
        if parameter_image:
            palette_row = 0.0
            for node in mat.node_tree.nodes:
                if node.type == 'GROUP':
                    socket = node.inputs.get('Palette Row')
                    if socket and not socket.is_linked:
                        try:
                            palette_row = float(socket.default_value)
                        except Exception:
                            pass
                        break
            try:
                material_params[mat.name] = {
                    'width': int(parameter_image.size[0]),
                    'values': [float(value) for value in parameter_image.pixels],
                    'paletteRow': palette_row,
                }
            except Exception:
                pass
        per_mat = {}
        for image in all_images(mat.node_tree):
            image_name = (image.name or '').lower()
            image_path = bpy.path.abspath(image.filepath) if image.filepath else ''
            haystack = image_name + ' ' + os.path.basename(image_path).lower()
            for key in ('slot1', 'slot2', 'slot3', 'slot4', 'slot5'):
                if key not in haystack or key in per_mat:
                    continue
                safe_name = ''.join(ch if ch.isalnum() else '_' for ch in mat.name)[:80]
                path = os.path.join(out_dir, safe_name + '_' + key + '.png')
                try:
                    image.save_render(path)
                    per_mat[key] = path
                except Exception:
                    pass
        if per_mat:
            material_slots[mat.name] = per_mat
except Exception as error:
    print('OpenSCP slot extraction partial:', repr(error))
finally:
    try:
        material_params_path = os.path.join(out_dir, 'material_params.json')
        with open(material_params_path, 'w', encoding='utf-8') as f:
            json.dump(material_params, f)
        slots['materialParams'] = material_params_path
    except Exception:
        pass
    slots['materialSlotsByMaterial'] = material_slots
    with open(out_map, 'w', encoding='utf-8') as f:
        json.dump(slots, f)
"#;
    let script_path = temp.join("import.py");
    fs::write(&script_path, script).map_err(|e| e.to_string())?;
    let blender = blender_executable();
    let args = [
        "--background".to_owned(),
        "--python".to_owned(),
        blender_path_arg(&script_path),
        "--".to_owned(),
        blender_path_arg(source),
        blender_path_arg(&out_glb),
        blender_path_arg(&out_map),
        blender_path_arg(&temp),
    ];
    let status = Command::new(&blender).args(&args).status().map_err(|_| format!("Blender executable not found: {}", blender.display()))?;
    // Some Blender builds return a non-zero code after writing a valid GLB
    // (for example when an optional image codec warns). The artifact is the
    // authoritative result; only fail when it was not created.
    if !status.success() && !out_glb.is_file() {
        let _ = fs::remove_dir_all(&temp);
        return Err("Blender could not export this file".into());
    }
    if !out_glb.is_file() {
        let _ = fs::remove_dir_all(&temp);
        return Err("Blender could not export this file".into());
    }
    let bytes = fs::read(&out_glb).map_err(|e| e.to_string())?;
    let relative = format!("assets/models/{}.glb", digest(&bytes));
    let target = safe_path(root, &relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    crate::atomic_fs::write_atomic(&target, &bytes, false).map_err(|e| e.to_string())?;
    let raw_slots: Value = fs::read_to_string(&out_map).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
    let mut slots = serde_json::Map::new();
    if let Some(object) = raw_slots.as_object() {
        for (key, value) in object {
            if key == "materialSlotsByMaterial" {
                let mut by_material = serde_json::Map::new();
                if let Some(materials) = value.as_object() {
                    for (material_name, material_value) in materials {
                        let mut copied = serde_json::Map::new();
                        if let Some(material_object) = material_value.as_object() {
                            for (slot, slot_path) in material_object {
                                let Some(path) = slot_path.as_str() else { continue };
                                let Ok(image_bytes) = fs::read(path) else { continue };
                                let ext = Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("png").to_ascii_lowercase();
                                let image_relative = format!("assets/textures/{}.{}", digest(&image_bytes), ext);
                                let image_target = safe_path(root, &image_relative)?;
                                fs::create_dir_all(image_target.parent().unwrap()).map_err(|e| e.to_string())?;
                                if !image_target.exists() { crate::atomic_fs::write_atomic(&image_target, &image_bytes, false).map_err(|e| e.to_string())?; }
                                copied.insert(slot.clone(), json!(image_relative));
                            }
                        }
                        if !copied.is_empty() { by_material.insert(material_name.clone(), Value::Object(copied)); }
                    }
                }
                slots.insert(key.clone(), Value::Object(by_material));
                continue;
            }
            let Some(path) = value.as_str() else { continue };
            let Ok(image_bytes) = fs::read(path) else { continue };
            let ext = Path::new(path).extension().and_then(|x| x.to_str()).unwrap_or("png").to_ascii_lowercase();
            let image_relative = format!("assets/textures/{}.{}", digest(&image_bytes), ext);
            let image_target = safe_path(root, &image_relative)?;
            fs::create_dir_all(image_target.parent().unwrap()).map_err(|e| e.to_string())?;
            if !image_target.exists() { crate::atomic_fs::write_atomic(&image_target, &image_bytes, false).map_err(|e| e.to_string())?; }
            slots.insert(key.clone(), json!(image_relative));
        }
    }
    let _ = fs::remove_dir_all(&temp);
    Ok(json!({"asset": relative, "materialSlots": slots, "source": "blend"}))
}

/// Blender's Windows path handling does not accept Rust's extended `\\?\\`
/// prefix, while the workspace uses it to support long project paths.
fn blender_path_arg(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

fn blender_executable() -> PathBuf {
    if let Ok(value) = std::env::var("BLENDER_PATH") {
        let path = PathBuf::from(value.trim_matches('"'));
        if path.is_file() { return path; }
    }
    let candidates = [
        PathBuf::from(r"D:\blender\blender.exe"),
        PathBuf::from(r"C:\Program Files\Blender Foundation\Blender\blender.exe"),
    ];
    candidates.into_iter().find(|path| path.is_file()).unwrap_or_else(|| PathBuf::from("blender"))
}

pub(super) fn describe(root: &Path, relative: &str) -> Result<Value, String> {
    let path = safe_path(root, relative)?;
    let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > 128 * 1024 * 1024 { return Err("Asset exceeds 128 MiB".into()); }
    if path.extension().is_some_and(|s| s == "package") {
        let package = dbpf::Package::open(&path).map_err(|e| e.to_string())?;
        let resources: Vec<_> = package.entries().iter().map(|e| json!({
            "typeId":e.id.type_id,"group":e.id.group,"instance":e.id.instance
        })).collect();
        return Ok(json!({"asset":relative,"path":path,"size":size,"resources":resources}));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(json!({"asset":relative,"path":path,"size":size,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}))
}

pub(super) fn validate(root: &Path, schema: &Schema) -> Option<&'static str> {
    let file = schema.config["asset"].as_str().unwrap_or("");
    if schema.kind == "texture-input" {
        if !schema.config["document"].as_str().unwrap_or("").is_empty() {
            return Some("rasterEditor.buildUnsupported");
        }
        return if file.is_empty() || safe_path(root, file).is_ok_and(|p| p.is_file()) { None }
        else { Some("Texture input is missing") };
    }
    if !file.ends_with(".package") {
        return Some("模型草稿可预览；构建目前需要原生 .package，GLB/LOTM → RW4 编译尚未接入");
    }
    if !schema.config["document"].as_str().unwrap_or("").is_empty() {
        return Some("Property schema 已保存；编辑后的 Property/RW4 编译尚未接入，不能按未编辑原包构建");
    }
    if safe_path(root, file).ok().and_then(|p| dbpf::Package::open(p).ok()).is_none() {
        return Some("Select a valid native asset package");
    }
    None
}

pub(super) fn validate_links(schemas: &[Schema], deps: &BTreeMap<String, Vec<String>>, out: &mut Vec<Diagnostic>) {
    for building in schemas.iter().filter(|s| s.kind == "building-asset") {
        for slot in 0..6 {
            let key = format!("slot{slot}");
            let source = building.config[&key].as_str().unwrap_or("");
            let input = schemas.iter().find(|s| s.id == source && s.kind == "texture-input");
            if input.is_none() || !deps.get(&building.id).is_some_and(|v| v.iter().any(|id| id == source)) {
                diagnostic(out, Some(&building.id), "slot", &format!("{key} must connect to a texture input"));
            } else if !input.unwrap().config["asset"].as_str().unwrap_or("").is_empty() {
                diagnostic(out, Some(&building.id), "compile", "贴图预览已绑定；替换材质槽的 RW4 写回尚未接入，禁止静默忽略贴图构建");
            }
        }
    }
}

#[cfg(test)]
mod raster_draft_tests {
    use super::*;

    #[test]
    fn raster_schema_cannot_be_silently_omitted_from_a_build() {
        let mut schema: Schema = serde_json::from_value(json!({
            "schema_version": 1, "id": "texture-input-1", "kind": "texture-input",
            "config": { "asset": "", "slot": 0, "document": "{\"format\":\"openscp.raster\"}" }
        })).unwrap();
        assert_eq!(validate(Path::new("."), &schema), Some("rasterEditor.buildUnsupported"));
        schema.config["document"] = json!("");
        assert!(validate(Path::new("."), &schema).is_none());
    }
}
