"""Materialize generic schemas, then author an independent park and parcel.
No concrete native park Unit/Lot is used as parent or copied layout.
"""
from pathlib import Path
import struct,json
import numpy as np
from PIL import Image
from build_candidate import PROP,key,array,vector,floatprop,intprop,cloned,parse,require,tgi
from authored_materials import raster_rgba
ROOT=Path(__file__).resolve().parent;OUT=ROOT/'独立资产研究0.6.0'

def make_profile(native,model_bounds):
    sources=[]
    def flat(start):
        records={};seen=set();k=start
        while True:
            require(k in native and k not in seen,'Missing/cyclic generic schema');seen.add(k)
            fields=cloned(native[k].read());sources.append(tgi(k))
            for h,b in fields.items():
                if h!=0x00b2cccb:records.setdefault(h,b)
            v=parse(native[k].read());parent=v.get('00B2CCCB',{}).get('value')
            if not parent:break
            k=tuple(parent)
        return records
    # Park-class behavior is generic: do not use 565B72B9 or concrete 59292C11.
    unit=flat((PROP,0x48e0c000,0x34ce7e14))
    # Core building graphics defaults, excluding park's prop/time/state layout.
    asset=flat((PROP,0x40e1c000,0x9ec474ff))
    menu=flat((PROP,0x48e1eb00,0xf970a822))
    menu[0x0db9fc63]=array(0x0db9fc63,32,[struct.pack('>3I',0x07049250,0,0)],12)
    unit[0x0a8a41da]=floatprop(0x0a8a41da,250)
    unit[0x09ae19d7]=floatprop(0x09ae19d7,5)
    width,depth=24.,20.
    for hexfield,value in [('0BBBCDED',width),('0BBBCE2A',depth)]:
        h=int(hexfield,16);asset[h]=floatprop(h,value)
    asset[0x0ccb7fc8]=vector(0x0ccb7fc8,(width,depth))
    asset[0x0ccb7fc9]=vector(0x0ccb7fc9,(0,0))
    asset[0x0fe01484]=array(0x0fe01484,57,[struct.pack('>6f',-width/2,-depth/2,0,width/2,depth/2,model_bounds[5])],24,0x801c)
    # Authored layer colours/indices: grass base + square-paved apron/path.
    for i in range(4):
        asset[0x0d02d586+i]=vector(0x0d02d586+i,(.75,.74,.68,1 if i==0 else 8),52)
        asset[0x0d7af042+i]=vector(0x0d7af042+i,(.55,.54,.48,0),52)
        asset[0x0d7af046+i]=floatprop(0x0d7af046+i,.006)
    asset[0x0d02d58a]=vector(0x0d02d58a,(0,0,0,0),51)
    asset[0x0d02d58b]=vector(0x0d02d58b,(.1,0,0,0),51)
    n=128;y,x=np.indices((n,n));wx=((x+.5)/n-.5)*width;wy=((y+.5)/n-.5)*depth
    paved=((np.abs(wx)<8)&(np.abs(wy)<6))|(np.abs(wx)<1.25)
    rgba=np.zeros((n,n,4),np.uint8);rgba[:,:,2]=paved.astype(np.uint8)*255
    # RGB selectors, not a painted diffuse image. B drives layer 1.
    mask=raster_rgba(rgba)
    picture=np.zeros((n,n,3),np.uint8);picture[:]=[78,107,52];picture[paved]=[179,179,165]
    picture[(np.abs(wx)<6)&(np.abs(wy)<4)]=[145,67,49]
    OUT.mkdir(exist_ok=True);Image.fromarray(picture).resize((512,512),Image.Resampling.NEAREST).save(OUT/'自有地块布局示意.png')
    manifest={'profile':'independent-parent-free-park-v1','runtimeParentCount':0,'genericSources':sorted(set(sources)),
        'concreteSourceUnitRemoved':'00B1B104:48E0C000:565B72B9','concreteSourceLotRemoved':'00B1B104:40E1C000:59292C11',
        'parcelMeters':[width,depth],'cost':250,'maintenancePerHour':5,'baseGroundTile':8,'firstLayerTile':1,
        'groundMask':'authored grass with paved apron and central path; native shared material atlas reused',
        'props':0,'paths':0,'spawners':0,'nativeAlternateStateMappings':0,
        'scope':'Own Menu/Unit/graphics dictionaries and parcel. Generic engine/park defaults are materialized; does not reimplement simulation, shader library or all shared materials.'}
    (OUT/'独立资产设计.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
    return {'menu':menu,'unit':unit,'asset':asset,'mask':mask,'baseTile':8,'manifest':manifest}
