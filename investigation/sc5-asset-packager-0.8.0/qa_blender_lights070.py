import bpy,sys,json,math,importlib
from pathlib import Path
from mathutils import Vector
import numpy as np
r=Path.cwd();sys.path.insert(0,str(r/'纽约立面插件/开发_1_24'));import ny_facade as a
a.register();a.initialize_ui();a.unregister();a.register();a=importlib.reload(a);a.register();a.initialize_ui()
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
bpy.ops.mesh.primitive_cube_add(size=2,location=(0,0,7.5));owner=bpy.context.object;owner.name='SC5 Light Test';owner.scale=(5,4,7.5);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
settings=bpy.context.scene.ny_lot_settings;settings.width=32;settings.depth=28;settings.resolution='256';bpy.ops.ny_facade.lot_create(fit=False)
scene=bpy.context.scene;scene.ny_prop_settings.owner=owner;scene.ny_prop_settings.preset='tree_a';a.props.create(bpy.context,Vector((10,8,0)))
p=scene.ny_light_settings;p.owner=owner
for preset,position in [('WALL_UP',(-3,4.1,1)),('WALL_UP',(3,4.1,1)),('GROUND_DOWN',(0,4.1,12)),('POINT',(10,0,3))]:
 p.preset=preset;o=a.lights.create(bpy.context,Vector(position));bpy.context.view_layer.update()
 doc=a.lights.collect(bpy.context,[owner],Vector(),1);item=next(i for i in doc['instances'] if i['id']==o['ny_light_uid']);direction=Vector(item['matrix'][3:6]);expected=o.matrix_world.to_3x3()@Vector((0,0,-1));assert direction.dot(expected)>.9999
 assert o.data.energy>0
for o in bpy.context.selected_objects:o.select_set(False)
owner.select_set(True);bpy.context.view_layer.objects.active=owner
out=r/'SC5 Asset Packager/Projects/LightsQA070';out.mkdir(parents=True,exist_ok=True)
report=a.native_asset.native_export(bpy.context,out/'LightsHouse.glb');assert report['propManifest']['count']==1 and report['lightManifest']['count']==4;assert report['triangles']==24
light=next(o for o in bpy.context.scene.objects if a.lights.is_light(o));light.scale=(2,1,1);bpy.context.view_layer.update()
try:a.lights.collect(bpy.context,[owner],Vector(),1)
except ValueError:pass
else:raise AssertionError('nonuniform light scale accepted')
light.scale=(1,1,1);bpy.context.view_layer.update()
# Export from a selected lamp also includes its owning building, ground and props.
owner.select_set(False);light.select_set(True);bpy.context.view_layer.objects.active=light
other=out/'selected-lamp';other.mkdir(exist_ok=True);r2=a.native_asset.native_export(bpy.context,other/'LightsHouse.glb');assert r2['lightManifest']['count']==4 and r2['propManifest']['count']==1
bpy.ops.wm.save_as_mainfile(filepath=str(out/'lights-placement.blend'))
(out/'blender-qa.json').write_text(json.dumps({'version':a.bl_info['version'],'reloadRegistration':True,'lights':4,'props':1,'bodyTriangles':24,'nativeDirectionMatchesBlenderLamp':True,'nonUniformRejected':True,'selectedLampExportsOwner':True,'report':report},ensure_ascii=False,indent=2),encoding='utf-8')
print('LIGHTS_BLENDER_PASS',report['lightManifest'])
