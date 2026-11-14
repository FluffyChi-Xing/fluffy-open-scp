"""Small DBPF v3 reader/writer for offline candidate builds; no install API."""
from pathlib import Path
from dataclasses import dataclass
import struct
from rw4_bridge import require

def refpack(data,size):
    p=2+(4 if data[0]&128 else 3)*(2 if data[0]&1 else 1);out=bytearray()
    while p<len(data):
        b=data[p];p+=1;stop=False;count=distance=0
        if b>=0xe0:
            if b>=0xfc:literal=b&3;stop=True
            else:literal=((b&31)+1)*4
        elif b>=0xc0:
            x,y,z=data[p:p+3];p+=3;literal=b&3;count=((b&12)<<6|z)+5;distance=(((b&16)<<4|x)<<8|y)+1
        elif b>=0x80:
            x,y=data[p:p+2];p+=2;literal=x>>6;count=(b&63)+4;distance=((x&63)<<8|y)+1
        else:
            x=data[p];p+=1;literal=b&3;count=((b&28)>>2)+3;distance=((b&96)<<3|x)+1
        require(p+literal<=len(data),'Truncated RefPack literal');out.extend(data[p:p+literal]);p+=literal
        if count:
            require(0<distance<=len(out),'Invalid RefPack backreference')
            for _ in range(count):out.append(out[-distance])
        require(len(out)<=size,'RefPack overrun')
        if stop:break
    require(len(out)==size,'RefPack size mismatch');return bytes(out)

@dataclass(frozen=True)
class Entry:
    path:Path;key:tuple;offset:int;stored:int;size:int;compression:int;instance_high:int=0
    def read(self):
        require(self.instance_high==0,'64-bit instance not supported for selected resource')
        with self.path.open('rb') as f:f.seek(self.offset);data=f.read(self.stored)
        require(len(data)==self.stored,'Truncated payload')
        require(self.compression in (0,0xffff),'Unknown resource compression')
        if self.compression:return refpack(data,self.size)
        require(len(data)==self.size,'Uncompressed payload size mismatch');return data

def index(path,allow_duplicates=False):
    path=Path(path);size=path.stat().st_size
    with path.open('rb') as f:
        h=f.read(96);require(len(h)==96 and h[:4]==b'DBPF','Not DBPF')
        require(struct.unpack_from('<I',h,4)[0]==3,'Only DBPF v3 supported')
        count=struct.unpack_from('<I',h,0x24)[0];length=struct.unpack_from('<I',h,0x2c)[0];off=struct.unpack_from('<I',h,0x40)[0]
        require(count<=1000000 and 0<=off<=off+length<=size,'Invalid package index bounds')
        f.seek(off);data=f.read(length)
    if count==0 and length==0:return []
    require(len(data)>=4,'Truncated package index')
    flags=struct.unpack_from('<I',data)[0];require(flags&~7==0,'Unknown index flags')
    p=4;shared={}
    for i in range(3):
        if flags&(1<<i):shared[i]=struct.unpack_from('<I',data,p)[0];p+=4
    out=[];seen=set()
    for _ in range(count):
        ids=[]
        for i in range(3):
            if i in shared:ids.append(shared[i])
            else:ids.append(struct.unpack_from('<I',data,p)[0]);p+=4
        ident,offset,stored,unpacked,comp,other=struct.unpack_from('<IIIIHH',data,p);p+=20
        stored&=0x7fffffff;key=(ids[0],ids[1],ident)
        require(0<=offset<=offset+stored<=size,'Resource outside package')
        identity=(*key,ids[2]);require(allow_duplicates or identity not in seen,'Duplicate TGI');seen.add(identity)
        out.append(Entry(path,key,offset,stored,unpacked,comp,ids[2]))
    require(p<=len(data),'Index overrun');return out

def package_bytes(resources):
    resources=sorted(resources.items());count=len(resources);length=8+28*count;offset=96+length
    h=bytearray(96);h[:4]=b'DBPF';struct.pack_into('<I',h,4,3)
    for at,value in ((0x24,count),(0x2c,length),(0x3c,3),(0x40,96)):struct.pack_into('<I',h,at,value)
    ix=bytearray(struct.pack('<II',4,0));payload=bytearray()
    for (t,g,i),data in resources:
        require(len(data)<0x80000000,'Payload too large')
        ix.extend(struct.pack('<6IHH',t,g,i,offset,len(data)|0x80000000,len(data),0,0));payload.extend(data);offset+=len(data)
    return bytes(h+ix+payload)
