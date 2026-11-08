"""Prototype pure-Python geometry contract -> native static Building40 RW4."""
from pathlib import Path
import json,struct,hashlib,math
from rw4_bridge import RW4,require,export_obj
from collision_proxy import replace_box_collision,Collision
ROOT=Path(__file__).resolve().parent;OUT=ROOT/'第四阶段研究'

def compile_contract(document,template,collision_source,material_rows=1,collision_bounds=None):
    require(document['schema']=='sc5.geometry-contract/1','Unknown geometry contract')
    require(document['unit']=='meter' and document['upAxis']=='Z','Explicit meter/Z-up conversion is required')
    require(document.get('uvConvention')=='blender-v-up','Unknown UV convention; do not guess or flip twice')
    require(document['materialMode'] in ('native-template','authored-palette-v1'),'Unsupported material compiler profile')
    if document['materialMode']=='native-template':require(material_rows==1,'Native-template prototype uses one material row')
    else:require(isinstance(material_rows,int) and 1<=material_rows<=256,'Invalid authored material row count')
    vertices=[];triangles=[];records=[]
    for obj in document['objects']:
        require(obj['worldTransformBaked'],'Bake object transforms first');offset=len(vertices)
        for v in obj['vertices']:
            require(isinstance(v['materialRow'],int) and 0<=v['materialRow']<material_rows,'Material row outside compiled parameter table')
            require(v['tangentSign']==1,'Mirrored tangent-frame encoding is unsupported by this profile')
            require(isinstance(v['seedByte'],int) and 0<=v['seedByte']<=255,'Invalid seed byte')
            require(len(v['uvTop'])==2,'Invalid top-layer UV')
            require(all(math.isfinite(x) for x in v['uvTop']),'Non-finite top-layer UV')
            vertices.append((v['position'],v['normal'],v['tangent'],v['uvBase']));records.append(v)
        for tri in obj['triangles']:
            require(len(tri)==3 and all(isinstance(i,int) and 0<=i<len(obj['vertices']) for i in tri),'Object-local index out of range')
            require(len(set(tri))==3,'Degenerate triangle indices')
            p0,p1,p2=[obj['vertices'][i]['position'] for i in tri]
            a=[p1[j]-p0[j] for j in range(3)];b=[p2[j]-p0[j] for j in range(3)]
            cross=(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
            require(sum(v*v for v in cross)>1e-20,'Zero-area triangle')
            triangles.append(tuple(i+offset for i in tri))
    model=RW4(template);require(model.profile()[6][5]==40,'Expected verified native Building40 profile')
    info=model.replace_geometry(vertices,triangles)
    profile=model.profile();vb=profile[3];payload=bytearray(vb.payload)
    for i,v in enumerate(records):
        # Historical AdvancedCollada FLOAT4 adapter uses 1-v for both layers.
        # The IR stays in Blender coordinates; perform this exactly once here.
        struct.pack_into('<4f',payload,i*40+24,v['uvBase'][0],1-v['uvBase'][1],v['uvTop'][0],1-v['uvTop'][1]);payload[i*40+22]=v['materialRow'];payload[i*40+23]=v['seedByte']
    model.set(vb.number,payload)
    info['collision']=replace_box_collision(model,RW4(collision_source),*(collision_bounds or info['bounds']))
    result=model.write();check=RW4(result);check.decode();cc=check.of_type(0x80003)[0];Collision(cc.payload,cc.pos).box_proxy()
    material_note='Native template material and six external textures retained' if document['materialMode']=='native-template' else 'Authored material rows written; custom textures linked in the subsequent material compiler'
    info.update(gameReady=False,profile='native-static-building40',uvAdapter='blender-v-up -> Float4 base/top: v=1-v, applied once; appearance not game-verified',bytes=len(result),sha256=hashlib.sha256(result).hexdigest(),limitations=[material_note,'Complex windows/interiors are not compiled','No game execution performed'])
    return result,info

def main():
    contract_path=OUT/'小屋.geometry.json';template=OUT/'175EFB4E.research-template.rw4';collision=ROOT/'第二阶段/CD6F8697.collision-source.rw4'
    doc=json.loads(contract_path.read_text(encoding='utf8'));result,info=compile_contract(doc,template.read_bytes(),collision.read_bytes())
    info.update(inputContractSha256=hashlib.sha256(contract_path.read_bytes()).hexdigest(),templateSha256=hashlib.sha256(template.read_bytes()).hexdigest(),collisionTemplateSha256=hashlib.sha256(collision.read_bytes()).hexdigest())
    (OUT/'小屋.Building40.experimental.rw4').write_bytes(result);export_obj(RW4(result),OUT/'小屋.Building40.decoded.obj')
    (OUT/'Building40编译验证.json').write_text(json.dumps(info,ensure_ascii=False,indent=2),encoding='utf8');print(json.dumps(info,ensure_ascii=True))

if __name__=='__main__':main()
