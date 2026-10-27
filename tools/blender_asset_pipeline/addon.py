"""OpenSCP Six-Slot Asset Pipeline Blender add-on.

Workflow: validate exported slots -> import a white model or mesh.json -> attach
the four UV contracts -> build the six-slot material -> export a contract GLB.
The exact B4 shader can be supplied through ``shader_script`` and is loaded from
the research workspace; a simple preview material is used when it is unavailable.
"""
from __future__ import annotations

import importlib.util
import json
import os
import struct
from pathlib import Path

import bpy
from bpy.props import BoolProperty, IntProperty, StringProperty
from bpy.types import Operator, Panel, PropertyGroup

from .core import validate_asset, write_manifest

bl_info = {"name": "OpenSCP Six-Slot Asset Pipeline", "author": "OpenSCP", "version": (0, 1, 0), "blender": (3, 1, 0), "category": "Import-Export"}


def _asset_dir(s):
    return Path(bpy.path.abspath(s.asset_dir)).expanduser()


def _load_shader(path):
    if not path or not Path(path).is_file():
        return None
    spec = importlib.util.spec_from_file_location("openscp_b4_shader", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _mesh_from_json(root):
    data = json.loads((root / "mesh.json").read_text(encoding="utf-8"))
    verts = [tuple(v["position"]) for v in data["vertices"]]
    faces = [tuple(t) for t in data["triangles"]]
    me = bpy.data.meshes.new("OpenSCP_Asset_Mesh")
    me.from_pydata(verts, [], faces)
    me.update()
    uv = [v["uv"] for v in data["vertices"]]
    columns = [float(v["column"]) / 255.0 for v in data["vertices"]]
    loop_vertex = [loop.vertex_index for loop in me.loops]
    for name, offset in (("UVBase", 0), ("UVTop", 2)):
        layer = me.uv_layers.new(name=name)
        for loop, vi in zip(layer.data, loop_vertex):
            loop.uv = uv[vi][offset:offset + 2]
    attr = me.attributes.new("matIndex", "FLOAT", "POINT")
    for item, value in zip(attr.data, columns):
        item.value = value
    obj = bpy.data.objects.new("OpenSCP_Asset", me)
    bpy.context.collection.objects.link(obj)
    return obj


def _import_white_model(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(path))
    created = [o for o in bpy.data.objects if o not in before and o.type == 'MESH']
    if not created:
        raise RuntimeError("white model contains no mesh")
    obj = created[0]
    me = obj.data
    # A new white model has no game UVs. Generate a stable world box projection
    # so an agent can iterate immediately; replace with probe UVs for fidelity.
    for layer in list(me.uv_layers):
        me.uv_layers.remove(layer)
    base = me.uv_layers.new(name="UVBase")
    top = me.uv_layers.new(name="UVTop")
    xs = [v.co.x for v in me.vertices]; ys = [v.co.y for v in me.vertices]
    zs = [v.co.z for v in me.vertices]
    sx = max(max(xs) - min(xs), 1e-6); sy = max(max(ys) - min(ys), 1e-6); sz = max(max(zs) - min(zs), 1e-6)
    for loop in me.loops:
        v = me.vertices[loop.vertex_index].co
        base.data[loop.index].uv = ((v.x - min(xs)) / sx, (v.z - min(zs)) / sz)
        top.data[loop.index].uv = ((v.y - min(ys)) / sy, (v.z - min(zs)) / sz)
    attr = me.attributes.get("matIndex") or me.attributes.new("matIndex", "FLOAT", "POINT")
    for item in attr.data:
        item.value = 0.0
    return obj


def _fallback_material(root):
    mat = bpy.data.materials.new("OpenSCP_SixSlot_Preview")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes.get("Principled BSDF")
    img = bpy.data.images.load(str(root / "slot1.png"), check_existing=True)
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.72
    mat["openscp_pipeline_mode"] = "preview"
    return mat


def _attach_material(obj, root, shader_script):
    shader = _load_shader(shader_script)
    if shader:
        shader.load_images(str(root))
        params = json.loads((root / "params.json").read_text(encoding="utf-8"))
        shader.build_params_image([c for row in params["values"] for c in row], params["width"], out_dir=str(root))
        group = shader.build_group(bpy.data)
        mat = shader.make_material("B4_Lit", group, mode="LIT")
        shader.set_uniforms(mat, Param_Cols=float(params["width"]))
    else:
        mat = _fallback_material(root)
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    return mat


def _contract_uv(obj):
    me = obj.data
    base = me.uv_layers.get("UVBase")
    top = me.uv_layers.get("UVTop")
    if not base or not top:
        raise RuntimeError("UVBase and UVTop are required before export")
    base_data = [tuple(x.uv) for x in base.data]
    top_data = [tuple(x.uv) for x in top.data]
    for layer in list(me.uv_layers):
        me.uv_layers.remove(layer)
    me.uv_layers.new(name="UV0")
    uv1 = me.uv_layers.new(name="UV1")
    base_layer = me.uv_layers.new(name="UVBase")
    top_layer = me.uv_layers.new(name="UVTop")
    attr = me.attributes.get("matIndex")
    values = [x.value for x in attr.data] if attr else []
    for i, loop in enumerate(uv1.data):
        vi = me.loops[i].vertex_index
        loop.uv = (values[vi] if vi < len(values) else 0.0, 0.5)
    for layer, data in ((base_layer, base_data), (top_layer, top_data)):
        for item, value in zip(layer.data, data):
            item.uv = value
    me.uv_layers.active_index = 0


def _glb_attributes(path):
    data = Path(path).read_bytes()
    pos, doc = 12, None
    while pos < len(data):
        size, kind = struct.unpack_from("<II", data, pos)
        if kind == 0x4E4F534A:
            doc = json.loads(data[pos + 8:pos + 8 + size])
            break
        pos += size + 8
    return sorted(doc["meshes"][0]["primitives"][0]["attributes"].keys()) if doc else []


class OSPG_Settings(PropertyGroup):
    asset_dir: StringProperty(name="Asset directory / 资产目录", subtype="DIR_PATH")
    white_model: StringProperty(name="White model GLB / 白模 GLB", subtype="FILE_PATH")
    shader_script: StringProperty(name="B4 shader.py / 着色器脚本", subtype="FILE_PATH")
    output_glb: StringProperty(name="Output GLB / 输出 GLB", subtype="FILE_PATH", default="//openscp_asset.glb")
    manifest: StringProperty(name="Manifest / 清单", subtype="FILE_PATH", default="//openscp_asset_manifest.json")
    material_columns: IntProperty(name="Columns / 材质列", default=114, min=1, max=256)
    use_exact_shader: BoolProperty(name="Use exact B4 shader / 使用精确着色器", default=True)


class OSPG_OT_Validate(Operator):
    bl_idname = "openscp_asset.validate"
    bl_label = "Validate slots / 校验槽位"
    def execute(self, context):
        result = validate_asset(_asset_dir(context.scene.opscp_pipeline))
        write_manifest(_asset_dir(context.scene.opscp_pipeline), bpy.path.abspath(context.scene.opscp_pipeline.manifest), result)
        self.report({'INFO' if result['ok'] else 'ERROR'}, json.dumps(result, ensure_ascii=False))
        return {'FINISHED'}


class OSPG_OT_Build(Operator):
    bl_idname = "openscp_asset.build"
    bl_label = "Build six-slot asset / 构建六槽位资产"
    def execute(self, context):
        s = context.scene.opscp_pipeline
        root = _asset_dir(s)
        result = validate_asset(root)
        if not result["ok"]:
            self.report({'ERROR'}, "; ".join(result["errors"]))
            return {'CANCELLED'}
        white = Path(bpy.path.abspath(s.white_model)) if s.white_model else None
        obj = _import_white_model(white) if white and white.is_file() else _mesh_from_json(root)
        _attach_material(obj, root, bpy.path.abspath(s.shader_script) if s.use_exact_shader else "")
        for item in bpy.context.selected_objects:
            item.select_set(False)
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        self.report({'INFO'}, f"Built {result['vertices']} vertices / {result['triangles']} triangles")
        return {'FINISHED'}


class OSPG_OT_Export(Operator):
    bl_idname = "openscp_asset.export_contract"
    bl_label = "Export contract GLB / 导出契约 GLB"
    def execute(self, context):
        obj = context.active_object
        if not obj or obj.type != 'MESH':
            self.report({'ERROR'}, "Select the built mesh / 请选中构建后的网格")
            return {'CANCELLED'}
        _contract_uv(obj)
        for item in bpy.context.selected_objects:
            item.select_set(False)
        obj.select_set(True)
        context.view_layer.objects.active = obj
        path = bpy.path.abspath(context.scene.opscp_pipeline.output_glb)
        bpy.ops.export_scene.gltf(filepath=path, export_format='GLB', use_selection=True, export_yup=False, export_texcoords=True, export_normals=True, export_materials='NONE')
        attrs = _glb_attributes(path)
        required = {"TEXCOORD_0", "TEXCOORD_1", "TEXCOORD_2", "TEXCOORD_3"}
        if not required.issubset(attrs):
            self.report({'ERROR'}, f"Contract failed: {attrs}")
            return {'CANCELLED'}
        self.report({'INFO'}, f"Exported {path}; contract OK")
        return {'FINISHED'}


class OSPG_PT_Main(Panel):
    bl_label = "OpenSCP Asset Pipeline / 资产流水线"
    bl_idname = "OSPG_PT_main"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'OpenSCP'
    def draw(self, context):
        l = self.layout
        s = context.scene.opscp_pipeline
        l.prop(s, "asset_dir")
        l.prop(s, "white_model")
        l.prop(s, "shader_script")
        l.prop(s, "use_exact_shader")
        l.prop(s, "output_glb")
        l.operator("openscp_asset.validate")
        l.operator("openscp_asset.build")
        l.operator("openscp_asset.export_contract")


CLASSES = (OSPG_Settings, OSPG_OT_Validate, OSPG_OT_Build, OSPG_OT_Export, OSPG_PT_Main)
def register():
    for cls in CLASSES: bpy.utils.register_class(cls)
    bpy.types.Scene.opscp_pipeline = bpy.props.PointerProperty(type=OSPG_Settings)
def unregister():
    del bpy.types.Scene.opscp_pipeline
    for cls in reversed(CLASSES): bpy.utils.unregister_class(cls)
if __name__ == "__main__": register()
