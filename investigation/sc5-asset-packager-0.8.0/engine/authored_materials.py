"""Small authored Building4 material profile: masks, normal, parameters, palette.

Data maps are generated numerically, not baked from display RGB. First profile is
opaque wall/roof/glazing/frame; interior projection and night emission are off.
"""
from pathlib import Path
import struct,json,hashlib
import numpy as np
from rw4_bridge import RW4,require
ROOT=Path(__file__).resolve().parent;OUT=ROOT/'材质测试0.2.0'
COLORS=[(220,202,165),(147,53,38),(35,82,112),(52,57,63)]

def raster_rgba(array):
    from PIL import Image
    a=np.asarray(array,dtype=np.uint8);h,w=a.shape[:2];mips=[]
    while True:
        b=a[:,:,[2,1,0,3]].tobytes();mips.append(struct.pack('>I',len(b))+b)
        ah,aw=a.shape[:2]
        if (aw,ah)==(1,1):break
        nh,nw=max(1,ah//2),max(1,aw//2)
        # All four channels are native numeric fields. RGBA image resampling
        # premultiplies by Alpha and destroys RGB when layer4 weight is zero.
        if ah in (1,nh*2) and aw in (1,nw*2):
            a=np.floor(a.reshape(nh,1 if ah==1 else 2,nw,1 if aw==1 else 2,4).mean(axis=(1,3))).astype(np.uint8)
        else:
            a=np.stack([np.asarray(Image.fromarray(a[:,:,i]).resize((nw,nh),Image.Resampling.BOX)) for i in range(4)],axis=2)
    return struct.pack('>6I',2,w,h,len(mips),8,21)+b''.join(mips)

def texture_rw4(template,pixels,fmt):
    r=RW4(template);require(r.file_type==0x04000000,'Expected native texture wrapper')
    t=r.of_type(0x20003);require(len(t)==1 and len(r.sections)==2,'Unexpected texture layout')
    p=bytearray(t[0].payload);h,w=pixels.shape[:2];require(w<=65535 and h<=65535,'Texture dimensions exceed wrapper')
    struct.pack_into('<I',p,0,fmt);struct.pack_into('<HH',p,12,w,h)
    struct.pack_into('<I',p,16,0x180 if fmt==116 else 0x120)
    blob=struct.unpack_from('<I',p,28)[0]
    data=np.asarray(pixels,dtype='<f4').tobytes() if fmt==116 else np.asarray(pixels,dtype=np.uint8)[:,:,[2,1,0,3]].tobytes()
    require(np.isfinite(pixels).all(),'Non-finite parameter texel')
    r.set(t[0].number,p);r.set(blob,data);result=r.write();back=RW4(result)
    require(back.sections[blob].payload==data,'Texture writeback mismatch');return result

def authored_maps():
    n=256;mask=np.zeros((n,n,4),np.uint8);mask[:,:,2]=128;mask[:,:,3]=255
    normal=np.zeros((n,n,4),np.uint8);normal[:]=[128,128,255,255]
    shader=np.zeros((n,n,4),np.uint8);shader[:]=[23,255,40,255]
    yy,xx=np.indices((128,128))
    # Wall: mortar joints and modest deterministic brightness grain.
    mortar=(yy%24<3)|((xx+(yy//24%2)*16)%32<2)
    wall=mask[:128,:128];wall[:,:,2]=np.where(mortar,86,136+((xx*17+yy*13)%11-5)).astype(np.uint8)
    # Roof: courses and staggered seams, independent of wall pattern.
    tiles=(yy%18<3)|((xx+(yy//18%2)*12)%24<2)
    mask[:128,128:,2]=np.where(tiles,72,140+(yy%18)*2).astype(np.uint8)
    # Glazing: a visible highlight plus the shader's specular channel.
    mask[128:,:128,2]=np.clip(110+xx*.45+yy*.10,0,230).astype(np.uint8)
    shader[128:,:128,2]=220
    # Frame/door: restrained grain, always opaque.
    mask[128:,128:,2]=(122+(yy%7)).astype(np.uint8)
    palette=np.zeros((16,512,4),np.uint8)
    for i,color in enumerate(COLORS):
        palette[:14,i*128:(i+1)*128,:3]=color;palette[:14,i*128:(i+1)*128,3]=120 if i==2 else 35
        palette[14:,i*128:(i+1)*128]=[0,150 if i==2 else 0,0,220 if i==2 else 70]
    params=np.zeros((4,4,4),dtype='<f4')
    for i in range(4):
        rect=[.5,.5,(i%2)*.5,(i//2)*.5]
        params[0,i]=[i*.25,i*.25,0,0]
        params[1,i]=rect;params[2,i]=rect;params[3,i]=[0,0,1,1]
    interior=np.zeros((64,64,4),np.uint8);interior[:,:,:3]=[65,54,43]
    return mask,normal,shader,palette,params,interior

def material_factory(model,allocate,native):
    from PIL import Image
    OUT.mkdir(exist_ok=True)
    mask,normal,shader,palette,params,interior=authored_maps()
    wrapper_float=(OUT/'DCBEA56D.texture-template.rw4').read_bytes();wrapper_rgba=(OUT/'2EF7D20A.texture-template.rw4').read_bytes()
    payloads=[texture_rw4(wrapper_float,params,116),raster_rgba(mask),raster_rgba(normal),raster_rgba(shader),texture_rw4(wrapper_rgba,palette,21),texture_rw4(wrapper_rgba,interior,21)]
    ids=[allocate('authored-material-slot-'+str(i)) for i in range(6)]
    resources={(0x2f4e681b if i in (0,4,5) else 0x2f4e681c,0,ids[i]):b for i,b in enumerate(payloads)}
    rw=RW4(model);mat=rw.of_type(0x2000b)[0];data=bytearray(mat.payload)
    at=32+rw.of_type(0x20004)[0].size
    records=[struct.unpack_from('<6I',data,at+i*24) for i in range(7)]
    require([r[0]&255 for r in records]==[45,0,1,2,3,4,5] and records[0][2]==0x259e950f,'Unexpected Building4 material references')
    for i,ident in enumerate(ids):struct.pack_into('<I',data,at+(i+1)*24+8,ident)
    rw.set(mat.number,data);new_model=rw.write()
    for name,array in [('slot1_tint_mask',mask),('slot2_normal_ao',normal),('slot3_surface',shader),('slot4_palette',palette),('slot5_interior_placeholder',interior)]:
        Image.fromarray(array).save(OUT/(name+'.png'))
    (OUT/'slot0_params.float32').write_bytes(params.tobytes())
    manifest={'profile':'authored-palette-v1','materialRows':4,'rows':['wall','roof','glazing','frame-door'],'colorsSrgb':COLORS,'paramTexture':{'format':116,'width':4,'height':4,'bytes':len(params.tobytes())},'opaque':True,'interiorProjectionEnabled':False,'nightEmissionEnabled':False,
        'slots':[{'slot':i,'tgi':f'{(0x2f4e681b if i in (0,4,5) else 0x2f4e681c):08X}:00000000:{ids[i]:08X}','bytes':len(payloads[i]),'sha256':hashlib.sha256(payloads[i]).hexdigest()} for i in range(6)],
        'notes':['All six textures/parameter resources are authored and included in the package','RGB masks select palette colours; this is not a conventional diffuse RGB image','Glazing is opaque reflective exterior in this experiment; native room/night compilation deferred','Game appearance requires user test']}
    (OUT/'自制材质编译记录.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
    return new_model,resources,manifest
