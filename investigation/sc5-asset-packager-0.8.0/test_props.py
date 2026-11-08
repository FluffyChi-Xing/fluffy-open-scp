import copy,hashlib,json,math,struct,sys,tempfile,unittest
from pathlib import Path
import numpy as np
from prop_contract import validate,read_manifest,rotate_matrix,compile_fields,preview_instances
sys.path.insert(0,str(Path(__file__).parent/'engine'))
from inspect_park_route import parse
class PropsTests(unittest.TestCase):
 def doc(self):return {'schema':'sc5.prop-instances/1','unit':'meter','upAxis':'+Z','space':'building-local','instances':[{'id':'one','preset':'tree_a','matrix':[1,0,0,0,1,0,0,0,1,3,5,2],'scale':.25}]}
 def test_orientation_rotates_basis_and_translation_once(self):
  d=self.doc()['instances'][0];S=np.array(d['matrix'][:9]).reshape(3,3);points=np.array([[0,0,0],[1,2,3]])
  for angle in (0,90,180,270):
   theta=math.radians(angle);R=np.array([[round(math.cos(theta)),-round(math.sin(theta)),0],[round(math.sin(theta)),round(math.cos(theta)),0],[0,0,1]])
   m=rotate_matrix(d['matrix'],angle)
   np.testing.assert_allclose((points@np.array(m[:9]).reshape(3,3))*.25+m[9:],((points@S)*.25+d['matrix'][9:])@R.T)
 def test_invalid_transforms_and_identity_are_rejected(self):
  for change in ('mirror','scale','nan','unknown','duplicate','nonuniform'):
   d=self.doc();i=d['instances'][0]
   if change=='mirror':i['matrix'][0]=-1
   elif change=='scale':i['scale']=0
   elif change=='nan':i['matrix'][9]=float('nan')
   elif change=='unknown':i['preset']='missing'
   elif change=='duplicate':d['instances'].append(copy.deepcopy(i))
   else:i['matrix'][0]=2
   with self.subTest(change=change),self.assertRaises(ValueError):validate(d)
 def test_sidecar_matches_export_and_missing_is_not_ignored(self):
  with tempfile.TemporaryDirectory() as folder:
   model=Path(folder)/'house.lotm';side=model.with_suffix('.props.json');data=json.dumps(self.doc()).encode();decl={'propManifest':{'file':side.name,'count':1,'sha256':hashlib.sha256(data).hexdigest()}}
   with self.assertRaises(ValueError):read_manifest(model,decl)
   side.write_bytes(data);self.assertEqual(len(read_manifest(model,decl)[0]['instances']),1)
   with self.assertRaises(ValueError):read_manifest(model,{})
   side.write_bytes(data+b' ')
   with self.assertRaises(ValueError):read_manifest(model,decl)
 def test_shared_preview_geometry_and_leaf_alpha(self):
  d=self.doc();d['instances'].append({**d['instances'][0],'id':'two'});meshes,objects,bounds=preview_instances(d)
  self.assertEqual(len(meshes),1);self.assertEqual(len(objects),2);self.assertEqual(meshes['tree_a']['image'].shape[2],4);self.assertTrue((meshes['tree_a']['image'][:,:,3]==0).any());self.assertTrue(np.all(bounds[1]>bounds[0]))
 def test_empty_old_export_is_compatible(self):
  with tempfile.TemporaryDirectory() as folder:self.assertEqual(read_manifest(Path(folder)/'old.lotm')[0]['instances'],[])
if __name__=='__main__':unittest.main()
