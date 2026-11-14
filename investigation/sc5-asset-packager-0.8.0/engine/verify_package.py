"""Offline candidate checks and corruption rejection; never accesses game files."""
import json,struct,io,sys
from pathlib import Path
from PIL import Image
from package_io import index
from inspect_park_route import parse
from rw4_bridge import RW4,require
from collision_proxy import Collision
from native_property_codec import validate_native_profile
from material_internal_refs import validate_model as validate_material_internals
from build_candidate import OUT,write,doc,sha
if len(sys.argv)>1:
    import build_candidate
    candidate_root=Path(sys.argv[1]).resolve()
    require(candidate_root.is_relative_to(build_candidate.OUTPUT_ROOT) and not candidate_root.is_relative_to(build_candidate.GAME),'Candidate directory must stay in workspace')
    OUT=candidate_root;build_candidate.OUT=OUT

def key(text):return tuple(int(x,16) for x in text.split(':'))
report=json.loads((OUT/'候选包验证报告.json').read_text(encoding='utf8'))
roles=report['identities'];package=OUT/report['package'];original=package.read_bytes()

def validate(path):
    resources={e.key:e.read() for e in index(path)}
    require(len(resources)==report['resourceCount'],'Unexpected resource count')
    require({k[0] for k in resources}=={0xb1b104,0x2f4e681b,0x2f4e681c,0x2f7d0004,0x3f8662ea,0x0a98eaf0},'Unexpected executable/script resource')
    props={k:validate_native_profile(b) for k,b in resources.items() if k[0]==0xb1b104}
    menu=props[key(roles['menu'])];unit=props[key(roles['unit'])];asset=props[key(roles['asset'])]
    require(menu['0975695F']['value']==[0,0,key(roles['unit'])[2]],'Wrong menuUnit')
    require(unit['08E17ED7']['value']==list(key(roles['asset'])),'Wrong unit asset')
    for i,h in enumerate(range(0xf9efbb,0xf9efbf)):
        require(asset[f'{h:08X}']['value']==list(key(roles['lods'][i])),'Wrong LOD reference')
    require(asset['0CCB7FD5']['value']==list(key(roles['mask'])),'Wrong ground reference')
    strings=json.loads(resources[key(roles['locale'])].decode('utf-8-sig'))
    require(resources[key(roles['locale'])].startswith(b'\xef\xbb\xbf'),'Locale BOM missing')
    if report.get('authoredMaterial'):require(key(roles['locale'])[1]==0x02fabf01,'Custom locale must use native locale group')
    for fields in props.values():
        for field_hash,field in fields.items():
            if field['type']!=34:continue
            raw=bytes.fromhex(field['recordHex'])
            require(field['flags']&0x30 and struct.unpack_from('>I',raw,12)[0]==520,'Text array item_size must be 520')
            for text in field['value']:
                if field_hash in ('0A09F5FA','0A09F5FB','09FB78CB'):
                    require(text['table']==key(roles['locale'])[2],'Own title/description points to a native table')
                if text['table']==key(roles['locale'])[2]:
                    require(f"0x{text['id']:08x}" in strings,'Unresolved own text')
    for k,b in resources.items():
        if k[0] in (0x2f7d0004,0x3f8662ea):
            im=Image.open(io.BytesIO(b));im.load()
            require(im.size==((128,128) if k[0]==0x2f7d0004 else (454,263)),'Bad UI image size')
        elif k[0]==0x2f4e681b:
            r=RW4(b)
            if r.file_type==1:
                validate_material_internals(r)
                r.decode();s=r.of_type(0x80003)[0];c=Collision(s.payload,s.pos);c.box_proxy()
                expected=asset['00F9EFBA']['value'][0]
                require(all(abs(a-b)<1e-5 for a,b in zip(c.low+c.high,expected)),'Wrong collision bounds')
                points,_=r.decode();require(all(expected[j]-1e-5<=p[j]<=expected[j+3]+1e-5 for p in points for j in range(3)),'Visible geometry outside bounds')
                require(not r.of_type(0x70001) and not r.of_type(0x7000c),'Unexpected animation/skeleton')
            else:
                require(r.file_type==0x04000000 and len(r.of_type(0x20003))==1,'Unknown texture wrapper')
                fmt,_,__,w,h,mips,___,____,blob=struct.unpack_from('<IIIHHIIII',r.of_type(0x20003)[0].payload)
                require(fmt in (21,116) and w>0 and h>0 and mips>>8==1,'Unexpected authored texture encoding')
                require(len(r.sections[blob].payload)==w*h*(16 if fmt==116 else 4),'Texture data size mismatch')
    for k,raw in resources.items():
        if k[0]!=0x2f4e681c or k==key(roles['mask']):continue
        version,w,h,mips,kind,fmt=struct.unpack_from('>6I',raw);require(version==2 and fmt==21 and kind==8,'Bad material Raster format')
        p=24
        for _ in range(mips):
            n=struct.unpack_from('>I',raw,p)[0];p+=4;require(n==w*h*4 and p+n<=len(raw),'Invalid material mip payload');p+=n;w=max(1,w//2);h=max(1,h//2)
        require(p==len(raw) and w==h==1,'Incomplete material mip chain')
    raw=resources[key(roles['mask'])];version,w,h,mips,kind,fmt=struct.unpack_from('>6I',raw)
    require(version==2 and w==h and 16<=w<=2048 and (w&(w-1))==0 and mips==w.bit_length() and kind==8 and fmt==21,'Bad ground raster header')
    p=24;n=w
    for _ in range(mips):
        length=struct.unpack_from('>I',raw,p)[0];p+=4
        require(length==n*n*4 and p+length<=len(raw),'Invalid raster mip')
        if not report.get('propertyProfile'):require(not any(raw[p:p+length]),'Ground selectors unexpectedly contain albedo pixels')
        p+=length;n//=2
    require(p==len(raw),'Raster trailing bytes')
    return resources


validate(package)
print("PACKAGE_VALIDATED")
