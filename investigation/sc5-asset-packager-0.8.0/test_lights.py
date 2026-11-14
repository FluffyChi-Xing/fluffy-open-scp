import copy,hashlib,json,math,struct,sys,tempfile,unittest
from pathlib import Path
import numpy as np
import light_contract as lights
sys.path.insert(0,str(Path(__file__).parent/'engine'))
from native_property_codec import validate_native_profile
from inspect_park_route import parse
class LightTests(unittest.TestCase):
 def doc(self):return {'schema':lights.SCHEMA,'unit':'meter','upAxis':'+Z','space':'building-local','instances':[{'id':'test','type':'SPOT','matrix':[1,0,0,0,0,1,0,-1,0,2,3,4],'color':[1,.5,.2],'radius':6.,'innerRadius':1.,'length':12.,'intensity':4.,'specular':.5,'control':'NATIVE_NIGHT'}]}
 def test_native_columns_group_and_transform(self):
  doc=self.doc();fields,e=lights.compile_fields(doc,90);raw=struct.pack('>I',len(fields))+b''.join(fields[h] for h in sorted(fields));validate_native_profile(raw);f=parse(raw)
  self.assertEqual(len(fields),18);self.assertEqual(f['0CAA8F12']['value'][0]['rawBytes'],50);self.assertEqual(f['0CAA8F10']['value'],f['0CE71B10']['value']);self.assertEqual(f['0CE71B12']['value'][0][0],.875);self.assertFalse(e['powerControlVerified'])
  np.testing.assert_allclose(e['instances'][0]['matrix'][9:],[-3,2,4])
 def test_wrong_parameters_rejected(self):
  for field,value in [('radius',0),('innerRadius',7),('intensity',float('nan')),('control','guessed-power'),('type','AREA')]:
   d=self.doc();d['instances'][0][field]=value
   with self.subTest(field=field),self.assertRaises(ValueError):lights.validate(d)
 def test_missing_mismatched_and_legacy_manifest(self):
  with tempfile.TemporaryDirectory() as raw:
   m=Path(raw)/'a.lotm';p=m.with_suffix('.lights.json');data=json.dumps(self.doc()).encode();decl={'lightManifest':{'file':p.name,'sha256':hashlib.sha256(data).hexdigest(),'count':1}}
   self.assertEqual(lights.read_manifest(m)[0]['instances'],[])
   with self.assertRaises(ValueError):lights.read_manifest(m,decl)
   p.write_bytes(data);self.assertEqual(len(lights.read_manifest(m,decl)[0]['instances']),1)
   p.write_bytes(data+b' ')
   with self.assertRaises(ValueError):lights.read_manifest(m,decl)
 def test_mirror_and_duplicate_rejected(self):
  d=self.doc();d['instances'][0]['matrix'][0]=-1
  with self.assertRaises(ValueError):lights.validate(d)
  d=self.doc();d['instances'].append(copy.deepcopy(d['instances'][0]))
  with self.assertRaises(ValueError):lights.validate(d)
if __name__=='__main__':unittest.main()
