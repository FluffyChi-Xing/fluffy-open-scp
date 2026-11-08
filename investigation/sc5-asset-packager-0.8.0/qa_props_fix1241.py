import bpy,sys,json,math,tempfile
from pathlib import Path
from mathutils import Vector
import numpy as np
from unittest.mock import patch
r=Path.cwd();sys.path.insert(0,str(r/'纽约立面插件/开发_1_24'));import ny_facade as a
a.register();a.initialize_ui();bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
bpy.ops.mesh.primitive_cube_add(size=2,location=(10,20,7.5));owner=bpy.context.object;owner.scale=(5,4,7.5);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
p=bpy.context.scene.ny_lot_settings;p.width=36;p.depth=36;p.resolution='256';bpy.ops.ny_facade.lot_create(fit=False)
s=bpy.context.scene.ny_prop_settings;s.owner=owner;s.preset='tree_c';s.scale=.25;s.yaw=.55;tree=a.props.create(bpy.context,Vector((20,30,0)))
for ob in bpy.context.selected_objects:ob.select_set(False)
tree.select_set(True);bpy.context.view_layer.objects.active=tree;bpy.context.view_layer.update()
before=a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)['instances'][0]
old_sig=a.props.mesh_signature(tree.data);bpy.ops.object.transform_apply(location=True,rotation=True,scale=True);bpy.context.view_layer.update();assert old_sig!=a.props.mesh_signature(tree.data),'test must reproduce 1.24.0 hash rejection'
after=a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)['instances'][0];np.testing.assert_allclose(before['matrix'],after['matrix'],atol=3e-5);assert abs(before['scale']-after['scale'])<1e-6
# New placement after an applied transform must use fresh canonical geometry.
new=a.props.create(bpy.context,Vector((3,4,0)),owner=owner);bpy.context.view_layer.update();assert new.data!=tree.data
items=a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)['instances'];item=next(i for i in items if i['id']==new['ny_prop_uid']);np.testing.assert_allclose(item['matrix'][9:],[-7,-16,0],atol=1e-5);assert abs(item['scale']-.25)<1e-6;bpy.data.objects.remove(new,do_unlink=True)
out=r/'SC5 Asset Packager/Projects/PropsFix1241';out.mkdir(parents=True,exist_ok=True)
for ob in bpy.context.selected_objects:ob.select_set(False)
owner.select_set(True);bpy.context.view_layer.objects.active=owner
report=a.native_asset.native_export(bpy.context,out/'Fixed.glb');assert report['propManifest']['count']==1;assert all((out/('Fixed'+suffix)).is_file() for suffix in ('.glb','.lotm','.lot.package','.props.json','.native.json','.lotpaint.json','.lotmask.png'))
# Edited geometry fails before any companion file is written, preserving old files.
tree.data.vertices[0].co.x+=1
failure=out/'rejected';failure.mkdir(exist_ok=True)
try:a.native_asset.native_export(bpy.context,failure/'Bad.glb')
except ValueError as exc:assert '恢复' in str(exc)
else:raise AssertionError('real deformation should be rejected')
assert list(failure.iterdir())==[]
# Repair affects only explicitly selected prop and restores a valid native proxy.
owner.select_set(False);tree.select_set(True);bpy.context.view_layer.objects.active=tree;world=tree.matrix_world.copy();bpy.ops.ny_facade.prop_restore();assert tree.matrix_world==world;a.props.collect(bpy.context,[owner],Vector((10,20,0)),1)
# Simulate mid-publication failure, assert old complete export restored exactly.
target=out/'rollback';target.mkdir(exist_ok=True);f=target/'a';g=target/'b';f.write_bytes(b'old-a');g.write_bytes(b'old-b');replace=a.native_asset.os.replace
failed=[False]
def failing(src,dst):
 if Path(dst)==g and Path(src).name.startswith('new-') and not failed[0]:failed[0]=True;raise OSError('injected publish failure')
 return replace(src,dst)
try:
 with patch.object(a.native_asset.os,'replace',side_effect=failing):a.native_asset.commit_exports(target,{f:b'new-a',g:b'new-b'})
except OSError:pass
else:raise AssertionError('expected failure')
assert f.read_bytes()==b'old-a' and g.read_bytes()==b'old-b';assert len(list(target.iterdir()))==2
result={'version':a.bl_info['version'],'oldHashFalsePositiveReproduced':True,'applyAllTransformsPreservesNativePlacement':True,'newPlacementUsesCanonicalMesh':True,'allSevenCompanionsExported':True,'trueEditRejectedBeforeAnyOutput':True,'restoreSelectedPropWorks':True,'publicationFailureRollsBackOldFiles':True};(out/'验证结果.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8');print('FIX1241_PASS',result)
