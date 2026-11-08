import bpy,sys,math
from pathlib import Path
from mathutils import Vector
import numpy as np
sys.path.insert(0,str(Path.cwd()/'纽约立面插件/开发_1_24'));import ny_facade as a
a.register();a.initialize_ui();bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False);bpy.ops.mesh.primitive_cube_add();owner=bpy.context.object;settings=bpy.context.scene.ny_prop_settings;settings.owner=owner
for preset in a.props.CATALOG['presets']:
 settings.preset=preset;settings.yaw=.8;o=a.props.create(bpy.context,Vector((6,8,1)));bpy.context.view_layer.update();before=a.props.collect(bpy.context,[owner],Vector(),1)['instances'][-1]
 for ob in bpy.context.selected_objects:ob.select_set(False)
 o.select_set(True);bpy.context.view_layer.objects.active=o;bpy.ops.object.transform_apply(location=True,rotation=True,scale=True);bpy.context.view_layer.update();after=a.props.collect(bpy.context,[owner],Vector(),1)['instances'][-1];np.testing.assert_allclose(before['matrix'],after['matrix'],atol=2e-5);assert abs(before['scale']-after['scale'])<1e-6;bpy.data.objects.remove(o,do_unlink=True)
 print('APPLY_TRANSFORMS_PASS',preset)
