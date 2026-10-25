"""building4 六槽位着色器 —— openscp refinedRender.ts 的逐行 Blender 移植。

对应源码：src/pages/packages/components/property-editor/refinedRender.ts
  - attachTintShader 的 <map_fragment> 注入段（L819-1032）
  - <normal_fragment_maps> 注入段（L1034-1065）
  - 纹理装载口径（L282-356：flipY=false / palette Nearest+sRGB / interior REPEAT+sRGB）

数据契约（与 openscp 导出器一致，must match）：
  1. facade 世界投影 UV：UVBase(uv2, TEXCOORD_2) / UVTop(uv3, TEXCOORD_3)，单位=平铺次数
  2. 逐顶点材质索引 matIndex（TEXCOORD_1.x = D3DCOLOR.G/255）
  3. slot0 参数表 = 114 列 × 4 行 RGBA float DataTexture（Nearest）
  4. slot1 tint / slot2 normal / slot3 shader / slot4 palette / slot5 interior

V 方向：openscp 用 `flipY=false`（PNG 首行 = v0），Blender 的 v0 在图片底部，
所以除参数表（自建 float 图，按同样口径写入）外，采样前统一做 v' = 1 - v。
"""

import bpy
import json
import math
import os
import os
import numpy as np

IMAGES = {}
PARAM_IMG = None


# ------------------------------------------------------------------ 节点原语
def _math(nt, op, a, b=None, c=None, clamp=False):
    n = nt.nodes.new("ShaderNodeMath")
    n.operation = op
    n.use_clamp = clamp
    for val, idx in ((a, 0), (b, 1), (c, 2)):
        if val is None:
            continue
        if isinstance(val, (int, float)):
            n.inputs[idx].default_value = float(val)
        else:
            nt.links.new(val, n.inputs[idx])
    return n.outputs[0]


def _vmath(nt, op, a, b=None, c=None, scale=None):
    n = nt.nodes.new("ShaderNodeVectorMath")
    n.operation = op
    for val, idx in ((a, 0), (b, 1), (c, 2)):
        if val is None:
            continue
        if isinstance(val, (tuple, list)):
            v = tuple(float(x) for x in val)
            n.inputs[idx].default_value = v if len(v) == 3 else v + (0.0,) * (3 - len(v))
        elif isinstance(val, (int, float)):
            n.inputs[idx].default_value = (float(val),) * 3
        else:
            nt.links.new(val, n.inputs[idx])
    if scale is not None:
        if isinstance(scale, (int, float)):
            n.inputs["Scale"].default_value = float(scale)
        else:
            nt.links.new(scale, n.inputs["Scale"])
    return n.outputs[0]


def _mix(nt, fac, a, b, blend="MIX"):
    """RGBA Mix；a/b 可为 socket、float 或 (r,g,b[,a])。"""
    n = nt.nodes.new("ShaderNodeMix")
    n.data_type = "RGBA"
    n.blend_type = blend

    def put(idx, val):
        if isinstance(val, (int, float)):
            n.inputs[idx].default_value = (float(val),) * 4 if idx in (6, 7) else float(val)
        elif isinstance(val, tuple):
            n.inputs[idx].default_value = tuple(val) if len(val) == 4 else (*val, 1.0)
        elif isinstance(val, (tuple, list)):
            n.inputs[idx].default_value = tuple(val)
        else:
            nt.links.new(val, n.inputs[idx])

    put(0, fac)
    put(6, a)
    put(7, b)
    return n.outputs[2]


def _mixf(nt, fac, a, b):
    n = nt.nodes.new("ShaderNodeMix")
    n.data_type = "FLOAT"
    for val, idx in ((fac, 0), (a, 2), (b, 3)):
        if isinstance(val, (int, float)):
            n.inputs[idx].default_value = float(val)
        else:
            nt.links.new(val, n.inputs[idx])
    return n.outputs[0]


def _mul(nt, color, scal):
    n = nt.nodes.new("ShaderNodeMix")
    n.data_type = "RGBA"
    n.blend_type = "MULTIPLY"
    n.inputs[0].default_value = 1.0
    put = lambda idx, val: nt.links.new(val, n.inputs[idx]) if not isinstance(val, (int, float, tuple)) else None
    if isinstance(color, tuple):
        n.inputs[6].default_value = color if len(color) == 4 else (*color, 1.0)
    else:
        nt.links.new(color, n.inputs[6])
    if isinstance(scal, (int, float)):
        n.inputs[7].default_value = (float(scal),) * 3 + (1.0,)
    elif isinstance(scal, tuple):
        n.inputs[7].default_value = scal if len(scal) == 4 else (*scal, 1.0)
    else:
        nt.links.new(scal, n.inputs[7])
    return n.outputs[2]


def _sep(nt, col):
    n = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(col, n.inputs[0])
    return n.outputs


def _sepxyz(nt, vec):
    n = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(vec, n.inputs[0])
    return n.outputs


def _comb(nt, x, y, z=0.0):
    n = nt.nodes.new("ShaderNodeCombineXYZ")
    for val, idx in ((x, 0), (y, 1), (z, 2)):
        if isinstance(val, (int, float)):
            n.inputs[idx].default_value = float(val)
        else:
            nt.links.new(val, n.inputs[idx])
    return n.outputs[0]


def _flip_v(nt, vec):
    """v' = 1 - v（对应 openscp 的 flipY=false 采样口径）"""
    return _vmath(nt, "MULTIPLY_ADD", vec, (1.0, -1.0, 0.0), (0.0, 1.0, 0.0))


def _tex(nt, img, vec, label, colorspace, interp="Linear", flip=True, repeat=True):
    n = nt.nodes.new("ShaderNodeTexImage")
    n.image = img
    n.label = label
    n.interpolation = interp
    n.extension = "REPEAT" if repeat else "EXTEND"
    img.colorspace_settings.name = colorspace
    nt.links.new(_flip_v(nt, vec) if flip else vec, n.inputs["Vector"])
    return n


# ------------------------------------------------------------------ 参数表
def build_params_image(params_f32, cols, out_dir=None):
    """slot0 f32 → 32bit EXR（cols × 4，行 0=palOrigin）。EXR 保精度且可打包。"""
    global PARAM_IMG
    texels = np.array(params_f32, dtype=np.float32).reshape(-1, 4).reshape(4, cols, 4)
    img = bpy.data.images.new("slot0_params_tmp", width=cols, height=4, float_buffer=True, alpha=True)
    img.colorspace_settings.name = "Non-Color"
    # Blender 像素自下而上：data row0 写到图片顶部（与 PNG 一致，便于统一 flip）
    px = np.zeros((4, cols, 4), dtype=np.float32)
    for k in range(4):
        px[3 - k] = texels[k]
    img.pixels.foreach_set(px.ravel())
    img.update()
    path = (out_dir or bpy.app.tempdir) + "\\slot0_params.exr"
    img.file_format = "OPEN_EXR"
    img.filepath_raw = path
    img.save()
    bpy.data.images.remove(img)
    PARAM_IMG = bpy.data.images.load(path)
    PARAM_IMG.colorspace_settings.name = "Non-Color"
    PARAM_IMG.alpha_mode = "CHANNEL_PACKED"
    return PARAM_IMG


# ------------------------------------------------------------------ 节点组
def build_group(node_tree_data=None):
    bpy_data = node_tree_data or bpy.data
    nt = bpy_data.node_groups.new("B4_SixSlots", "ShaderNodeTree")
    it = nt.interface

    def inp(nm, st, default=None):
        s = it.new_socket(name=nm, in_out="INPUT", socket_type=st)
        if default is not None:
            s.default_value = default
        return s

    def outp(nm, st):
        it.new_socket(name=nm, in_out="OUTPUT", socket_type=st)

    inp("UV Base", "NodeSocketVector")
    inp("UV Top", "NodeSocketVector")
    inp("Mat Index", "NodeSocketFloat", 0.0)      # 0..1（byte/255）
    inp("Param Cols", "NodeSocketFloat", 114.0)
    inp("Palette Row", "NodeSocketFloat", 0.0)
    inp("Spec Mode", "NodeSocketFloat", 0.0)      # 0 auto / 1 G / 2 B
    inp("Interior Glow", "NodeSocketFloat", 16.0)
    inp("Powered", "NodeSocketFloat", 1.0)
    inp("Relief", "NodeSocketFloat", 0.0)         # 0=关 1=开（cone-step）
    inp("AO Strength", "NodeSocketFloat", 1.0)
    inp("Roughness Floor", "NodeSocketFloat", 0.5)   # 防止高光过强像金属

    outp("Color", "NodeSocketColor")              # lerp(interior, outdoor, opacity)
    outp("Emission", "NodeSocketColor")
    outp("Roughness", "NodeSocketFloat")
    outp("Specular", "NodeSocketFloat")
    outp("Alpha", "NodeSocketFloat")
    outp("Normal", "NodeSocketVector")
    outp("Debug Coverage", "NodeSocketFloat")
    outp("Debug Material", "NodeSocketFloat")

    gin = nt.nodes.new("NodeGroupInput")
    gin.location = (-2600, 0)

    # ---------------- 参数表逐像素选列（vMatUV）
    mat_u = _math(nt, "DIVIDE",
                  _math(nt, "ADD", _math(nt, "FLOOR", _math(nt, "MULTIPLY_ADD",
                                                           gin.outputs["Mat Index"], 255.0, 0.1)), 0.5),
                  gin.outputs["Param Cols"])

    def params_row(off):
        n = _tex(nt, PARAM_IMG, _comb(nt, mat_u, off), f"slot0 row@{off}", "Non-Color",
                 interp="Closest", flip=True, repeat=False)
        return n

    def rgba(node):
        r, g, b = _sep(nt, node.outputs["Color"])[:3]
        return r, g, b, node.outputs["Alpha"]

    pal_origin = params_row(0.125)
    xform = params_row(0.375)
    xform2 = params_row(0.625)
    room = params_row(0.875)

    xs, ys, xo, yo = rgba(xform)
    xs2, ys2, xo2, yo2 = rgba(xform2)
    px_, py_, rz, rw = rgba(room)
    pal_u1, pal_u2, int_scale, int_off = rgba(pal_origin)

    # ---------------- Base 域
    base_uv = _vmath(nt, "ADD",
                     _vmath(nt, "MULTIPLY", _vmath(nt, "FRACTION", gin.outputs["UV Base"]),
                            _comb(nt, xs, ys)),
                     _comb(nt, xo, yo))

    # ---------------- Top 域（padding + outsideTile）
    one_pad = _comb(nt, _math(nt, "ADD", px_, 1.0), _math(nt, "ADD", py_, 1.0))
    relief_src = _vmath(nt, "SUBTRACT",
                        _vmath(nt, "MULTIPLY", _vmath(nt, "FRACTION", gin.outputs["UV Top"]), one_pad),
                        _comb(nt, _math(nt, "MULTIPLY", px_, 0.5), _math(nt, "MULTIPLY", py_, 0.5)))
    rsx, rsy, _z = _sepxyz(nt, relief_src)
    outside = _math(nt, "ADD",
                    _math(nt, "ADD", _math(nt, "MAXIMUM", _math(nt, "MULTIPLY", rsx, -1.0), 0.0),
                          _math(nt, "MAXIMUM", _math(nt, "MULTIPLY", rsy, -1.0), 0.0)),
                    _math(nt, "ADD", _math(nt, "MAXIMUM", _math(nt, "SUBTRACT", rsx, 1.0), 0.0),
                          _math(nt, "MAXIMUM", _math(nt, "SUBTRACT", rsy, 1.0), 0.0)))
    has_top = _math(nt, "MULTIPLY",
                    _math(nt, "GREATER_THAN", _math(nt, "ABSOLUTE", xs2), 1e-6),
                    _math(nt, "GREATER_THAN", _math(nt, "ABSOLUTE", ys2), 1e-6))
    rel_clamped = _vmath(nt, "MINIMUM", _vmath(nt, "MAXIMUM", relief_src, (-0.0, -0.0, 0.0)), (1.0, 1.0, 0.0))
    top_uv = _vmath(nt, "ADD",
                    _vmath(nt, "MULTIPLY", rel_clamped, _comb(nt, xs2, ys2)),
                    _comb(nt, xo2, yo2))

    # ---------------- 采样
    tint_b = _tex(nt, IMAGES["tint"], base_uv, "slot1 tint · base", "Non-Color")
    tint_t = _tex(nt, IMAGES["tint"], top_uv, "slot1 tint · top", "Non-Color")
    nrm_b = _tex(nt, IMAGES["normal"], base_uv, "slot2 normal · base", "Non-Color")
    nrm_t = _tex(nt, IMAGES["normal"], top_uv, "slot2 normal · top", "Non-Color")
    sh_b = _tex(nt, IMAGES["shader"], base_uv, "slot3 shader · base", "Non-Color")
    sh_t = _tex(nt, IMAGES["shader"], top_uv, "slot3 shader · top", "Non-Color")

    facade = _math(nt, "MULTIPLY", _math(nt, "MULTIPLY", has_top,
                                        _math(nt, "SUBTRACT", 1.0, _math(nt, "MINIMUM", outside, 1.0))),
                   tint_t.outputs["Alpha"])

    # palette 子采样坐标：sub = rg*(1/512,1/16)+(1/1024,1/32)
    def sub_uv(tint_col):
        r, g, b = _sep(nt, tint_col)[:3]
        return (_math(nt, "MULTIPLY_ADD", r, 1 / 512.0, 1 / 1024.0),
                _math(nt, "MULTIPLY_ADD", g, 1 / 16.0, 1 / 32.0),
                b)

    sb_u, sb_v, sb_b = sub_uv(tint_b.outputs["Color"])
    st_u, st_v, st_b = sub_uv_raw = sub_uv(tint_t.outputs["Color"])

    def palette(px_u, sub_u, sub_v, row_off):
        u = _math(nt, "ADD", px_u, sub_u)
        v = _math(nt, "ADD", row_off, sub_v)
        n = _tex(nt, IMAGES["palette"], _comb(nt, u, v), f"slot4 palette row {row_off}",
                 "sRGB", interp="Closest", repeat=True)
        return n.outputs["Color"], n.outputs["Alpha"]

    row_tint = gin.outputs["Palette Row"]
    row_surf = _math(nt, "ADD", row_tint, 0.875)

    pal_b, pal_ba = palette(pal_u1, sb_u, sb_v, row_tint)
    pal_t, pal_ta = palette(pal_u2, st_u, st_v, row_tint)
    pal_color = _mix(nt, facade, pal_b, pal_t)
    tint_mul = _math(nt, "MULTIPLY", _mixf(nt, facade, sb_b, st_b), 2.0)

    surf_b, _ = palette(pal_u1, sb_u, sb_v, row_surf)
    surf_t, _ = palette(pal_u2, st_u, st_v, row_surf)
    surf_color = _mix(nt, facade, surf_b, surf_t)

    # ---------------- shader map / normal / AO
    shader = _mix(nt, facade, sh_b.outputs["Color"], sh_t.outputs["Color"])
    normal = _mix(nt, facade, nrm_b.outputs["Color"], nrm_t.outputs["Color"])
    ao = _math(nt, "MULTIPLY", normal.alpha if hasattr(normal, "alpha") else 1.0, 1.0)

    nrm_ao = _mixf(nt, facade, nrm_b.outputs["Alpha"], nrm_t.outputs["Alpha"])
    ao = _math(nt, "MULTIPLY", nrm_ao, gin.outputs["AO Strength"], clamp=True)

    sm_r, sm_g, sm_b = _sep(nt, shader)[:3]
    sm_a = _mixf(nt, facade, sh_b.outputs["Alpha"], sh_t.outputs["Alpha"])

    # spec 通道：auto = a<0.5 ? B : G
    auto_spec = _mixf(nt, _math(nt, "GREATER_THAN", sm_a, 0.5), sm_b, sm_g)
    spec_raw = _mixf(nt, _math(nt, "GREATER_THAN", gin.outputs["Spec Mode"], 0.5),
                     auto_spec,
                     _mixf(nt, _math(nt, "GREATER_THAN", gin.outputs["Spec Mode"], 1.5), sm_g, sm_b))
    spec_strength = _math(nt, "MULTIPLY", spec_raw, 2.0)

    pal_a = _mixf(nt, facade, pal_ba, pal_ta)
    spec_a = _math(nt, "MULTIPLY", pal_a, tint_mul)
    spec_e = _math(nt, "MULTIPLY_ADD", _math(nt, "POWER", spec_a, 3.0), 2048.0, 1.0)
    gloss = _math(nt, "MULTIPLY", spec_a, spec_strength, clamp=True)

    # ---------------- 合成
    pal_rgb = _mix(nt, facade, pal_b, pal_t)
    albedo = _mul(nt, pal_rgb, _math(nt, "MULTIPLY", tint_mul, ao))
    opacity = sm_a

    # 内景（盒体投影简化：eye=+N；完整版由 attr 切线架提供视线）
    iu = _vmath(nt, "MULTIPLY", gin.outputs["UV Base"], _comb(nt, xs, ys))
    iu = _vmath(nt, "MULTIPLY", iu, _comb(nt, rz, rw))
    iu_f = _vmath(nt, "FRACTION", iu)
    ix, iy, _ = _sepxyz(nt, iu_f)
    # box projection（eye 沿法线）
    posx = _math(nt, "MULTIPLY_ADD", ix, -2.0, 1.0)
    posy = _math(nt, "MULTIPLY_ADD", iy, -2.0, 1.0)
    # eye=(0,0,0.5) → k.z=4, k.x/k.y=inf → t=4 → target=pos+4*eye → z=1, xy=pos
    tx = _math(nt, "MULTIPLY", posx, 0.5)   # target.xy * lerp(0.9,0.5,1)=0.5
    ty = _math(nt, "MULTIPLY", posy, 0.5)
    resx = _math(nt, "MULTIPLY_ADD", tx, -0.5, 0.5)
    resy = _math(nt, "MULTIPLY_ADD", ty, -0.5, 0.5)
    int_tc = _comb(nt,
                   _math(nt, "MULTIPLY_ADD", resx, int_scale, 0.0),
                   _math(nt, "MULTIPLY_ADD", resy, int_scale, int_off))
    int_tex = _tex(nt, IMAGES["interior"], int_tc, "slot5 interior", "sRGB", repeat=True)
    int_col = int_tex.outputs["Color"]
    int_a = int_tex.outputs["Alpha"]
    self_light = _math(nt, "MULTIPLY", _math(nt, "MULTIPLY", int_a, gin.outputs["Interior Glow"]),
                       gin.outputs["Powered"])

    emiss_int = _mul(nt, int_col, _math(nt, "MULTIPLY", self_light,
                                        _math(nt, "SUBTRACT", 1.0, opacity)))
    pal_emiss = _mul(nt, albedo, _math(nt, "MULTIPLY",
                                       _math(nt, "MULTIPLY", _sep(nt, surf_color)[0], tint_mul),
                                       gin.outputs["Powered"]))
    emiss_ext = _mul(nt, pal_emiss, opacity)
    emiss = _mix(nt, 1.0, emiss_int, emiss_ext, blend="ADD")

    color = _mix(nt, opacity, int_col, albedo)

    rough = _math(nt, "SUBTRACT", 1.0, gloss, clamp=True)
    rough = _math(nt, "MAXIMUM", rough, gin.outputs["Roughness Floor"])
    alpha = _math(nt, "GREATER_THAN", tint_b.outputs["Alpha"], 0.5)

    out = nt.nodes.new("NodeGroupOutput")
    out.location = (1600, 0)
    for nm, src in (("Color", color), ("Emission", emiss), ("Roughness", rough),
                    ("Specular", gloss), ("Alpha", alpha), ("Normal", normal),
                    ("Debug Coverage", facade), ("Debug Material", mat_u)):
        nt.links.new(src, out.inputs[nm])
    return nt


# ------------------------------------------------------------------ 材质
def load_images(base_dir, prefix="0x4dc31f9b-lod1-mat01"):
    global IMAGES
    mapping = {
        "tint": "slot1.png",
        "normal": "slot2.png",
        "shader": "slot3.png",
        "palette": "slot4.png",
        "interior": "slot5.png",
    }
    IMAGES = {}
    for k, fn in mapping.items():
        cand = [base_dir + "\\" + fn, base_dir + "\\" + f"{prefix}-{fn}"]
        path = next((p for p in cand if os.path.exists(p)), None)
        if path is None:
            raise RuntimeError(f"missing texture {fn} in {base_dir}")
        img = bpy.data.images.load(path, check_existing=True)
        img.alpha_mode = "CHANNEL_PACKED"
        IMAGES[k] = img
    return IMAGES


def make_material(name, ng, mode="DATA"):
    """mode=DATA：无光照复现（可直接与 openscp 的材质数据对拍）；
       mode=LIT：albedo/roughness/emission 交给 EEVEE 光照。"""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()

    out = nt.nodes.new("ShaderNodeOutputMaterial")
    out.location = (900, 0)
    grp = nt.nodes.new("ShaderNodeGroup")
    grp.node_tree = ng
    grp.location = (300, 0)

    uv_b = nt.nodes.new("ShaderNodeUVMap")
    uv_b.uv_map = "UVBase"
    uv_b.location = (0, 300)
    uv_t = nt.nodes.new("ShaderNodeUVMap")
    uv_t.uv_map = "UVTop"
    uv_t.location = (0, 180)
    attr = nt.nodes.new("ShaderNodeAttribute")
    attr.attribute_name = "matIndex"
    attr.attribute_type = "GEOMETRY"
    attr.location = (0, 60)

    nt.links.new(uv_b.outputs["UV"], grp.inputs["UV Base"])
    nt.links.new(uv_t.outputs["UV"], grp.inputs["UV Top"])
    nt.links.new(attr.outputs["Fac"], grp.inputs["Mat Index"])

    if mode == "DATA":
        em = nt.nodes.new("ShaderNodeEmission")
        em.location = (650, 0)
        nt.links.new(grp.outputs["Color"], em.inputs["Color"])
        nt.links.new(em.outputs[0], out.inputs["Surface"])
    else:
        bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
        bsdf.location = (650, 0)
        nt.links.new(grp.outputs["Color"], bsdf.inputs["Base Color"])
        nt.links.new(grp.outputs["Emission"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = 1.0
        nt.links.new(grp.outputs["Roughness"], bsdf.inputs["Roughness"])
        nt.links.new(grp.outputs["Alpha"], bsdf.inputs["Alpha"])
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nm.location = (400, -250)
        nm.inputs["Strength"].default_value = 1.5
        nt.links.new(grp.outputs["Normal"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
        nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    mat.surface_render_method = "DITHERED"
    mat.use_backface_culling = False
    mat.use_fake_user = True
    mat["b4_mode"] = mode
    return mat


def set_uniforms(mat, **kw):
    grp = [n for n in mat.node_tree.nodes if n.type == "GROUP"][0]
    names = {s.name: s for s in grp.inputs}
    for k, v in kw.items():
        nm = k.replace("_", " ")
        if nm in names:
            names[nm].default_value = v
