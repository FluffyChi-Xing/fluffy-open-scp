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

import json
import os
import struct
import sys

import bpy


def log(message):
    print(f"[lotm-blend] {message}", flush=True)


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
    (1, "slot1_tint", "sRGB", "base"),
    (2, "slot2_normal", "Non-Color", "normal"),
    (3, "slot3_shader", "Non-Color", None),
    (4, "slot4_palette", "sRGB", None),
    (5, "slot5_interior", "sRGB", "emission"),
)


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

    materials = {}
    for entry in manifest.get("materials", []):
        materials[entry["index"]] = build_material(
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
