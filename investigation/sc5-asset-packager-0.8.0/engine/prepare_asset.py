"""0.8.6: original unit-render single-mesh path, numeric data-map mipmaps."""
from compile_tower_asset import ROOT,image,accessor
from probe_tower import read
from compile_geometry_contract import compile_contract
from rw4_bridge import require
from PIL import Image
from pathlib import Path
import numpy as np,json,hashlib,importlib.util
import native_controls as ctl
from asset_transforms import rotate_xyz, room_strip
OUT=ROOT/'希望大厦研究0.8.6';SOURCE=ROOT/'希望大厦研究0.8.4/输入副本/大连希望大厦2.lotm'
def prepare(rotation_degrees=180, night_only=True):
 OUT.mkdir(exist_ok=True);chunks,mats,_,diag=read(SOURCE);count=len(mats);require(count<=128,'Palette profile supports at most 128 source groups');width=2**int(np.ceil(np.log2(count)));params=np.zeros((4,width,4),'<f4');pal=np.zeros((16,512,4),np.uint8);objects=[]
 surfacekey=lambda m:tuple(hashlib.sha256(m[0][s]).hexdigest() for s in [4,1,6])
 roomkey=lambda m:(hashlib.sha256(m[0][7]).hexdigest(),float(m[1][0,0,2]),float(m[1][0,0,3]))
 surfacekeys=list(dict.fromkeys(surfacekey(m) for m in mats));roomkeys=list(dict.fromkeys(roomkey(m) for m in mats));sg=2**int(np.ceil(np.log2(np.ceil(np.sqrt(len(surfacekeys)+1)))));roomsize=max(1024,64*2**int(np.ceil(np.log2(len(roomkeys)))));maps=[np.zeros((sg*512,sg*512,4),np.uint8) for _ in range(3)]+[np.zeros((roomsize,roomsize,4),np.uint8)];stats=[]
 for i,((g,b),m) in enumerate(zip(chunks,mats)):
  src,p,cols=m;require(cols==1,'Unsupported parameter source');p=p[:,0].copy();si=surfacekeys.index(surfacekey(m));ri=roomkeys.index(roomkey(m));x=(si%sg)*512;y=(si//sg)*512;window=bool(p[0,2]>0 and np.any(p[2,:2]))
  for j,s in enumerate([4,1,6]):
   a=image(src[s])
   if j==2 and np.any(p[2,:2]):
    a=a.copy();height=a[:,:,0].astype(float);a[:,:,0]=np.clip(np.round(23+(height-23)/sg),0,255).astype(np.uint8);a[:,:,1]=np.clip(a[:,:,1].astype(float)*sg,0,255).astype(np.uint8)
   maps[j][y:y+512,x:x+512]=np.asarray(Image.fromarray(a).resize((512,512),Image.Resampling.NEAREST))
  sp=image(src[5])
  for layer in range(2):
   s=int(round(float(p[0,layer])*512))%512;dest=i*4+layer*2;pal[:,dest:dest+2]=sp[:,np.array([s,s+1])%512];p[0,layer]=dest/512
  room=image(src[7])
  if p[0,2]>0 and diag.get('nativeControlProfile')=='window-relative-relief-hdr-room-v2':
   original=np.asarray(Image.open(ROOT/'office-room.png').convert('RGBA'))
   corrected=ctl.office_room_sheet(original)
   # Office-specific old recipe (scale 1/16, generated 512-square sheet).
   if room.shape==(512,512,4) and p[0,2]==1/16:
    if not room[:,:,3].any():corrected[:,:,3]=0
    room=corrected
  strip,state_evidence=room_strip(room,float(p[0,2]),float(p[0,3]),night_only=night_only and p[0,2]>0)
  maps[3][ri*64:(ri+1)*64,:1024]=strip
  for row in [1,2]:
   if np.any(p[row,:2]):p[row]=[p[row,0]/sg,p[row,1]/sg,p[row,2]/sg+x/(sg*512),p[row,3]/sg+y/(sg*512)]
  if p[0,2]!=0:p[0,2:]=[64/roomsize,ri*64/roomsize]
  p[3,2:]*=sg
  if not np.any(p[2,:2]):p[2]=[1/sg,1/sg,(sg-1)/sg,(sg-1)/sg]
  params[:,i]=p;stats.append({'group':i,'surfaceCell':si,'roomStrip':ri,'heightGain':1/sg,'sourceParams':m[1].tolist(),'packedParams':p.tolist(),'roomStates':state_evidence,'nightPolicyApplied':bool(night_only and m[1][0,0,2]>0)})
  pr=g['meshes'][0]['primitives'][0];a=pr['attributes'];pos=accessor(g,b,a['POSITION']);norm=accessor(g,b,a['NORMAL']);tan=accessor(g,b,a['TANGENT']);uv=accessor(g,b,a['TEXCOORD_0']);top=accessor(g,b,a['TEXCOORD_3']);seeds=accessor(g,b,a['TEXCOORD_1']);tri=accessor(g,b,pr['indices']).reshape(-1,3);verts=[];lookup={};remap=[]
  pos=rotate_xyz(pos,rotation_degrees);norm=rotate_xyz(norm,rotation_degrees);tan[:,:3]=rotate_xyz(tan[:,:3],rotation_degrees)
  for j in range(len(pos)):
   v={'position':pos[j].tolist(),'normal':norm[j].tolist(),'tangent':tan[j,:3].tolist(),'tangentSign':1.,'uvBase':uv[j].tolist(),'uvTop':top[j].tolist(),'materialRow':i,'seedByte':int(np.clip(round(float(seeds[j,1])*255),0,255))};k=tuple(v['position']+v['normal']+v['tangent']+v['uvBase']+v['uvTop'])+(v['seedByte'],)
   if k not in lookup:lookup[k]=len(verts);verts.append(v)
   remap.append(lookup[k])
  objects.append({'name':f'NativeGroup{i}','worldTransformBaked':True,'vertices':verts,'triangles':[[remap[int(j)] for j in t] for t in tri]})
 contract={'schema':'sc5.geometry-contract/1','unit':'meter','upAxis':'Z','uvConvention':'blender-v-up','materialMode':'authored-palette-v1','objects':objects};(OUT/'希望大厦.geometry.json').write_text(json.dumps(contract,separators=(',',':')),encoding='utf8');model,info=compile_contract(contract,(ROOT/'第四阶段研究/175EFB4E.research-template.rw4').read_bytes(),(ROOT/'第二阶段/CD6F8697.collision-source.rw4').read_bytes(),material_rows=width);(OUT/'希望大厦_单网格兼容.rw4').write_bytes(model)
 maps[1][-512:,-512:]=[128,128,255,255];maps[2][-512:,-512:]=[23,255,0,255]
 for name,a in zip(['tint','normal','surface','interior'],maps):Image.fromarray(a).save(OUT/(name+'.png'))
 Image.fromarray(pal).save(OUT/'palette.png');(OUT/'params.float32').write_bytes(params.tobytes());info.update(parameterWidth=width,sourceGroups=count,meshCount=1,materialAssignmentCount=1,surfaceGrid=sg,roomAtlasSize=roomsize,sourceProfile=diag.get('nativeControlProfile'),nightGainUnchanged=False,officeRoomStatesCorrected=True,transparentTopFallback=True,atlasReliefGain=1/sg)
 (OUT/'几何材质转换.json').write_text(json.dumps(info,ensure_ascii=False,indent=2),encoding='utf8');(OUT/'材质采样变换.json').write_text(json.dumps(stats,ensure_ascii=False,indent=2),encoding='utf8');print('SINGLE_COMPATIBLE',info['vertices'],info['triangles'],sg)
if __name__=='__main__':prepare()
