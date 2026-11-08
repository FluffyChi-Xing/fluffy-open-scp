"""Experimental geometry bridge. No game installation or complete asset packaging.

Supports explicit Museum-52, static Building40 and Float36 single-mesh profiles.
Complete independent asset packaging is not implemented: output is NOT game-ready.
"""
from __future__ import annotations
import argparse, hashlib, json, math, struct
from dataclasses import dataclass
from pathlib import Path

MAGIC=bytes.fromhex('89525734773332000d0a1a0a00200400343534003030300000000000')
BLOB=0x10030

class Unsupported(ValueError): pass

def require(ok, message):
    if not ok: raise Unsupported(message)

def align(n,a):
    require(a>0 and a&(a-1)==0,'Invalid section alignment')
    return (n+a-1)&-a

@dataclass
class Section:
    number:int; pos:int; size:int; alignment:int; indirect:int; kind:int; payload:bytes

class RW4:
    def __init__(self,data:bytes):
        require(len(data)>=152 and data[:28]==MAGIC,'Unsupported RW4 header')
        self.original=data
        self.file_type,self.count=struct.unpack_from('<II',data,28)
        self.index=struct.unpack_from('<I',data,48)[0]
        self.end=struct.unpack_from('<I',data,68)[0]
        require(0<self.count<100000 and self.index+self.count*24<=self.end<=len(data),'Invalid section index')
        self.sections=[]
        for i in range(self.count):
            p,zero,size,a,ind,k=struct.unpack_from('<6I',data,self.index+i*24)
            p+=self.end if k==BLOB else 0
            require(zero==0 and 0<=p<=p+size<=len(data),'Section out of bounds')
            align(p,a)
            require(p%a==0,'Misaligned section')
            require((p>=self.end if k==BLOB else p+size<=self.index),'Unsupported section placement')
            self.sections.append(Section(i,p,size,a,ind,k,data[p:p+size]))
        spans=sorted((s.pos,s.pos+s.size) for s in self.sections if s.size)
        require(all(a[1]<=b[0] for a,b in zip(spans,spans[1:])),'Overlapping sections')
        first=struct.unpack_from('<I',data,52)[0]
        require(first+28<=self.index,'Bad header sections')
        self.fixup_header=first+struct.unpack_from('<I',data,first+20)[0]
        require(self.fixup_header+40<=self.index,'Bad fixup metadata')
        require(struct.unpack_from('<I',data,self.fixup_header)[0]==0x10007,'Bad fixup header')
        n=struct.unpack_from('<I',data,self.fixup_header+4)[0]
        require(self.index+self.count*24+n*8<=self.end,'Bad fixup count')
        self.fixups=[struct.unpack_from('<II',data,self.index+self.count*24+i*8) for i in range(n)]
        for target,offset in self.fixups:
            require(target<self.count and offset+4<=self.sections[target].size,'Fixup out of range')
        self.changed=set()

    def of_type(self,kind): return [s for s in self.sections if s.kind==kind]

    def set(self,n,payload,allow_shrink=False):
        s=self.sections[n]
        require(not any(t==n for t,_ in self.fixups),'Cannot edit section containing unknown fixups')
        require(s.kind==BLOB or len(payload)==s.size or (allow_shrink and len(payload)<s.size),'Non-blob growth requires unsupported relocation')
        s.payload=bytes(payload);self.changed.add(n)

    def write(self):
        # Keep all non-blob offsets, indices and fixups stable. Reflow only blobs.
        out=bytearray(self.original[:self.end]);positions={}
        for s in sorted(self.of_type(BLOB),key=lambda x:x.pos):
            p=align(len(out),s.alignment);out.extend(b'\0'*(p-len(out)))
            positions[s.number]=p;out.extend(s.payload)
        for s in self.sections:
            p=positions.get(s.number,s.pos)
            if s.kind!=BLOB:
                out[p:p+s.size]=b'\0'*s.size
                out[p:p+len(s.payload)]=s.payload
            struct.pack_into('<6I',out,self.index+s.number*24,p-self.end if s.kind==BLOB else p,0,len(s.payload),s.alignment,s.indirect,s.kind)
        struct.pack_into('<I',out,76,len(out)-self.end)
        rebuilt=bytes(out);RW4(rebuilt)
        return rebuilt

    def profile(self,mesh_number=None):
        require(self.file_type==1,'Expected model')
        if mesh_number is None:require(len(self.of_type(0x20009))==1,'Specify mesh number for multi-mesh profiles')
        mesh=self.sections[mesh_number] if mesh_number is not None else self.of_type(0x20009)[0];require(mesh.kind==0x20009,'Expected mesh');m=struct.unpack('<10I',mesh.payload)
        require(m[0:2]==(40,4) and m[4]==1 and m[5]==0 and m[6]==m[3]*3,'Unsupported draw layout')
        require(m[2]<self.count and m[9]<self.count,'Bad mesh refs')
        va=self.sections[m[9]];ta=self.sections[m[2]]
        require(va.kind==0x20005 and ta.kind==0x20007,'Bad buffer reference types')
        v=struct.unpack('<7I',va.payload);t=struct.unpack('<7I',ta.payload)
        require(v[0]<self.count and v[6]<self.count and t[6]<self.count,'Bad buffer refs')
        vf=self.sections[v[0]];vb=self.sections[v[6]];ib=self.sections[t[6]]
        require(vf.kind==0x20004 and vb.kind==ib.kind==BLOB,'Bad format/blob types')
        require(v[5] in (36,40,52) and t[4] in (101,102) and v[3]*v[5]==len(vb.payload) and t[2]*(2 if t[4]==101 else 4)==len(ib.payload),'Unsupported stride/index encoding')
        count=struct.unpack_from('<H',vf.payload,12)[0]
        decl=[]
        for j in range(count):
            q=24+j*12;off,typ,use=struct.unpack_from('>HHH',vf.payload,q+1)
            decl.append((off,typ,use,vf.payload[q+7]))
        building=[(0,2,0,0),(12,5,3,0),(16,5,6,0),(20,4,10,0),(24,3,5,0)]
        expected=(building+[(40,5,5,1),(44,7,5,2)] if v[5]==52 else building if v[5]==40 else [(0,2,0,0),(12,2,3,0),(24,2,6,0)])
        require(decl==expected,'Unsupported vertex declaration')
        require(m[8]==v[3] and m[6]==t[2],'Shared/sliced pools not supported')
        if v[5]==52:
            for i in range(v[3]):
                require(vb.payload[i*52+40:i*52+52]==b'\0'*12,'Nonzero special vertex fields are unsupported')
        return mesh,va,ta,vb,ib,m,v,t

    def decode(self):
        if len(self.of_type(0x20009))>1:
            points=[];triangles=[]
            for mesh in self.of_type(0x20009):
                _,_,_,vb,ib,m,v,t=self.profile(mesh.number);offset=len(points)
                points.extend(struct.unpack_from('<3f',vb.payload,i*v[5]) for i in range(v[3]))
                ix=struct.unpack('<'+('H' if t[4]==101 else 'I')*t[2],ib.payload);require(ix and max(ix)<v[3],'Invalid multi-mesh indices')
                triangles.extend([offset+ix[i+j] for j in range(3)] for i in range(0,len(ix),3))
            require(all(math.isfinite(x) for p in points for x in p),'Non-finite multi-mesh positions');return points,triangles
        mesh,va,ta,vb,ib,m,v,t=self.profile()
        points=[struct.unpack_from('<3f',vb.payload,i*v[5]) for i in range(v[3])]
        indices=list(struct.unpack('<'+('H' if t[4]==101 else 'I')*t[2],ib.payload))
        require(indices and max(indices)<len(points),'Invalid indices')
        require(all(math.isfinite(x) for p in points for x in p),'Non-finite position')
        return points,[indices[i:i+3] for i in range(0,len(indices),3)]

    def replace_geometry(self,vertices,triangles,material_row=0):
        require(3<=len(vertices)<=1000000 and triangles,'Geometry exceeds this template profile')
        require(0<=material_row<=255,'Material row out of range')
        mesh,va,ta,vb,ib,m,v,t=self.profile()
        out=bytearray()
        for vert in vertices:
            p,n,tangent,uv=vert
            require(len(p)==len(n)==len(tangent)==3 and len(uv)==2,'Malformed vertex')
            require(all(math.isfinite(float(x)) for a in vert for x in a),'Non-finite vertex')
            def packed(a):
                length=math.sqrt(sum(x*x for x in a));require(length>1e-9,'Zero direction')
                return bytes([max(0,min(255,round(x/length*127.5+127.5))) for x in a]+[255])
            if v[5] in (40,52):
                # Explicit Museum profile: ARGB, G=parameter row, B=window seed.
                out.extend(struct.pack('<3f',*p)+packed(n)+packed(tangent)+bytes((0,0,material_row,84))+struct.pack('<4f',*uv,*uv))
                if v[5]==52:out.extend(b'\0'*12)
            else:
                require(material_row==0,'Float36 profile has no material-row attribute')
                def normalise(a):
                    length=math.sqrt(sum(x*x for x in a));require(length>1e-9,'Zero direction');return [x/length for x in a]
                out.extend(struct.pack('<9f',*p,*normalise(n),*normalise(tangent)))
        flat=[i for tri in triangles for i in tri]
        require(all(len(tri)==3 for tri in triangles) and all(isinstance(i,int) and 0<=i<len(vertices) for i in flat),'Invalid triangle index')
        index32=len(vertices)>65536
        self.set(vb.number,out);self.set(ib.number,struct.pack('<'+('I' if index32 else 'H')*len(flat),*flat))
        mv=list(m);mv[3]=len(triangles);mv[6]=len(flat);mv[7]=min(flat);mv[8]=len(vertices)
        vv=list(v);vv[3]=len(vertices);tt=list(t);tt[2]=len(flat);tt[4]=102 if index32 else 101
        for s,values in ((mesh,mv),(va,vv),(ta,tt)): self.set(s.number,struct.pack('<'+'I'*len(values),*values))
        bounds=self.of_type(0x80005)
        require(len(bounds)==1 and bounds[0].size==32,'Unsupported bounds')
        p=bytearray(bounds[0].payload)
        lo=[min(a[0][i] for a in vertices) for i in range(3)];hi=[max(a[0][i] for a in vertices) for i in range(3)]
        struct.pack_into('<3f',p,0,*lo);struct.pack_into('<3f',p,16,*hi);self.set(bounds[0].number,p)
        return dict(vertices=len(vertices),triangles=len(triangles),bounds=[lo,hi],gameReady=False,
                    vertexStride=v[5],animationSections=len(self.of_type(0x70001)),
                    limitations=['Template collision retained','Template material and external textures retained','No independent Unit/menu/Lot generated','Not tested in game'])

def read_obj(path):
    """Static triangulated OBJ, preserving UV seams and hard edges.

    Recomputes per-triangle normals/tangents; no guessed coordinate conversion.
    Concave polygons must be triangulated by the authoring tool first.
    """
    p=[];uv=[];vertices=[];triangles=[]
    raw=Path(path).read_bytes()
    try:text=raw.decode('utf-8-sig')
    except UnicodeDecodeError:text=raw.decode('gb18030')
    for line in text.splitlines():
        a=line.partition('#')[0].split()
        if not a:continue
        if a[0]=='v':p.append(tuple(map(float,a[1:4])))
        elif a[0]=='vt':uv.append(tuple(map(float,a[1:3])))
        elif a[0]=='f':
            require(len(a)==4,'Triangulate OBJ faces before conversion')
            corners=[]
            for token in a[1:]:
                pair=token.split('/');idx=int(pair[0]);require(idx!=0,'OBJ zero index')
                pi=idx-1 if idx>0 else len(p)+idx;require(0<=pi<len(p),'OBJ position index out of range')
                tex=(0.,0.)
                if len(pair)>1 and pair[1]:
                    ix=int(pair[1]);require(ix!=0,'OBJ zero UV index');ix=ix-1 if ix>0 else len(uv)+ix
                    require(0<=ix<len(uv),'OBJ UV index out of range');tex=uv[ix]
                corners.append((p[pi],tex))
            a0,a1,a2=[x[0] for x in corners];e1=[y-x for x,y in zip(a0,a1)];e2=[y-x for x,y in zip(a0,a2)]
            n=(e1[1]*e2[2]-e1[2]*e2[1],e1[2]*e2[0]-e1[0]*e2[2],e1[0]*e2[1]-e1[1]*e2[0])
            require(sum(x*x for x in n)>1e-16,'Degenerate OBJ triangle')
            d1=[corners[1][1][j]-corners[0][1][j] for j in range(2)];d2=[corners[2][1][j]-corners[0][1][j] for j in range(2)]
            det=d1[0]*d2[1]-d1[1]*d2[0]
            tangent=tuple((e1[j]*d2[1]-e2[j]*d1[1])/det for j in range(3)) if abs(det)>1e-12 else tuple(e1)
            start=len(vertices);vertices.extend((pt,n,tangent,tex) for pt,tex in corners);triangles.append((start,start+1,start+2))
    require(vertices,'No OBJ triangles');return vertices,triangles

def export_obj(rw4,path):
    points,triangles=rw4.decode()
    Path(path).write_text('\n'.join(['# Geometry decoded back from experimental RW4']+['v '+' '.join(map(str,p)) for p in points]+['f '+' '.join(str(i+1) for i in t) for t in triangles])+'\n',encoding='utf-8')

def main():
    p=argparse.ArgumentParser(description=__doc__);s=p.add_subparsers(dest='command',required=True)
    q=s.add_parser('roundtrip');q.add_argument('template');q.add_argument('output')
    q=s.add_parser('convert');q.add_argument('template');q.add_argument('obj');q.add_argument('output');q.add_argument('--material-row',type=int,default=0)
    q.add_argument('--collision-proxy',help='Known native six-face box RW4; generates a matching bounding-box proxy')
    a=p.parse_args();dst=Path(a.output)
    require(dst.resolve()!=Path(a.template).resolve(),'Never overwrite the template')
    if a.command=='convert':require(dst.resolve()!=Path(a.obj).resolve(),'Never overwrite the input OBJ')
    rw=RW4(Path(a.template).read_bytes())
    report={'gameReady':False}
    if a.command=='convert':
        report=rw.replace_geometry(*read_obj(a.obj),a.material_row)
        if a.collision_proxy:
            require(dst.resolve()!=Path(a.collision_proxy).resolve(),'Never overwrite collision source')
            from collision_proxy import replace_box_collision
            report['collision']=replace_box_collision(rw,RW4(Path(a.collision_proxy).read_bytes()),*report['bounds'])
            report['limitations']=[x for x in report['limitations'] if x!='Template collision retained']
        if rw.of_type(0x70001) or rw.of_type(0x7000c):report['limitations'].append('Template animation/skeleton retained')
    result=rw.write();RW4(result).decode();dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(result)
    report.update(sha256=hashlib.sha256(result).hexdigest(),bytes=len(result),modifiedSections=sorted(rw.changed))
    if a.command=='roundtrip':report['byteIdentical']=result==rw.original
    else:export_obj(RW4(result),dst.with_suffix('.decoded.obj'))
    dst.with_suffix('.report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps(report,ensure_ascii=False))

if __name__=='__main__':main()
