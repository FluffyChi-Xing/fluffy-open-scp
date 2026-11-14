"""0.7: add generic interaction/query configuration, retain independent layout."""
from pathlib import Path
import json
import independent_park_profile as base
from build_candidate import PROP,cloned,parse,tgi,require
ROOT=Path(__file__).resolve().parent;OUT=ROOT/'交互朝向研究0.7.0'
UI_FIELDS=[0x0a8e881d,0x0a8e881e,0x0cabb2cf,0x0cabb2fc,0x0cafbdc3,0x0cd4e49c,0x0d7eadac,0x0e544c70,0x0e7a6fde,0x0d7970ed,0x0e8ba0c6,0x0e8ba0c7,0x0ca52005]

def make_profile(native,bounds):
    old=base.OUT
    try:
        base.OUT=OUT;p=base.make_profile(native,bounds)
    finally:base.OUT=old
    fields={};sources=[];k=(PROP,0x40e1c000,0x34ce7e14)
    while k in native:
        v=parse(native[k].read(),True);sources.append(tgi(k))
        for h,f in v.items():fields.setdefault(int(h,16),bytes.fromhex(f['recordHex']))
        par=v.get('00B2CCCB',{}).get('value')
        if not par:break
        k=tuple(par)
    require(fields[0x0ad90138]==bytes.fromhex('0ad901380001800001'),'Generic interaction flag profile changed')
    for h in [0x0ad90138,0x0afe2feb,*UI_FIELDS]:
        if h in fields:p['asset'][h]=fields[h]
    p['manifest'].update(profile='independent-interactive-park-v2',interactionFlag='0AD90138=true',interactionFlagOrigin='generic templatetoy graphics, no concrete park layout',restoredGenericFields=[f'{h:08X}' for h in [0x0ad90138,0x0afe2feb,*UI_FIELDS] if h in fields],interactionSources=sources,frontConvention='model front/door = +Y, road-facing local edge; geometry normalized by a Z-axis half turn',runtimeParentCount=0)
    (OUT/'独立资产设计.json').write_text(json.dumps(p['manifest'],ensure_ascii=False,indent=2),encoding='utf8')
    return p
