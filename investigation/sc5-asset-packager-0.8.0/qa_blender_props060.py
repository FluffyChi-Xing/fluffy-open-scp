import bpy,sys,json,math,importlib
from pathlib import Path
r=Path.cwd();sys.path.insert(0,str(r/'纽约立面插件/开发_1_24'));import ny_facade as a
from mathutils import Vector
a.register();a.initialize_ui();a.unregister();a.register();a.initialize_ui()
a=importlib.reload(a);a.register();a.initialize_ui()
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
bpy.ops.mesh.primitive_cube_add(size=2,location=(10,20,7.5));owner=bpy.context.object;owner.name='Prop Test House';owner.scale=(5,4,7.5);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
p=bpy.context.scene.ny_lot_settings;p.width=36;p.depth=36;p.resolution='256'
bpy.ops.ny_facade.lot_create(fit=False);ground=bpy.context.object
ps=bpy.context.scene.ny_prop_settings;ps.owner=owner
points=[(-3,8,0),(23,8,0),(23,32,0),(-3,32,0),(6,32,0)]
for key,pt in zip(a.props.CATALOG['presets'],points):
 ps.preset=key;ps.yaw=math.radians(30);o=a.props.create(bpy.context,Vector(pt));bpy.context.view_layer.update();assert (o.matrix_world.translation-Vector(pt)).length<1e-4
# Shared meshes, independent instance IDs.
ps.preset='tree_a';extra=a.props.create(bpy.context,Vector((14,32,0)));assert extra.data==next(o.data for o in bpy.context.scene.objects if o.get('ny_prop_preset')=='tree_a')
for ob in bpy.context.selected_objects:ob.select_set(False)
owner.select_set(True);bpy.context.view_layer.objects.active=owner
out=r/'SC5 Asset Packager/Projects/PropsQA060';out.mkdir(parents=True,exist_ok=True)
report=a.native_asset.native_export(bpy.context,out/'PropsHouse.glb');doc=json.loads((out/'PropsHouse.props.json').read_text(encoding='utf-8'));assert len(doc['instances'])==6
assert report['triangles']==24,report['triangles']
for item,pt in zip(doc['instances'],[]):pass
extra.scale=(1,2,1);bpy.context.view_layer.update()
try:a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)
except ValueError:pass
else:raise AssertionError('nonuniform transform accepted')
extra.scale=(.25,)*3;bpy.context.view_layer.update()
bpy.ops.wm.save_as_mainfile(filepath=str(out/'placement.blend'))
(out/'blender-qa.json').write_text(json.dumps({'registerReload':True,'propCount':6,'bodyTriangles':report['triangles'],'nonUniformRejected':True,'report':report},ensure_ascii=False,indent=2),encoding='utf-8')
print('PROPS_BLENDER_QA_PASS',report)

