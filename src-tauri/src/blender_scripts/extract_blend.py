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
        return [round(float(item), 8) for item in value]
    if isinstance(value, bool):
        return bool(value)
    if isinstance(value, (int, float)):
        return round(float(value), 8)
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
            values = [None] * len(attribute.data)
            if attribute.data_type == "FLOAT_VECTOR":
                attribute.data.foreach_get("vector", values)
                corner_attributes[attribute.name] = [
                    [round(value[0], 8), round(value[1], 8), round(value[2], 8)]
                    for value in values
                ]
            else:
                attribute.data.foreach_get("vector", values)
                corner_attributes[attribute.name] = [
                    [round(value[0], 8), round(value[1], 8), 0.0] for value in values
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


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    source, output = args
    register_reader_schema()
    try:
        bpy.ops.wm.open_mainfile(filepath=source)
        scene = bpy.context.scene
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
        bundle = {
            "version": 1,
            "frame_color": [round(float(value), 12) for value in scene.ny_facade.frame_color],
            "unit": (
                scene.unit_settings.scale_length
                if scene.unit_settings.system != "NONE"
                else 1.0
            ),
            "objects": [dump_object(obj) for obj in bpy.data.objects],
            "materials": [dump_material(material) for material in bpy.data.materials],
            "node_trees": trees,
            "images": images,
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
