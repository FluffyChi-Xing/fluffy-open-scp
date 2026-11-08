import bpy,sys,math,json
from pathlib import Path
from mathutils import Vector
r=Path.cwd();sys.path.insert(0,str(r/'纽约立面插件/开发_1_24'));import ny_facade as a
a.register();a.initialize_ui();out=r/'SC5 Asset Packager/Projects/PropsQA060'
bpy.ops.wm.open_mainfile(filepath=str(out/'placement.blend'))
owner=next(o for o in bpy.context.scene.objects if o.name=='Prop Test House');objects=[o for o in bpy.context.scene.objects if a.props.is_prop(o)];assert len(objects)==6
before=a.props.collect(bpy.context,[owner],Vector((10,20,0)),1);assert len(before['instances'])==6
# Detect proxy edits rather than silently exporting unchanged native geometry.
m=objects[0].data;old=m.vertices[0].co.copy();m.vertices[0].co.x+=1
try:a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)
except ValueError:pass
else:raise AssertionError('edited proxy accepted')
m.vertices[0].co=old
# Shared mesh and matrix handling under centimetre scene units and transformed owner.
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
bpy.context.scene.unit_settings.system='METRIC';bpy.context.scene.unit_settings.scale_length=.01
bpy.ops.mesh.primitive_cube_add(location=(1200,-500,0));o=bpy.context.object;o.scale=(2,3,4);o.rotation_euler.z=.7;bpy.context.view_layer.update();s=bpy.context.scene.ny_prop_settings;s.owner=o;s.preset='tree_a';s.scale=.25
p=a.props.create(bpy.context,Vector((1300,-200,0)));bpy.context.view_layer.update();assert (p.matrix_world.translation-Vector((1300,-200,0))).length<.01
origin=Vector((1200,-500,0));d=a.props.collect(bpy.context,[o],origin,.01);i=d['instances'][0];assert abs(i['scale']-.25)<1e-5;assert (Vector(i['matrix'][9:])-Vector((1,3,0))).length<1e-5
# Scene-unit changes preserve visible world size in exported metre coordinates.
bpy.context.scene.unit_settings.scale_length=.02;d=a.props.collect(bpy.context,[o],origin,.02);assert abs(d['instances'][0]['scale']-.5)<1e-5
(out/'blender-transform-qa.json').write_text(json.dumps({'savedSceneReopened':True,'packedImages':sum(im.packed_file is not None for im in bpy.data.images),'proxyEditRejected':True,'centimetreSceneUnits':True,'transformedParentKeepsWorldPlacement':True,'unitChangesPreservePhysicalScale':True},indent=2),encoding='utf-8')
print('BLENDER_TRANSFORM_QA_PASS')
