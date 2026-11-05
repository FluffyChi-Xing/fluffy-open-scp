"""Rebuild a game LOTM payload as an editable .blend scene.

Invoked by `export_lot_model_blend` after the back end has staged the payload:
one GLB per mesh, the six-slot PNGs per material, and `manifest.json`.

Design notes
- The LOTM mesh GLBs are stock glTF, but some exporters write TEXCOORD_n as
  VEC4 (the fourth component carries a per-vertex seed).  Blender's glTF
  importer crashes on those, so every VEC4 UV accessor is rewritten to VEC2
  here before import.  Nothing else in the mesh is touched.
- Materials are rebuilt from the payload's six-slot maps rather than reused
  from the GLB: the GLB material is a renderer-derived preview, while the
  slots are the game-side source of truth.
- The .blend is self-contained: images are packed, and the payload metadata
  (source TGI, parameter tables, slot legend) is embedded as text blocks.
"""

import importlib.util
import json
import os
import struct
import sys

import bpy


def log(message):
    print(f"[lotm-blend] {message}", flush=True)


def load_b4_shader(bundle_dir):
    """Import the repo's B4 six-slot shader builder from the bundle."""
    path = os.path.join(bundle_dir, "b4_shader.py")
    if not os.path.isfile(path):
        return None
    spec = importlib.util.spec_from_file_location("openscp_b4_shader", path)
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
    except Exception as error:  # missing numpy etc. -> preview fallback
        log(f"b4 shader unavailable: {error!r}")
        return None
    return module


# ---------------------------------------------------------------- GLB rewrite

def _accessor_values(doc, blob, accessor_index, components):
    """Read a (possibly strided) float accessor into a list of tuples."""
    accessor = doc["accessors"][accessor_index]
    view = doc["bufferViews"][accessor["bufferView"]]
    base = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    stride = view.get("byteStride") or components * 4
    values = []
    for index in range(accessor["count"]):
        start = base + index * stride
        values.append(struct.unpack_from("<" + "f" * components, blob, start))
    return values


def _append_blob(doc, blob, payload):
    while len(blob) % 4:
        blob.append(0)
    offset = len(blob)
    blob.extend(payload)
    doc["bufferViews"].append(
        {"buffer": 0, "byteOffset": offset, "byteLength": len(payload)}
    )
    return len(doc["bufferViews"]) - 1


def _rewrite_vec4_uvs(path, temp_dir):
    """Return a path whose TEXCOORD_n accessors are all VEC2."""
    with open(path, "rb") as handle:
        data = bytearray(handle.read())
    if len(data) < 20 or struct.unpack_from("<I", data, 0)[0] != 0x46546C67:
        return path
    json_length = struct.unpack_from("<I", data, 12)[0]
    doc = json.loads(bytes(data[20:20 + json_length]))
    bin_start = 20 + json_length + 8
    blob = bytearray(data[bin_start:])

    replaced = 0
    for mesh in doc.get("meshes", []):
        for primitive in mesh.get("primitives", []):
            attributes = primitive.get("attributes", {})
            for name, accessor_index in list(attributes.items()):
                if not name.startswith("TEXCOORD"):
                    continue
                accessor = doc["accessors"][accessor_index]
                if accessor.get("type") != "VEC4":
                    continue
                values = _accessor_values(doc, blob, accessor_index, 4)
                flat = []
                for value in values:
                    flat.extend(value[:2])
                view_index = _append_blob(doc, blob, struct.pack("<" + "f" * len(flat), *flat))
                doc["accessors"].append(
                    {
                        "bufferView": view_index,
                        "componentType": 5126,
                        "count": accessor["count"],
                        "type": "VEC2",
                    }
                )
                attributes[name] = len(doc["accessors"]) - 1
                replaced += 1
    if not replaced:
        return path

    while len(blob) % 4:
        blob.append(0)
    doc["buffers"] = [{"byteLength": len(blob)}]
    encoded = json.dumps(doc, separators=(",", ":")).encode("utf-8")
    encoded += b" " * ((4 - len(encoded) % 4) % 4)
    total = 12 + 8 + len(encoded) + 8 + len(blob)
    out = bytearray()
    out.extend(struct.pack("<III", 0x46546C67, 2, total))
    out.extend(struct.pack("<II", len(encoded), 0x4E4F534A))
    out.extend(encoded)
    out.extend(struct.pack("<II", len(blob), 0x004E4942))
    out.extend(blob)
    rewritten = os.path.join(temp_dir, "sanitized-" + os.path.basename(path))
    with open(rewritten, "wb") as handle:
        handle.write(out)
    return rewritten


# ------------------------------------------------------------------ materials

SLOT_SPECS = (
    # slot, node label, colour space, packed into Base Color / Normal / Emission
    (1, "slot1_tint", "Non-Color", "base"),
    (2, "slot2_normal", "Non-Color", "normal"),
    (3, "slot3_shader", "Non-Color", None),
    (4, "slot4_palette", "sRGB", None),
    (5, "slot5_interior", "sRGB", "emission"),
)

SLOT_FILENAMES = {
    1: "slot1.png",
    2: "slot2.png",
    3: "slot3.png",
    4: "slot4.png",
    5: "slot5.png",
}


def material_dir_of(entry, bundle_dir):
    slots = entry.get("slots") or {}
    first = next(iter(slots.values()), None)
    if not first:
        return None
    return os.path.dirname(os.path.join(bundle_dir, first))


def build_b4_materials(entry, shader, bundle_dir, suffix=""):
    """Wire the payload material through the pipeline's B4_SixSlots group.

    Returns (lit, data) or None when any slot/param table is missing — the
    caller then falls back to the preview material.
    """
    directory = material_dir_of(entry, bundle_dir)
    params = entry.get("paramsF32")
    param_cols = int(entry.get("paramCols") or 0)
    if not directory or not params or param_cols <= 0:
        return None
    # The group samples a 4-row RGBA table; anything else would reshape wrong.
    if len(params) != param_cols * 16:
        log(
            f"material {entry.get('index')}: param table is {len(params)} floats, "
            f"expected {param_cols * 16}; falling back to preview material"
        )
        return None
    if not all(os.path.isfile(os.path.join(directory, name))
               for name in SLOT_FILENAMES.values()):
        return None
    shader.load_images(directory)
    shader.build_params_image(list(params), param_cols, out_dir=directory)
    group = shader.build_group(bpy.data)
    lit = shader.make_material(f"B4_Lit{suffix}", group, mode="LIT")
    data = shader.make_material(f"B4_Data{suffix}", group, mode="DATA")
    shader.set_uniforms(lit, Param_Cols=float(param_cols))
    shader.set_uniforms(data, Param_Cols=float(param_cols))
    return lit


def build_material(entry, source_label, bundle_dir):
    material = bpy.data.materials.new(name=f"OpenSCP_{entry['index']:02d}_{source_label}")
    material.use_nodes = True
    tree = material.node_tree
    tree.nodes.clear()
    output = tree.nodes.new("ShaderNodeOutputMaterial")
    output.location = (420, 0)
    bsdf = tree.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.location = (120, 0)
    bsdf.inputs["Roughness"].default_value = 0.82
    bsdf.inputs["Metallic"].default_value = 0.0
    if "Emission Strength" in bsdf.inputs:
        bsdf.inputs["Emission Strength"].default_value = 0.0
    tree.links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])

    slot_nodes = {}
    for offset, (slot, label, colour_space, wired) in enumerate(SLOT_SPECS):
        path = (entry.get("slots") or {}).get(str(slot))
        if not path:
            continue
        image = bpy.data.images.load(
            os.path.join(bundle_dir, path), check_existing=True
        )
        image.colorspace_settings.name = colour_space
        node = tree.nodes.new("ShaderNodeTexImage")
        node.name = label
        node.label = label
        node.image = image
        node.location = (-360, 260 - offset * 220)
        slot_nodes[wired] = node
        if wired == "base":
            tree.links.new(node.outputs["Color"], bsdf.inputs["Base Color"])
        elif wired == "emission" and "Emission Color" in bsdf.inputs:
            tree.links.new(node.outputs["Color"], bsdf.inputs["Emission Color"])

    normal_source = slot_nodes.get("normal")
    if normal_source is not None:
        normal_map = tree.nodes.new("ShaderNodeNormalMap")
        normal_map.location = (-80, -220)
        tree.links.new(normal_source.outputs["Color"], normal_map.inputs["Color"])
        tree.links.new(normal_map.outputs["Normal"], bsdf.inputs["Normal"])

    material["openscp_slot0_param_cols"] = int(entry.get("paramCols") or 0)
    return material


# ---------------------------------------------------------------------- scene

# The B4 shader group samples with its own v -> 1-v flip, and the pipeline's
# assets store glTF-convention UVs (the dsh export stores them that way too).
# Blender's glTF importer converts to Blender convention, so restore the source
# convention on the two contract layers.
# glTF TEXCOORD order → pipeline domain names (b4_shader.py's data contract).
CONTRACT_UV_BY_INDEX = {2: "UVBase", 3: "UVTop"}


def contract_uv_layers(mesh):
    layers = list(mesh.uv_layers)
    loops = len(mesh.loops)
    for index, target in CONTRACT_UV_BY_INDEX.items():
        if index >= len(layers):
            continue
        layer = layers[index]
        buf = [0.0] * (loops * 2)
        layer.data.foreach_get("uv", buf)
        for slot in range(1, len(buf), 2):
            buf[slot] = 1.0 - buf[slot]
        layer.data.foreach_set("uv", buf)
        layer.name = target
    # Constant layers carry no information; drop them so the file matches the
    # pipeline export shape. Data-bearing extras (material index/seed) stay.
    for layer in list(mesh.uv_layers):
        if layer.name in ("UVBase", "UVTop"):
            continue
        buf = [0.0] * (loops * 2)
        layer.data.foreach_get("uv", buf)
        us = buf[0::2]
        vs = buf[1::2]
        if max(us) - min(us) < 1e-9 and max(vs) - min(vs) < 1e-9:
            mesh.uv_layers.remove(layer)


def mat_index_attribute(mesh):
    """Per-vertex material index (TEXCOORD_1.x = D3DCOLOR.G/255)."""
    layer = mesh.uv_layers.get("TEXCOORD_1_material") or mesh.uv_layers.get(
        "TEXCOORD_1"
    )
    if layer is None:
        return
    loop_count = len(mesh.loops)
    uvs = [0.0] * (loop_count * 2)
    layer.data.foreach_get("uv", uvs)
    vertex_of = [0] * loop_count
    mesh.loops.foreach_get("vertex_index", vertex_of)
    values = [0.0] * len(mesh.vertices)
    filled = [False] * len(mesh.vertices)
    for loop, vertex in enumerate(vertex_of):
        if not filled[vertex]:
            values[vertex] = uvs[loop * 2]
            filled[vertex] = True
    attribute = mesh.attributes.get("matIndex")
    if attribute is None:
        attribute = mesh.attributes.new("matIndex", "FLOAT", "POINT")
    attribute.data.foreach_set("value", values)


def uv_layer_rename(mesh, names):
    for index, layer in enumerate(mesh.uv_layers):
        if index == 0:
            layer.name = "UVMap"
        elif index < len(names):
            layer.name = names[index]


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    bundle_dir, out_blend = args
    with open(os.path.join(bundle_dir, "manifest.json"), encoding="utf-8") as handle:
        manifest = json.load(handle)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0

    shader = load_b4_shader(bundle_dir)
    materials = {}
    material_entries = manifest.get("materials", [])
    single = len(material_entries) == 1
    for entry in material_entries:
        built = (
            build_b4_materials(
                entry, shader, bundle_dir, suffix="" if single else f"_{entry['index']:02d}"
            )
            if shader is not None
            else None
        )
        materials[entry["index"]] = built or build_material(
            entry, manifest.get("assetName") or "asset", bundle_dir
        )

    temp_dir = os.path.join(bundle_dir, "sanitized")
    os.makedirs(temp_dir, exist_ok=True)
    imported = 0
    for mesh_entry in manifest.get("meshes", []):
        before = set(bpy.data.objects)
        source = os.path.join(bundle_dir, mesh_entry["file"])
        prepared = _rewrite_vec4_uvs(source, temp_dir)
        bpy.ops.import_scene.gltf(filepath=prepared)
        added = [obj for obj in bpy.data.objects if obj not in before]
        material = materials.get(mesh_entry.get("material", 0))
        names = manifest.get("uvLayerNames") or []
        for obj in added:
            if obj.type != "MESH":
                continue
            imported += 1
            obj.data.materials.clear()
            if material is not None:
                obj.data.materials.append(material)
            uv_layer_rename(obj.data, names)
            # Contract UVs and the per-vertex material index are what the B4
            # group consumes; set them after the importer's own naming.
            mat_index_attribute(obj.data)
            contract_uv_layers(obj.data)
        # The glTF importer keeps the Y-up -> Z-up conversion on the object.
        # Bake it so the mesh data matches the pipeline export (identity
        # object transform, Z-up vertex coordinates).
        mesh_added = [obj for obj in added if obj.type == "MESH"]
        if mesh_added:
            bpy.ops.object.select_all(action="DESELECT")
            for obj in mesh_added:
                obj.select_set(True)
            bpy.context.view_layer.objects.active = mesh_added[0]
            bpy.ops.object.transform_apply(
                location=True, rotation=True, scale=True
            )

    # Preview rig matching the pipeline's original-asset files: dark world and
    # two suns so the file shows the asset correctly right after opening.
    world = bpy.data.worlds.new("OpenSCP_World")
    world.use_nodes = True
    background = next(
        node for node in world.node_tree.nodes if node.type == "BACKGROUND"
    )
    background.inputs[0].default_value = (0.012, 0.03, 0.07, 1.0)
    background.inputs[1].default_value = 1.0
    scene.world = world
    for name, energy, rotation, angle in (
        ("Sun", 5.0, (1.086, 0.0, -0.322), 0.03490658476948738),
        ("Fill", 1.6, (1.113, 0.0, 2.257), 0.009180432185530663),
    ):
        data = bpy.data.lights.new(name, type="SUN")
        data.energy = energy
        data.angle = angle
        light = bpy.data.objects.new(name, data)
        light.rotation_euler = rotation
        scene.collection.objects.link(light)

    # Self-contained documentation and payload data inside the file.
    readme = bpy.data.texts.new("OpenSCP_README")
    readme.write(manifest.get("readme") or "")
    legend = bpy.data.texts.new("OpenSCP_manifest.json")
    legend.write(json.dumps(manifest, ensure_ascii=False, indent=2))
    for entry in manifest.get("materials", []):
        params = entry.get("paramsF32")
        if not params:
            continue
        block = bpy.data.texts.new(f"params_mat{entry['index']:02d}.json")
        block.write(
            json.dumps(
                {
                    "paramCols": entry.get("paramCols"),
                    "values": params,
                },
                ensure_ascii=False,
            )
        )

    bpy.ops.file.pack_all()
    os.makedirs(os.path.dirname(os.path.abspath(out_blend)) or ".", exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=out_blend, compress=True)
    log(f"meshes={imported} materials={len(materials)} -> {out_blend}")


if __name__ == "__main__":
    main()
