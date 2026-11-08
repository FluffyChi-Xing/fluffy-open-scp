"""Build the shared native-prop preview library from read-only installed resources."""
from pathlib import Path
import sys,struct,json,gzip,hashlib,io,shutil
import numpy as np
from PIL import Image
ROOT=Path(__file__).resolve().parent.parent
sys.path.insert(0,str(ROOT/'SC5 Asset Packager/engine'))
from package_io import index
from rw4_bridge import RW4
GAME=Path('D:/E/gameecg/SimCity：Cites of Tomorrow/SimCityData')
OUT=ROOT/'SC5 Asset Packager/prop_assets';OUT.mkdir(exist_ok=True)
inv={e.key:e for name in ('SimCity_Game.package','SimCity_Graphics.package') for e in index(GAME/name)}
rows=[('tree_a','原版乔木 A',0x14984c68,0x89d658df,12,.25,True),('tree_b','原版乔木 B',0x14984c69,0x1113b131,11,.25,True),('tree_c','原版乔木 C',0x14984c6a,0xc2fbd178,10,.25,True),('tree_conifer','原版针叶树',0x14984c6b,0x4df43690,9,.25,True),('street_furniture','原版街道家具组合',0x9375c65e,0xad64cb3d,0,1.,False)]
cat={'schema':'sc5.native-prop-catalog/1','version':'2026.11.01.1','presets':{}}
for key,label,rid,model,bin_,scale,tree in rows:
 e=inv[(0x2f4e681b,0,model)];raw=e.read();r=RW4(raw);mesh=r.of_type(0x20009)[0];m=struct.unpack('<10I',mesh.payload);v=struct.unpack('<7I',r.sections[m[9]].payload);t=struct.unpack('<7I',r.sections[m[2]].payload)
 vb=r.sections[v[6]].payload;ib=r.sections[t[6]].payload;fmt=r.sections[v[0]].payload;count=struct.unpack_from('<H',fmt,12)[0];decl={}
 for j in range(count):
  q=24+j*12;off,typ,usage=struct.unpack_from('>HHH',fmt,q+1);decl[(usage,fmt[q+7])]=(off,typ)
 def attribute(usage,n):
  off,typ=decl[(usage,0)];assert typ in (1,2)
  return np.ndarray((v[3],n),dtype='<f4',buffer=vb,offset=off,strides=(v[5],4)).copy()
 pos=attribute(0,3);normal=attribute(3,3);uv=attribute(5,2);ix=np.frombuffer(ib,dtype='<u2' if t[4]==101 else '<u4').copy();assert len(ix)==m[6] and ix.max()<len(pos)
 tex=r.of_type(0x20003)[0];f,_,_,w,h,_,_,_,blob=struct.unpack_from('<IIIHHIIII',tex.payload);pixels=r.sections[blob].payload
 if f in (0x31545844,0x35545844):
  top=((w+3)//4)*((h+3)//4)*(8 if f==0x31545844 else 16);header=bytearray(128);header[:4]=b'DDS ';struct.pack_into('<7I',header,4,124,0x81007,h,w,top,0,1);struct.pack_into('<III',header,76,32,4,f);struct.pack_into('<I',header,108,0x1000)
  image=Image.open(io.BytesIO(bytes(header)+pixels[:top])).convert('RGBA')
 else:assert f==21;image=Image.frombytes('RGBA',(w,h),pixels[:w*h*4],'raw','BGRA')
 image.save(OUT/(key+'.png'))
 doc={'schema':'sc5.prop-preview-mesh/1','positions':pos.tolist(),'normals':normal.tolist(),'uv':uv.tolist(),'indices':ix.tolist(),'image':key+'.png','alphaCutoff':.5 if tree else None,'nativeModel':f'2F4E681B:00000000:{model:08X}'}
 (OUT/(key+'.mesh.json.gz')).write_bytes(gzip.compress(json.dumps(doc,separators=(',',':')).encode(),mtime=0))
 cat['presets'][key]={'label':label,'resourceId':f'{rid:08X}','nativeModel':doc['nativeModel'],'preferredBin':bin_,'defaultScale':scale,'tree':tree,'previewMesh':key+'.mesh.json.gz','image':key+'.png','modelSha256':hashlib.sha256(raw).hexdigest(),'bounds':[pos.min(axis=0).tolist(),pos.max(axis=0).tolist()],'wrapper':f'ADA272{0x4f-(rid-0x14984c68):02X}' if tree else None}
 print(key,'verts',len(pos),'tris',len(ix)//3,'alpha',image.getextrema()[3])
(OUT/'catalog.json').write_text(json.dumps(cat,ensure_ascii=False,indent=2),encoding='utf-8')
plugin=ROOT/'纽约立面插件/开发_1_24/ny_facade/assets/props';shutil.copytree(OUT,plugin,dirs_exist_ok=True)
