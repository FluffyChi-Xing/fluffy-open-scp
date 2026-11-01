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
meshes = [o for o in bpy.data.objects if o.type == 'MESH']
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
