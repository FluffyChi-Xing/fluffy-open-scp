"""OpenSCP native adapter: run the bundled SC5 asset exporter headlessly.

Arguments after ``--``:
  addon_parent  directory containing the ``ny_facade`` package
  source_blend  source .blend
  output_glb    destination .glb (the add-on also writes a sibling .lotm)
"""

import bpy
import os
import sys

addon_parent, source, output = sys.argv[-3:]
sys.path.insert(0, addon_parent)

import ny_facade  # noqa: E402

ny_facade.register()
from ny_facade import native_asset  # noqa: E402

bpy.ops.wm.open_mainfile(filepath=source)


def apply_modifiers():
    """Apply modifiers on a private copy before the native export.

    The Asset Editor refuses objects with unapplied modifiers
    ("原生导出暂不支持未应用修改器").  Authoring files routinely carry a
    "Smooth by Angle" geometry-nodes modifier, so apply every modifier on a
    single-user data-block.  The source .blend is never saved, so this only
    changes the in-memory export scene.
    """
    for obj in list(bpy.context.view_layer.objects):
        if obj.type != 'MESH' or not obj.modifiers:
            continue
        bpy.ops.object.select_all(action='DESELECT')
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        if obj.data.users > 1:
            obj.data = obj.data.copy()
        for modifier in list(obj.modifiers):
            try:
                bpy.ops.object.modifier_apply(modifier=modifier.name)
            except Exception as error:  # keep the original message actionable
                raise RuntimeError(
                    f'Cannot apply modifier {modifier.name!r} on {obj.name!r}: {error}'
                ) from error


apply_modifiers()
# bpy.data.objects includes orphan data-blocks linked into no collection;
# select_set on them raises because they are not in the active view layer.
meshes = [o for o in bpy.context.view_layer.objects if o.type == 'MESH']
if not meshes:
    raise RuntimeError('Blend file contains no building mesh')
bpy.ops.object.select_all(action='DESELECT')
for obj in meshes:
    obj.hide_set(False)
    obj.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]

native_asset._native_export_impl(bpy.context, output, center=True)
if not os.path.isfile(output):
    raise RuntimeError('Native exporter did not produce a GLB')
lotm = os.path.splitext(output)[0] + '.lotm'
if not os.path.isfile(lotm):
    raise RuntimeError('Native exporter did not produce a LOTM')
print('OPENSCP_NATIVE_OK', output, lotm)
