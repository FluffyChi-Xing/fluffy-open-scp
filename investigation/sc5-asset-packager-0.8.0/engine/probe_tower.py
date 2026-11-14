from pathlib import Path
import struct,json,io,hashlib
import numpy as np
from PIL import Image
ROOT=Path(__file__).resolve().parent;OUT=ROOT/'希望大厦研究0.8.0'
def glb(b):
 n=struct.unpack_from('<I',b,12)[0];return json.loads(b[20:20+n]),b[28+n:]
def read(path=None):
 b=Path(path or OUT/'输入副本/大连希望大厦.lotm').read_bytes();at=12;chunks=[]
 def take():
  nonlocal at
  n=struct.unpack_from('<I',b,at)[0];at+=4;v=b[at:at+n];at+=n;return v
 for _ in range(struct.unpack_from('<I',b,8)[0]):chunks.append(glb(take()))
 n=struct.unpack_from('<I',b,at)[0];at+=4;materials=[]
 for i in range(n):
  maps=[take() for _ in range(8)];params=np.frombuffer(take(),'<f4').reshape(4,-1,4).copy();cols=struct.unpack_from('<I',b,at)[0];at+=4
  materials.append((maps,params,cols))
 kinds=[struct.unpack_from('<IB',b,at+i*5) for i in range(n)];at+=5*n;diagnostic=json.loads(take());assert at==len(b)
 return chunks,materials,kinds,diagnostic
if __name__=='__main__':
 c,m,k,d=read();print('stats',d.get('triangles'),d.get('dimensionsMeters'),k)
 for i,(maps,p,cols) in enumerate(m):
  sizes=[Image.open(io.BytesIO(x)).size if x else None for x in maps];g,bb=c[i];print(i,sizes,p.tolist(),g['accessors'][0]['count'])
