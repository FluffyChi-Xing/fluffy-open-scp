"""Versioned static-prop interchange and native Property compiler (no game writes)."""
from pathlib import Path
import json,math,hashlib,struct,gzip
SCHEMA='sc5.prop-instances/1'
CATALOG_ROOT=Path(__file__).resolve().parent/'prop_assets'
def catalog(root=None):
    return json.loads(((Path(root) if root else CATALOG_ROOT)/'catalog.json').read_text(encoding='utf-8'))
def validate(doc,cat=None):
    cat=cat or catalog()
    if doc.get('schema')!=SCHEMA:raise ValueError('不支持的道具导出版本')
    if doc.get('unit')!='meter' or doc.get('upAxis')!='+Z' or doc.get('space')!='building-local':raise ValueError('道具必须使用与楼体相同的米制 Z 向上局部坐标')
    objects=doc.get('instances')
    if not isinstance(objects,list) or len(objects)>1000:raise ValueError('道具数量无效（每个资产最多 1000 个）')
    ids=set()
    for item in objects:
        if not isinstance(item,dict) or item.get('preset') not in cat['presets']:raise ValueError('含有未支持的道具预设')
        uid=item.get('id')
        if not isinstance(uid,str) or not uid or uid in ids:raise ValueError('道具标识为空或重复')
        ids.add(uid);matrix=item.get('matrix');scale=item.get('scale')
        if not isinstance(matrix,list) or len(matrix)!=12 or not all(isinstance(x,(int,float)) and math.isfinite(x) for x in matrix):raise ValueError('道具变换矩阵无效')
        if not isinstance(scale,(int,float)) or not math.isfinite(scale) or not .001<=scale<=100:raise ValueError('道具缩放超出支持范围')
        if any(abs(x)>100000 for x in matrix[9:]):raise ValueError('道具位置超出支持范围')
        rows=[matrix[i:i+3] for i in (0,3,6)]
        for i in range(3):
            for j in range(3):
                if abs(sum(a*b for a,b in zip(rows[i],rows[j]))-(1 if i==j else 0))>1e-4:raise ValueError('道具含剪切或非均匀缩放，请使用统一缩放')
        a,b,c=rows;det=a[0]*(b[1]*c[2]-b[2]*c[1])-a[1]*(b[0]*c[2]-b[2]*c[0])+a[2]*(b[0]*c[1]-b[1]*c[0])
        if det<.999:raise ValueError('不支持镜像道具，请应用正向旋转和统一缩放')
    return doc

def read_manifest(model,diagnostic=None):
    path=Path(model).with_suffix('.props.json');declared=(diagnostic or {}).get('propManifest')
    if not path.is_file():
        if declared:raise ValueError('缺少本次导出的道具清单：'+path.name+'；请完整复制插件导出的文件')
        return {'schema':SCHEMA,'unit':'meter','upAxis':'+Z','space':'building-local','instances':[]},None
    if path.stat().st_size>4*1024*1024:raise ValueError('道具清单超过 4 MiB')
    data=path.read_bytes();doc=validate(json.loads(data))
    if declared:
        if declared.get('file')!=path.name or declared.get('sha256')!=hashlib.sha256(data).hexdigest() or declared.get('count')!=len(doc['instances']):raise ValueError('道具清单与楼体导出不匹配，请重新完整导出')
    elif doc['instances']:raise ValueError('楼体未声明此道具清单；请使用新版插件重新导出，避免拼错资产')
    return doc,path

def rotate_matrix(matrix,degrees):
    from asset_transforms import rotation_degrees
    angle=rotation_degrees(degrees);c=round(math.cos(math.radians(angle)));s=round(math.sin(math.radians(angle)))
    # Stored S rows, acting as S-transpose on column vectors.
    out=[]
    for i in (0,3,6):x,y,z=matrix[i:i+3];out.extend([c*x-s*y,s*x+c*y,z])
    x,y,z=matrix[9:];out.extend([c*x-s*y,s*x+c*y,z]);return out

def compile_fields(doc,degrees,native):
    from inspect_park_route import parse
    cat=catalog();validate(doc,cat);groups={};evidence=[]
    for item in doc['instances']:groups.setdefault(item['preset'],[]).append(item)
    fields={};exported=[]
    def array(h,typ,payload,count,size):return struct.pack('>IHHII',h,typ,0x809c,count,size)+payload
    for preset,objects in sorted(groups.items()):
        p=cat['presets'][preset];rid=int(p['resourceId'],16)
        candidates=[(0xb1b104,g,rid) for g in (0x40e0c100,0x40e1c100)]
        source=next((k for k in candidates if k in native),None)
        if source is None:raise ValueError('游戏资源缺少道具：'+p['label'])
        raw=native[source].read();record=parse(raw);signature=record.get('0D8C29C3' if p['tree'] else '00F9EFBB',{}).get('value')
        if p['tree']:
            expected=[0xb1b104,0x40e02d00,int(p['wrapper'],16)]
            if signature not in (expected,[0,*expected[1:]]):raise ValueError('树木资源版本与预设不一致：'+p['label'])
            dependencies=[expected,(0xb1b104,0x40002d00,0xc602cd31),(0xb1b104,0x40002d00,0x5f804d7e)]
        else:
            expected=[int(x,16) for x in p['nativeModel'].split(':')]
            if signature!=expected:raise ValueError('静态道具资源版本与预设不一致：'+p['label'])
            dependencies=[]
        dependencies.append(tuple(int(x,16) for x in p['nativeModel'].split(':')))
        for dependency in dependencies:
            if tuple(dependency) not in native:raise ValueError('道具依赖缺失：'+':'.join(f'{x:08X}' for x in dependency))
        model_key=tuple(int(x,16) for x in p['nativeModel'].split(':'))
        if hashlib.sha256(native[model_key].read()).hexdigest()!=p['modelSha256']:raise ValueError('道具模型已被修改，与预览库不一致：'+p['label'])
        bin_=p['preferredBin'];n=len(objects)
        fields[0x0c12ef20+bin_]=array(0x0c12ef20+bin_,32,struct.pack('>III',rid,0,0),1,12)
        transforms=[]
        for item in objects:
            m=rotate_matrix(item['matrix'],degrees);transforms.append(struct.pack('>H13f',15,item['scale'],*m));exported.append({'id':item['id'],'preset':preset,'bin':bin_,'resourceId':p['resourceId'],'matrix':m,'scale':item['scale']})
        fields[0x0c12ef30+bin_]=array(0x0c12ef30+bin_,56,b''.join(transforms),n,56)
        fields[0x0c12ef40+bin_]=array(0x0c12ef40+bin_,9,struct.pack('>'+'i'*n,*range(n)),n,4)
        # Static display slots: no percentage-driven population/resource fill.
        for base in (0x0c12ef50,0x0c12ef60):fields[base+bin_]=struct.pack('>IHHB',base+bin_,1,0x8000,0)
        evidence.append({'preset':preset,'count':n,'resourceProperty':':'.join(f'{x:08X}' for x in source),'resourceSha256':hashlib.sha256(raw).hexdigest(),'model':p['nativeModel'],'dependencies':[':'.join(f'{x:08X}' for x in d) for d in dependencies]})
    return fields,{'schema':SCHEMA,'count':len(exported),'instances':exported,'resources':evidence,'scope':'static-original-props','runtimeVerified':False}

def preview_instances(doc,root=None):
    import numpy as np
    from PIL import Image
    base=Path(root) if root else CATALOG_ROOT;cat=catalog(base);validate(doc,cat);templates={};instances=[];low=[];high=[]
    for item in doc['instances']:
        key=item['preset'];p=cat['presets'][key]
        if key not in templates:
            d=json.loads(gzip.decompress((base/p['previewMesh']).read_bytes()))
            templates[key]={'positions':np.asarray(d['positions'],np.float32),'normals':np.asarray(d['normals'],np.float32),'uv':np.asarray(d['uv'],np.float32),'indices':np.asarray(d['indices'],np.uint32),'image':np.array(Image.open(base/p['image']).convert('RGBA'))[::-1].copy(),'alphaCutoff':d['alphaCutoff']}
        m=np.eye(4,dtype=np.float32);m[:3,:3]=np.array(item['matrix'][:9],np.float32).reshape(3,3).T*item['scale'];m[:3,3]=item['matrix'][9:];instances.append({'preset':key,'matrix':m,'id':item['id']})
        lo,hi=np.array(p['bounds']);corners=np.array([[x,y,z,1] for x in (lo[0],hi[0]) for y in (lo[1],hi[1]) for z in (lo[2],hi[2])]);points=(corners@m.T)[:,:3];low.append(points.min(axis=0));high.append(points.max(axis=0))
    return templates,instances,(np.min(low,axis=0),np.max(high,axis=0)) if low else None
