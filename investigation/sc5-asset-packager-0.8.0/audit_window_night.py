"""Audit the universal native day/night rule against the entire local catalog.

Uses source atlas data and the production room-strip function; no Blender or
game process is required and no source image is modified.
"""
from pathlib import Path
import hashlib
import importlib.util
import json
import numpy as np
from PIL import Image
from asset_transforms import room_strip

ROOT=Path(__file__).resolve().parent
PACKS=ROOT.parent/'纽约立面插件/ny_facade/assets/packs'


def audit():
    catalog_path=PACKS/'catalog.json'
    catalog=json.loads(catalog_path.read_text(encoding='utf-8'))
    module_spec=importlib.util.spec_from_file_location('audit_native_controls',ROOT/'engine/native_controls.py')
    controls=importlib.util.module_from_spec(module_spec);module_spec.loader.exec_module(controls)
    images={};sources=[]
    for pack in catalog['packs']:
        path=PACKS/pack/'slot5.png'
        raw=path.read_bytes()
        image=np.array(Image.open(path).convert('RGBA'))
        if pack=='Office':image=controls.office_room_sheet(image)
        images[pack]=image
        sources.append({'pack':pack,'sha256':hashlib.sha256(raw).hexdigest()})

    def check(pack,scale,offset):
        if not np.isfinite([scale,offset]).all() or scale<0:
            raise ValueError(f'Unsupported source room rectangle: {pack} {scale} {offset}')
        if scale==0:return {'interiorEnabled':False,'changedDayEmission':False}
        original,_=room_strip(images[pack],scale,offset,night_only=False)
        fixed,evidence=room_strip(images[pack],scale,offset,night_only=True)
        np.testing.assert_array_equal(fixed[:,:,:3],original[:,:,:3],err_msg=pack+' RGB changed')
        for state in range(16):
            a=fixed[:,state*64:(state+1)*64]
            b=original[:,state*64:(state+1)*64]
            if state%4==1:np.testing.assert_array_equal(a,b,err_msg=pack+' night changed')
            elif a[:,:,3].any():raise AssertionError(pack+' day/off emits')
        # Occupancy must remain respected: dark windows are not forced lit.
        dark=images[pack].copy();dark[:,:,3]=0
        if room_strip(dark,scale,offset)[0][:,:,3].any():raise AssertionError(pack+' dark window became lit')
        return {'interiorEnabled':True,
                'changedDayEmission':any(e['alphaBefore']>0 for e in evidence if e['state']%4!=1),
                'rgbUnchanged':True,'nightUnchanged':True,'allNonNightAlphaZero':True,'darkPreserved':True}

    windows=[]
    for entry in catalog['windows']:
        pack=entry['pack'];row=list(catalog['packs'][pack]['rows'][0][entry['recipe']])
        if 'room_tile' in entry:row[2:4]=entry['room_tile']
        if pack=='Office':row[2:4]=[1/16,0]
        windows.append({'id':entry['id'],'name':entry['name'],'pack':pack,
                        'scale':row[2],'offset':row[3],**check(pack,row[2],row[3])})
    # Covers legacy Commercial fallback row 19 and recipes not currently exposed
    # as named window presets, as well as all named presets above.
    recipes=[]
    for pack,info in catalog['packs'].items():
        for i,row in enumerate(info['rows'][0]):
            scale,offset=(1/16,0) if pack=='Office' else row[2:4]
            recipes.append({'pack':pack,'row':i,**check(pack,scale,offset)})
    result={'schema':'sc5.window-night-catalog-audit/1','compilerVersion':'0.4.0',
            'productionFunction':'asset_transforms.room_strip',
            'catalogSha256':hashlib.sha256(catalog_path.read_bytes()).hexdigest(),
            'packCount':len(images),'namedWindowCount':len(windows),'recipeCount':len(recipes),
            'windowsWithInterior':sum(w['interiorEnabled'] for w in windows),
            'windowsWithNonNightEmissionRemoved':sum(w['changedDayEmission'] for w in windows),
            'allChecksPassed':True,'gameRuntimeVerified':False,'sourceImages':sources,
            'windowResults':windows,'recipeResults':recipes,
            'scope':'Supported native Building40 four-state/16-cell room layout; verifies data emission, not all visual rendering or arbitrary future shader layouts.'}
    target=ROOT/'release/0.4.0/全部窗型昼夜核验.json'
    target.parent.mkdir(exist_ok=True,parents=True)
    target.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({k:result[k] for k in ('packCount','namedWindowCount','recipeCount','windowsWithInterior','windowsWithNonNightEmissionRemoved','allChecksPassed')},ensure_ascii=False))
    return result


if __name__=='__main__':audit()
