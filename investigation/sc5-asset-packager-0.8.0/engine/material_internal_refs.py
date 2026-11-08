"""Strict sampler-tail profile for the verified 525-byte Building40 material.

The external six-slot resource IDs are shader data, NOT the entire material.
The remaining native texture slots contain local RW4 section indices too.
"""
import struct
from rw4_bridge import require
TAIL_OFFSET=373
def sampler_refs(payload):
 require(len(payload)==525 and struct.unpack_from('<I',payload,0)[0]==525,'Unrecognized native material tail; refuse implicit relocation')
 require(struct.unpack_from('<I',payload,24)[0]==0x80000002,'Unexpected native material renderer')
 at=TAIL_OFFSET;refs=[]
 def u():
  nonlocal at
  require(at+4<=len(payload),'Truncated sampler tail');v=struct.unpack_from('<i',payload,at)[0];at+=4;return v
 while True:
  slot=u()
  if slot==-1:break
  require(0<=slot<=16,'Invalid native sampler index');offset=at;raster=u();require(raster>=-1,'Invalid raster index');refs.append({'sampler':slot,'offset':offset,'section':raster})
  for name,maximum in [('stage',32),('sampler',13)]:
   mask=u()
   if mask:
    while True:
     state=u()
     if state==-1:break
     require(1<=state<=maximum,'Invalid native state identifier');u()
 require(at==len(payload),'Unparsed material tail bytes');return refs
def relocate(payload,mapping):
 data=bytearray(payload)
 for r in sampler_refs(payload):
  if r['section']<0:continue
  require(r['section'] in mapping,'Native sampler raster has no relocation mapping');struct.pack_into('<i',data,r['offset'],mapping[r['section']])
 return bytes(data)
def validate_model(model):
 result=[]
 meshes={s.number for s in model.of_type(0x20009)};assigned=set()
 for link in model.of_type(0x2001a):
  require(len(link.payload)==12,'Unsupported mesh/material assignment')
  mesh,count,mat=struct.unpack('<3I',link.payload)
  require(count==1 and mesh in meshes and mesh not in assigned,'Invalid or duplicate mesh assignment')
  require(mat<model.count and model.sections[mat].kind==0x2000b,'Mesh assignment points to non-material')
  assigned.add(mesh)
 require(meshes==assigned,'Mesh material assignment coverage incomplete')
 for mat in model.of_type(0x2000b):
  for r in sampler_refs(mat.payload):
   n=r['section']
   if n<0:continue
   require(n<model.count,f'Material {mat.number} sampler {r["sampler"]}: raster index out of range')
   kind=model.sections[n].kind
   require(kind==0x20003,f'Material {mat.number} sampler {r["sampler"]}: raster points to {kind:08X}, expected texture 00020003')
   texture=model.sections[n];require(len(texture.payload)==32,'Unexpected internal texture header');blob=struct.unpack_from('<I',texture.payload,28)[0]
   require(blob<model.count and model.sections[blob].kind==0x10030,'Internal texture points to non-blob data')
   result.append({'material':mat.number,**r,'targetKind':f'{kind:08X}'})
 return result
