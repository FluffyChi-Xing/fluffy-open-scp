"""Native point/spot attachments, independent from room emission and static props."""
from pathlib import Path
import json,hashlib,math,struct
from prop_contract import rotate_matrix
SCHEMA='sc5.light-instances/1'
ROOT=Path(__file__).resolve().parent
GROUP=0x8e43f125  # Owned symbolic group, explicitly bound in both tables.

def validate(doc):
    if not isinstance(doc,dict) or doc.get('schema')!=SCHEMA or doc.get('unit')!='meter' or doc.get('upAxis')!='+Z' or doc.get('space')!='building-local':raise ValueError('灯光清单格式或坐标约定不受支持')
    instances=doc.get('instances')
    if not isinstance(instances,list) or len(instances)>128:raise ValueError('每个资产最多支持 128 个装饰光源')
    ids=set()
    for item in instances:
        if not isinstance(item,dict) or item.get('type') not in ('POINT','SPOT'):raise ValueError('首版只支持点光和聚光')
        if not isinstance(item.get('id'),str) or not item['id'] or item['id'] in ids:raise ValueError('灯光标识为空或重复')
        ids.add(item['id'])
        m=item.get('matrix');color=item.get('color')
        if not isinstance(m,list) or len(m)!=12 or not all(type(x) in (float,int) and math.isfinite(x) for x in m):raise ValueError('灯光变换矩阵无效')
        rows=[m[i:i+3] for i in (0,3,6)]
        for i in range(3):
            for j in range(3):
                if abs(sum(a*b for a,b in zip(rows[i],rows[j]))-(1 if i==j else 0))>1e-4:raise ValueError('灯光旋转矩阵存在缩放或剪切')
        a,b,c=rows
        if a[0]*(b[1]*c[2]-b[2]*c[1])-a[1]*(b[0]*c[2]-b[2]*c[0])+a[2]*(b[0]*c[1]-b[1]*c[0])<.999:raise ValueError('不支持镜像灯光')
        if any(abs(x)>100000 for x in m[9:]):raise ValueError('灯光位置超出范围')
        if not isinstance(color,list) or len(color)!=3 or not all(type(x) in (float,int) and math.isfinite(x) and 0<=x<=1 for x in color):raise ValueError('灯光颜色无效')
        for field,lo,hi in [('radius',.05,500),('innerRadius',0,500),('length',.05,500),('intensity',0,50),('specular',0,20)]:
            v=item.get(field)
            if type(v) not in (int,float) or not math.isfinite(v) or not lo<=v<=hi:raise ValueError('灯光参数超出范围：'+field)
        if item['innerRadius']>item['radius']:raise ValueError('灯光内半径不能大于外半径')
        if item.get('control')!='NATIVE_NIGHT':raise ValueError('尚未支持此灯光控制配方')
    return doc

def read_manifest(model,diagnostic=None):
    path=Path(model).with_suffix('.lights.json');decl=(diagnostic or {}).get('lightManifest')
    if not path.is_file():
        if decl:raise ValueError('缺少灯光清单：'+path.name)
        return {'schema':SCHEMA,'unit':'meter','upAxis':'+Z','space':'building-local','instances':[]},None
    if path.stat().st_size>1024*1024:raise ValueError('灯光清单超过 1 MiB')
    data=path.read_bytes();doc=validate(json.loads(data))
    if decl:
        if decl.get('file')!=path.name or decl.get('sha256')!=hashlib.sha256(data).hexdigest() or decl.get('count')!=len(doc['instances']):raise ValueError('灯光清单与楼体不是同一次导出，请完整重新导出')
    elif doc['instances']:raise ValueError('楼体未声明灯光清单，请使用新版插件导出')
    return doc,path

def compile_fields(doc,degrees):
    validate(doc);items=doc['instances']
    if not items:return {},{'count':0,'runtimeVerified':False}
    profile=json.loads((ROOT/'light_native_profile.json').read_text(encoding='utf-8'));fields={};exported=[]
    def value(t,v):
        if t==32:return struct.pack('>III',v[2],v[0],v[1])
        if t==56:return struct.pack('>H12f',14,*v)
        if t==13:return struct.pack('>f',v)
        if t==1:return bytes([bool(v)])
        if t in (48,50):return struct.pack('>'+'f'*len(v),*v)
        if t==18:
            b=v.encode('utf-8');return struct.pack('>I',len(b))+b
        raise ValueError(t)
    def array(h,t,vals,size):return struct.pack('>IHHII',h,t,0x809c,len(vals),size)+b''.join(value(t,v) for v in vals)
    columns={int(h,16):[] for h in profile['fields']}
    for item in items:
        m=rotate_matrix(item['matrix'],degrees)
        row={0x0caa8f10:[0,0,GROUP],0x0caa8f11:[0,0,0x75d4c8cd if item['type']=='POINT' else 0x2f0ff9fd],0x0caa8f12:m,0x0caa8f13:item['color'],0x0caa8f14:item['radius'],0x0caa8f15:item['innerRadius'],0x0caa8f16:item['intensity'],0x0caa8f17:'SC5_'+item['id'][:24],0x0d4c96b4:item['specular'],0x0d4cad23:item['length'],0x0e01200f:[0,0,profile['cullTier']],0x0e4df244:0.,0x0e98d445:False,0x0ec4567e:0.}
        for h,v in row.items():columns[h].append(v)
        exported.append({**item,'matrix':m})
    for text,meta in profile['fields'].items():
        h=int(text,16);fields[h]=array(h,meta['type'],columns[h],meta['size'])
    # Own group uses measured native night-time range. No guessed power/occupancy flags.
    for h,t,vals,size in [(0x0ce71b10,32,[[0,0,GROUP]],12),(0x0ce71b11,32,[[0,0,profile['nightPeriod']]],12),(0x0ce71b12,48,[profile['nightRange']],8),(0x0ce71b13,48,[[0.,0.]],8)]:fields[h]=array(h,t,vals,size)
    return fields,{'schema':SCHEMA,'count':len(items),'instances':exported,'group':f'{GROUP:08X}','nightRange':profile['nightRange'],'profileSource':profile['source'],'powerControlVerified':False,'runtimeVerified':False}
