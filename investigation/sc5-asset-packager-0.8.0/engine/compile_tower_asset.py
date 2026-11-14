"""LOTM9 -> single Building40, tiled native data maps, no preview baking."""
from probe_tower import *
from authored_materials import texture_rw4,raster_rgba
from compile_geometry_contract import compile_contract
from rw4_bridge import RW4,require
def accessor(g,b,i):
 a=g['accessors'][i];v=g['bufferViews'][a['bufferView']];n={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[a['type']];dt={5126:'<f4',5125:'<u4',5123:'<u2'}[a['componentType']]
 return np.frombuffer(b,dt,count=a['count']*n,offset=v.get('byteOffset',0)+a.get('byteOffset',0)).reshape(-1,n).copy()
def image(b):return np.asarray(Image.open(io.BytesIO(b)).convert('RGBA')).copy()
def compile_input():
 chunks,mats,kinds,diag=read();objects=[];before=0;signs={};pal=np.zeros((16,512,4),np.uint8);params=np.zeros((4,64,4),'<f4')
 roomkey=lambda m:(hashlib.sha256(m[0][7]).hexdigest(),float(m[1][0,0,2]),float(m[1][0,0,3]))
 surface_keys=list(dict.fromkeys(tuple(hashlib.sha256(m[0][s]).hexdigest() for s in [4,1,6]) for m in mats));room_keys=list(dict.fromkeys(roomkey(m) for m in mats))
 grid=lambda n:2**int(np.ceil(np.log2(np.ceil(np.sqrt(n)))))
 sg=grid(len(surface_keys));roomsize=max(1024,64*2**int(np.ceil(np.log2(len(room_keys)))));atlases=[np.zeros((sg*512,sg*512,4),np.uint8) for _ in range(3)]+[np.zeros((roomsize,roomsize,4),np.uint8)]
 for i,((g,b),(maps,p,cols)) in enumerate(zip(chunks,mats)):
  require(cols==1,'Multiple source parameter columns need explicit row mapping');p=p[:,0].copy();srcpal=image(maps[5]);si=surface_keys.index(tuple(hashlib.sha256(maps[s]).hexdigest() for s in [4,1,6]));ri=room_keys.index(roomkey((maps,p[:,None,:],cols)));x=(si%sg)*512;y=(si//sg)*512
  for j,slot in enumerate([4,1,6]):
   im=Image.open(io.BytesIO(maps[slot])).convert('RGBA').resize((512,512),Image.Resampling.NEAREST)
   atlases[j][y:y+512,x:x+512]=np.asarray(im)
  source_room=image(maps[7]);rh,rw=source_room.shape[:2];tile=(np.arange(64)+.5)/64
  for state in range(16):
   xx=np.floor(np.mod((tile+state)*p[0,2],1)*rw).astype(int);yy=np.floor(np.mod(tile*p[0,2]+p[0,3],1)*rh).astype(int)
   atlases[3][ri*64:(ri+1)*64,state*64:(state+1)*64]=source_room[yy[:,None],xx[None,:]]
  for layer in range(2):
   src=int(round(float(p[0,layer])*512))%512;dest=i*4+layer*2
   pal[:,dest:dest+2]=srcpal[:,np.array([src,src+1])%512];p[0,layer]=dest/512
  for row in [1,2]:
   if np.any(p[row,:2]):p[row]=[p[row,0]/sg,p[row,1]/sg,p[row,2]/sg+x/(sg*512),p[row,3]/sg+y/(sg*512)]
  if p[0,2]!=0:p[0,2:]=[64/roomsize,ri*64/roomsize]
  p[3,2:]*=sg;params[:,i]=p
  pr=g['meshes'][0]['primitives'][0];a=pr['attributes'];pos=accessor(g,b,a['POSITION']);norm=accessor(g,b,a['NORMAL']);tan=accessor(g,b,a['TANGENT']);uv=accessor(g,b,a['TEXCOORD_0']);top=accessor(g,b,a['TEXCOORD_3']);ix=accessor(g,b,a['TEXCOORD_1']);tri=accessor(g,b,pr['indices']).reshape(-1,3)
  before+=len(pos);verts=[];lookup={};remap=[]
  for j in range(len(pos)):
   # Exported LOTM chunks remain Z-up/metres. Front is source -Y.
   po=pos[j]*[-1,-1,1];no=norm[j]*[-1,-1,1];ta=tan[j,:3]*[-1,-1,1];seed=int(np.clip(round(float(ix[j,1])*255),0,255));signs[str(tan[j,3])]=signs.get(str(tan[j,3]),0)+1
   v={'position':po.tolist(),'normal':no.tolist(),'tangent':ta.tolist(),'tangentSign':1.,'uvBase':uv[j].tolist(),'uvTop':top[j].tolist(),'materialRow':i,'seedByte':seed}
   key=tuple(po)+tuple(no)+tuple(ta)+tuple(uv[j])+tuple(top[j])+(i,seed)
   if key not in lookup:lookup[key]=len(verts);verts.append(v)
   remap.append(lookup[key])
  triangles=[[remap[int(j)] for j in t] for t in tri]
  objects.append({'name':g['meshes'][0]['name'],'worldTransformBaked':True,'vertices':verts,'triangles':triangles})
 contract={'schema':'sc5.geometry-contract/1','upAxis':'Z','unit':'meter','uvConvention':'blender-v-up','materialMode':'authored-palette-v1','objects':objects}
 (OUT/'希望大厦.geometry.json').write_text(json.dumps(contract,separators=(',',':')),encoding='utf8')
 print('GEOMETRY',before,sum(len(o['vertices']) for o in objects),sum(len(o['triangles']) for o in objects),flush=True)
 model,info=compile_contract(contract,(ROOT/'第四阶段研究/175EFB4E.research-template.rw4').read_bytes(),(ROOT/'第二阶段/CD6F8697.collision-source.rw4').read_bytes(),material_rows=64)
 (OUT/'希望大厦.rw4').write_bytes(model)
 for name,arr in zip(['tint','normal','surface','interior'],atlases):Image.fromarray(arr).save(OUT/(name+'.png'))
 Image.fromarray(pal).save(OUT/'palette.png');(OUT/'params.float32').write_bytes(params.tobytes())
 info.update(materialRows=61,parameterWidth=64,sourceVertices=before,sourceTangentSigns=signs,sourceGroupCount=len(chunks),sourceTriangleCount=sum(len(o['triangles']) for o in objects),surfaceAtlasSize=sg*512,interiorAtlasSize=roomsize,uniqueSurfaceGroups=len(surface_keys),uniqueInteriorGroups=len(room_keys),frontAxis='source -Y rotated to game +Y',groundMeshCount=0)
 (OUT/'几何材质转换.json').write_text(json.dumps(info,ensure_ascii=False,indent=2),encoding='utf8')
def material_factory(model,allocate,native):
 f=(ROOT/'材质测试0.2.0/DCBEA56D.texture-template.rw4').read_bytes();r=(ROOT/'材质测试0.2.0/2EF7D20A.texture-template.rw4').read_bytes();width=json.loads((OUT/'几何材质转换.json').read_text('utf8')).get('parameterWidth',64);p=np.frombuffer((OUT/'params.float32').read_bytes(),'<f4').reshape(4,width,4)
 arrays=[image((OUT/(n+'.png')).read_bytes()) for n in ['tint','normal','surface','palette','interior']]
 payloads=[texture_rw4(f,p,116),*[raster_rgba(a) for a in arrays[:3]],texture_rw4(r,arrays[3],21),texture_rw4(r,arrays[4],21)]
 ids=[allocate('tower-native-slot-'+str(i)) for i in range(6)];resources={(0x2f4e681b if i in (0,4,5) else 0x2f4e681c,0,ids[i]):v for i,v in enumerate(payloads)}
 rw=RW4(model);mat=rw.of_type(0x2000b)[0];data=bytearray(mat.payload);at=32+rw.of_type(0x20004)[0].size
 for i,ident in enumerate(ids):struct.pack_into('<I',data,at+(i+1)*24+8,ident)
 rw.set(mat.number,data)
 manifest={'profile':'lotm9-native-atlas-v1','materialRows':61,'opaque':False,'interiorProjectionEnabled':True,'nightEmissionEnabled':True,'slots':[{'slot':i,'tgi':f'{k[0]:08X}:00000000:{k[2]:08X}','bytes':len(v),'sha256':hashlib.sha256(v).hexdigest()} for i,(k,v) in enumerate(resources.items())],'notes':['61 LOTM groups preserved; native maps tiled, not display RGB baked','Day/night content comes from source asset; game behavior requires testing','Source tangent handedness recorded; Building40 profile retains native packed tangent convention']}
 return rw.write(),resources,manifest
if __name__=='__main__':compile_input()
