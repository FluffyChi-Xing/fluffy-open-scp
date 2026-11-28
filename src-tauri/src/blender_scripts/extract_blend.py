"""Extract a plugin-authored .blend into a neutral JSON bundle.

OpenSCP's native Rust exporter consumes this bundle; this script contains **no
export logic**.  It only reads Blender's data model:

- object custom properties and the `ny_layers` collection (the plug-in stores
  authoring state as `settings_json` strings);
- mesh vertices / polygons / loops / UV layers / face attributes;
- material custom properties and node graphs (nodes, links, socket defaults);
- scene unit scale and image references.

The `NYLayer` stub below declares the same six stored fields as the Asset
Editor plug-in so Blender binds the existing ID-property data.  It is our own
reader schema, not the plug-in package: no `ny_facade` import or registration
happens here.
"""

import base64
import bpy
import json
import os
import sys


class NYSceneSettings(bpy.types.PropertyGroup):
    """Minimal scene reader schema: only the fields the exporter consumes."""

    frame_color: bpy.props.FloatVectorProperty(size=4, subtype="COLOR", default=(0.8, 0.85, 0.87, 1.0))


class NYLayer(bpy.types.PropertyGroup):
    name: bpy.props.StringProperty(default="立面")
    uid: bpy.props.StringProperty()
    face_ids: bpy.props.StringProperty(default="[]")
    topology: bpy.props.StringProperty(default="[]")
    settings_json: bpy.props.StringProperty(default="{}")
    role: bpy.props.StringProperty(default="WINDOW")


def register_reader_schema():
    bpy.utils.register_class(NYSceneSettings)
    bpy.utils.register_class(NYLayer)
    bpy.types.Scene.ny_facade = bpy.props.PointerProperty(type=NYSceneSettings)
    bpy.types.Object.ny_layers = bpy.props.CollectionProperty(type=NYLayer)


def register_reader_schema_unregister():
    if hasattr(bpy.types.Object, "ny_layers"):
        del bpy.types.Object.ny_layers
    if hasattr(bpy.types.Scene, "ny_facade"):
        del bpy.types.Scene.ny_facade
    bpy.utils.unregister_class(NYLayer)
    bpy.utils.unregister_class(NYSceneSettings)


def scalar(value):
    if isinstance(value, (bool, int, float, str)):
        return value
    try:
        return [round(float(item), 8) for item in value]
    except TypeError:
        return None


def node_value(socket):
    """Default value of a socket, or None when it has no readable default."""
    if not hasattr(socket, "default_value"):
        return None
    value = socket.default_value
    if hasattr(value, "__len__"):
        return [float(item) for item in value]
    if isinstance(value, bool):
        return bool(value)
    if isinstance(value, (int, float)):
        return value
    return value


def dump_tree(tree):
    if tree is None:
        return None
    nodes = []
    for node in tree.nodes:
        item = {
            "name": node.name,
            "label": node.label,
            "type": node.type,
            "inputs": [
                {
                    "name": socket.name,
                    "linked": bool(socket.is_linked),
                    "value": node_value(socket),
                }
                for socket in node.inputs
            ],
            "outputs": [
                {
                    "name": socket.name,
                    "linked": bool(socket.is_linked),
                    "value": node_value(socket),
                }
                for socket in node.outputs
            ],
        }
        if node.type == "TEX_IMAGE":
            item["image"] = node.image.name if node.image else None
            item["image_size"] = (
                [node.image.size[0], node.image.size[1]] if node.image else None
            )
        elif node.type == "UVMAP":
            item["uv_map"] = node.uv_map
        elif node.type == "GROUP" and node.node_tree:
            item["node_tree"] = node.node_tree.name
        elif node.type == "ATTRIBUTE":
            item["attribute_name"] = node.attribute_name
        nodes.append(item)
    links = [
        {
            "from_node": link.from_node.name,
            "from_socket": link.from_socket.name,
            "to_node": link.to_node.name,
            "to_socket": link.to_socket.name,
        }
        for link in tree.links
    ]
    return {"name": tree.name, "nodes": nodes, "links": links}


def dump_mesh(mesh):
    uv_layers = {}
    for layer in mesh.uv_layers:
        values = [0.0] * (len(mesh.loops) * 2)
        layer.data.foreach_get("uv", values)
        uv_layers[layer.name] = [
            [round(values[index], 8), round(values[index + 1], 8)]
            for index in range(0, len(values), 2)
        ]
    face_attributes = {}
    for attribute in mesh.attributes:
        if attribute.domain != "FACE":
            continue
        if attribute.data_type not in {"FLOAT", "INT", "BOOLEAN"}:
            continue
        try:
            values = [None] * len(attribute.data)
            attribute.data.foreach_get("value", values)
            face_attributes[attribute.name] = [
                bool(value) if attribute.data_type == "BOOLEAN" else scalar(value)
                for value in values
            ]
        except (RuntimeError, TypeError):
            continue
    corner_attributes = {}
    for attribute in mesh.attributes:
        if attribute.domain != "CORNER":
            continue
        if attribute.data_type not in {"FLOAT_VECTOR", "FLOAT2"}:
            continue
        try:
            # foreach_get("vector", ...) fills ONE flat buffer of count*dim
            # floats.  A list of per-element vectors raises RuntimeError
            # ("internal error setting the array"), which silently dropped every
            # corner attribute - including NY_DecalCoord, the saved decal
            # coordinates the exporter must reuse instead of re-projecting.
            dimension = 3 if attribute.data_type == "FLOAT_VECTOR" else 2
            count = len(attribute.data)
            values = [0.0] * (count * dimension)
            attribute.data.foreach_get("vector", values)
            if dimension == 2:
                corner_attributes[attribute.name] = [
                    [round(values[index], 8), round(values[index + 1], 8), 0.0]
                    for index in range(0, count * 2, 2)
                ]
            else:
                corner_attributes[attribute.name] = [
                    [
                        round(values[index], 8),
                        round(values[index + 1], 8),
                        round(values[index + 2], 8),
                    ]
                    for index in range(0, count * 3, 3)
                ]
        except (RuntimeError, TypeError):
            continue
    polygons = []
    for polygon in mesh.polygons:
        polygons.append(
            {
                "vertices": list(polygon.vertices),
                "loops": list(polygon.loop_indices),
                "normal": [round(value, 8) for value in polygon.normal],
                "material": polygon.material_index,
                "area": round(polygon.area, 10),
            }
        )
    vertices = [[round(value, 8) for value in vertex.co] for vertex in mesh.vertices]
    return {
        "vertices": vertices,
        "polygons": polygons,
        "uv_layers": uv_layers,
        "face_attributes": face_attributes,
        "corner_attributes": corner_attributes,
        "materials": [material.name if material else None for material in mesh.materials],
    }


def dump_object(obj):
    item = {
        "name": obj.name,
        "props": {key: obj[key] for key in obj.keys()},
        "layers": [
            {
                "name": layer.name,
                "uid": layer.uid,
                "role": layer.role,
                "face_ids": layer.face_ids,
                "topology": layer.topology,
                "settings_json": layer.settings_json,
            }
            for layer in obj.ny_layers
        ],
        "matrix": [round(value, 8) for row in obj.matrix_world for value in row],
    }
    if obj.type == "MESH":
        item["mesh"] = dump_mesh(obj.data)
    return item


def dump_material(material):
    props = {key: material[key] for key in material.keys()}
    item = {"name": material.name, "props": props, "tree": None}
    if material.use_nodes and material.node_tree:
        item["tree"] = "MAT:" + material.name
    return item


def used_materials(meshes):
    materials = set()
    for obj in meshes:
        slots = obj.data.materials
        for index in {face.material_index for face in obj.data.polygons}:
            if index < len(slots) and slots[index]:
                materials.add(slots[index])
    return sorted(materials, key=lambda item: item.name)


def packed_window_sources(materials, known_packs):
    windows, images = {}, {}
    for material in materials:
        signature = material.get("ny_resource_signature", "")
        fields = signature.split(":", 2) if isinstance(signature, str) else []
        if (len(fields) != 3 or fields[1] in known_packs
                or material.get("ny_kind") != "WINDOW" or not material.node_tree):
            continue
        pack = fields[1]
        pending = [(material.node_tree, None)]
        seen = set()
        while pending:
            tree, binding = pending.pop()
            if tree in seen:
                continue
            seen.add(tree)
            pending.extend((node.node_tree, node) for node in tree.nodes
                           if node.type == "GROUP" and node.node_tree)
            # These texture-node numbers are the five bindings in
            # resources.py::configure_window. Read the effective preview binding,
            # including cached exterior sheets, and the exact float table.
            nodes = [tree.nodes.get(name) for name in (
                "Image Texture.004", "Image Texture.006", "Image Texture.008",
                "Image Texture.010", "Image Texture.014", "Image Texture",
            )]
            if any(node is None or node.type != "TEX_IMAGE" or not node.image for node in nodes):
                continue
            *slots, params = [node.image for node in nodes]
            if not all(
                image.name.startswith((f"NY_PACK_{pack}_slot{slot}", f"NY_Win3_{pack}_{slot}_"))
                and image.packed_file
                and image.packed_file.data.startswith(b"\x89PNG\r\n\x1a\n")
                for slot, image in enumerate(slots, 1)
            ):
                continue
            if "_Parameters" not in params.name or params.size[0] < 1 or params.size[1] != 4:
                continue
            stride = params.size[0] * 4
            rect = list(params.pixels[2 * stride:2 * stride + 4])
            row = list(params.pixels[3 * stride:3 * stride + 4])
            if len(rect) != 4 or len(row) != 4:
                continue
            if binding is None or "Palette Row" not in binding.inputs:
                continue
            windows[material.name] = {
                "images": [image.name for image in slots],
                "rect": rect,
                "row": row,
                "palette_row": binding.inputs["Palette Row"].default_value,
            }
            for image in slots:
                if image.name not in images:
                    images[image.name] = base64.b64encode(image.packed_file.data).decode("ascii")
            break
    return windows, images


def packed_cutout_sources(materials, images):
    cutouts = {}
    for material in materials:
        if not material.node_tree:
            continue
        def source(node, socket):
            if node is None or (isinstance(socket, str) and socket not in node.inputs):
                return None
            links = node.inputs[socket].links
            return links[0].from_node if len(links) == 1 else None

        output = next((n for n in material.node_tree.nodes
                       if n.type == "OUTPUT_MATERIAL" and n.is_active_output), None)
        shader = source(output, "Surface")
        if shader is None or shader.type != "BSDF_PRINCIPLED":
            continue
        clip = source(shader, "Alpha")
        mix = source(shader, "Base Color")
        normal_map = source(shader, "Normal")
        if (clip is None or clip.type != "MATH" or clip.operation != "GREATER_THAN"
                or clip.inputs[1].is_linked or clip.inputs[1].default_value != 0.5
                or mix is None or mix.type != "MIX_RGB" or mix.blend_type != "MULTIPLY"
                or mix.inputs[0].is_linked or mix.inputs[0].default_value != 1.0
                or mix.inputs[2].is_linked
                or normal_map is None or normal_map.type != "NORMAL_MAP"
                or normal_map.space != "TANGENT" or normal_map.inputs["Strength"].is_linked):
            continue
        color = source(clip, 0)
        normal = source(normal_map, "Color")
        if (color is None or normal is None or color != source(mix, 1)
                or clip.inputs[0].links[0].from_socket.name != "Alpha"
                or mix.inputs[1].links[0].from_socket.name != "Color"
                or normal_map.inputs["Color"].links[0].from_socket.name != "Color"):
            continue
        uv = source(color, "Vector")
        if uv is None or uv.type != "UVMAP" or not uv.uv_map or uv != source(normal, "Vector"):
            continue
        if not all(
            node.type == "TEX_IMAGE" and node.extension == "REPEAT"
            and node.interpolation == "Linear" and node.image and node.image.packed_file
            and node.image.packed_file.data.startswith(b"\x89PNG\r\n\x1a\n")
            for node in (color, normal)
        ):
            continue
        if (color.image.colorspace_settings.name != "sRGB"
                or normal.image.colorspace_settings.name != "Non-Color"):
            continue
        cutouts[material.name] = {
            "uv": uv.uv_map, "color": color.image.name, "normal": normal.image.name,
            "tint": list(mix.inputs[2].default_value)[:3],
            "normal_strength": normal_map.inputs["Strength"].default_value,
        }
        for node in (color, normal):
            if node.image.name not in images:
                images[node.image.name] = base64.b64encode(node.image.packed_file.data).decode("ascii")
    return cutouts


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    source, output = args[:2]
    known_packs = set()
    if len(args) == 3:
        with open(args[2], encoding="utf-8") as handle:
            known_packs.update(json.load(handle)["packs"])
    register_reader_schema()
    try:
        bpy.ops.wm.open_mainfile(filepath=source)
        scene = bpy.context.scene
        view_layer = bpy.context.view_layer
        # The Asset Editor exports meshes selected in the view layer.
        # Use its visible meshes for the headless import instead of all data-blocks.
        meshes = [
            obj
            for obj in view_layer.objects
            if obj.type == "MESH" and obj.visible_get(view_layer=view_layer)
        ]
        trees = {}
        for tree in bpy.data.node_groups:
            trees[tree.name] = dump_tree(tree)
        for material in bpy.data.materials:
            if material.use_nodes and material.node_tree:
                trees["MAT:" + material.name] = dump_tree(material.node_tree)
        images = [
            {
                "name": image.name,
                "size": [image.size[0], image.size[1]],
                "filepath": image.filepath,
                "packed": bool(image.packed_file),
            }
            for image in bpy.data.images
        ]
        materials = used_materials(meshes)
        packed_windows, packed_images = packed_window_sources(materials, known_packs)
        packed_cutouts = packed_cutout_sources(materials, packed_images)
        bundle = {
            "version": 1,
            "frame_color": [round(float(value), 12) for value in scene.ny_facade.frame_color],
            "unit": (
                scene.unit_settings.scale_length
                if scene.unit_settings.system != "NONE"
                else 1.0
            ),
            "objects": [dump_object(obj) for obj in meshes],
            "materials": [dump_material(material) for material in bpy.data.materials],
            "node_trees": trees,
            "images": images,
            "packed_windows": packed_windows,
            "packed_window_images": packed_images,
            "packed_cutouts": packed_cutouts,
        }
        os.makedirs(os.path.dirname(os.path.abspath(output)) or ".", exist_ok=True)
        with open(output, "w", encoding="utf-8") as handle:
            json.dump(bundle, handle, ensure_ascii=False, separators=(",", ":"))
        print(
            "OPENSCP_EXTRACT_OK",
            len(bundle["objects"]),
            len(bundle["materials"]),
            len(bundle["node_trees"]),
            output,
        )
    finally:
        register_reader_schema_unregister()


if __name__ == "__main__":
    main()
