"""Native collision proxy parsing and positive affine transformation.

Preserves a known box's triangulation, attributes and KD tree. It does not invent
or regenerate the undocumented collision-tree encoding for arbitrary topology.
"""
import math,struct
from rw4_bridge import require,align

class Collision:
    def __init__(self,payload,base):
        self.payload=bytes(payload);self.base=base
        require(len(payload)>=116,'Truncated collision')
        self.low=struct.unpack_from('<3f',payload,0);self.high=struct.unpack_from('<3f',payload,16)
        require(all(math.isfinite(x) for x in self.low+self.high),'Non-finite collision bounds')
        magic,depth,self.nt,zero,self.nv,pt,pv,p4,p3=struct.unpack_from('<9I',payload,32)
        self.magic=magic;self.depth=depth
        self.pt,self.pv,self.p4,self.p3=(x-base for x in (pt,pv,p4,p3))
        require(zero==0 and 0<self.nv<=1000000 and 0<self.nt<=1000000,'Invalid collision counts')
        require(pv==align(base+68,16) and pt==pv+self.nv*16 and p3==pt+self.nt*16,'Unsupported collision layout')
        require(p4==align(p3+((self.nt+7)//8)*4,16),'Unsupported collision attribute layout')
        require(self.p4+48<=len(payload),'Truncated collision tail')
        anchor,self.nodes,repeat,zero=struct.unpack_from('<4I',payload,self.p4)
        require(anchor==pv-32 and repeat==self.nt and zero==0,'Invalid collision tail header')
        require(self.p4+48+self.nodes*32==len(payload),'Unknown collision trailing data')
        require(payload[:32]==payload[self.p4+16:self.p4+48],'Collision bounds copies disagree')
        self.vertices=[struct.unpack_from('<3f',payload,self.pv+i*16) for i in range(self.nv)]
        self.triangles=[]
        for i in range(self.nt):
            a,b,c,flag=struct.unpack_from('<4I',payload,self.pt+i*16)
            require(max(a,b,c)<self.nv,'Collision triangle index out of range')
            self.triangles.append((a,b,c))
        require(all(self.low[j]-1e-4<=p[j]<=self.high[j]+1e-4 for p in self.vertices for j in range(3)),'Collision vertices outside bounds')
        self.tree=[struct.unpack_from('<6i2f',payload,self.p4+48+i*32) for i in range(self.nodes)]

    def validate_tree(self):
        """Validate observed branch schema against triangle coordinate extrema."""
        seen=set()
        def descend(i):
            require(0<=i<self.nodes and i not in seen,'Invalid collision tree reference/cycle');seen.add(i)
            tag,axis,nl,il,nr,ir,splitl,splitr=self.tree[i]
            require(tag==0 and axis in (0,1,2),'Unsupported collision branch encoding')
            def child(n,start):
                if n==-1:return descend(start)
                require(n>0 and 0<=start<start+n<=self.nt,'Invalid leaf range')
                return list(range(start,start+n))
            left=child(nl,il);right=child(nr,ir)
            maximum=max(self.vertices[v][axis] for t in left for v in self.triangles[t])
            minimum=min(self.vertices[v][axis] for t in right for v in self.triangles[t])
            require(maximum<=splitl+1e-4 and minimum>=splitr-1e-4,'Tree split excludes collision geometry')
            return left+right
        require(self.nodes>0,'No explicit collision tree')
        covered=descend(0)
        require(len(seen)==self.nodes and sorted(covered)==list(range(self.nt)),'Collision tree does not cover each triangle exactly once')
        return True

    def box_proxy(self):
        self.validate_tree()
        require(self.nt==12 and self.nv==24,'Expected native six-face box')
        corners={tuple(p) for p in self.vertices}
        expected={(x,y,z) for x in (self.low[0],self.high[0]) for y in (self.low[1],self.high[1]) for z in (self.low[2],self.high[2])}
        require(corners==expected and len(corners)==8,'Proxy is not a box')
        # Confirm six rectangle faces, two triangles each, not just eight corners.
        faces={}
        for tri in self.triangles:
            fixed=[(j,self.vertices[tri[0]][j]) for j in range(3) if len({self.vertices[v][j] for v in tri})==1]
            require(len(fixed)==1,'Invalid box face triangle');faces[fixed[0]]=faces.get(fixed[0],0)+1
        require(len(faces)==6 and set(faces.values())=={2},'Incomplete box surface')

    def transformed(self,low,high,destination):
        self.box_proxy()
        require(destination%16==0,'Collision destination must be 16-byte aligned')
        require(all(math.isfinite(x) for x in low+high) and all(high[j]>low[j] for j in range(3)),'Invalid proxy dimensions')
        scale=[(high[j]-low[j])/(self.high[j]-self.low[j]) for j in range(3)]
        def transform(x,j):return (x-self.low[j])*scale[j]+low[j]
        p=bytearray(self.payload)
        for off in (0,self.p4+16):
            struct.pack_into('<3f',p,off,*low);struct.pack_into('<3f',p,off+16,*high)
        for i,v in enumerate(self.vertices):struct.pack_into('<3f',p,self.pv+i*16,*(transform(v[j],j) for j in range(3)))
        for i,node in enumerate(self.tree):
            axis=node[1];struct.pack_into('<2f',p,self.p4+48+i*32+24,transform(node[6],axis),transform(node[7],axis))
        for at,relative in ((52,self.pt),(56,self.pv),(60,self.p4),(64,self.p3)):
            struct.pack_into('<I',p,at,destination+relative)
        struct.pack_into('<I',p,self.p4,destination+self.pv-32)
        result=bytes(p);check=Collision(result,destination);check.validate_tree();return result

def replace_box_collision(model,source,low,high):
    dst=model.of_type(0x80003);src=source.of_type(0x80003)
    require(len(dst)==len(src)==1,'Expected one collision section')
    proxy=Collision(src[0].payload,src[0].pos)
    model.set(dst[0].number,proxy.transformed(tuple(low),tuple(high),dst[0].pos),allow_shrink=True)
    return {'kind':'native-affine-box-proxy','triangles':proxy.nt,'vertices':proxy.nv,'treeNodes':proxy.nodes,'bounds':[low,high],'treeValidated':True}
