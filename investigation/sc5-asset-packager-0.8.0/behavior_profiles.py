"""Versioned native behavior recipes, independent of menu and graphics.

Only the first park recipe is enabled. No scripts, native layouts or global
rules are copied into an output package. Evidence pins are hashes, not assets.
"""
from pathlib import Path
import hashlib
import json
import sys

LEGACY='legacy-display'
PARK='native-park-w1-v1'
PROFILES={LEGACY:'旧版兼容（保留既有行为）',PARK:'基础公园（原生低收入公园规则 · 待实机验证）'}
PROFILE_FILE=Path(__file__).with_name('behavior_native_profile.json')
EFFECT_FIELDS=tuple(range(0x0E965A70,0x0E965A76))+tuple(range(0x0E965BE0,0x0E965BE6))
# Identity, author-owned footprint and localized name stay with the tested
# custom asset. Native unlock conditions and inherited parents are not copied.
PROTECTED=(0x08E17ED7,0x0ACFFA64,0x09FB78CB)
OMIT=(0x00B2CCCB,0x0BF5D2E3,0x0BF5D2E4)

def selection(settings):
    value=settings.get('behaviorProfile',LEGACY)
    if not isinstance(value,str) or value not in PROFILES:raise ValueError('尚未支持的建筑功能预设：'+str(value))
    if 'provideSimulationService' in settings and not isinstance(settings['provideSimulationService'],bool):
        raise ValueError('provideSimulationService 必须为布尔值')
    if settings.get('provideSimulationService') and value==LEGACY:
        raise ValueError('尚未支持此模拟效果配置，请明确选择基础公园功能预设')
    if not isinstance(settings.get('behaviorParameters',{}),dict) or settings.get('behaviorParameters'):
        raise ValueError('本版采用固定原生公园配方，尚未开放自定义幸福度、地价或服务参数')
    return value

def summary(settings):
    key=selection(settings)
    return {'profile':key,'label':PROFILES[key],'runtimeVerified':False,
            'scope':'legacy-behavior-preserved' if key==LEGACY else 'native-park-behavior-recipe',
            'limitations':['菜单位置与功能分别选择','未提供学位、产水或自定义地价效果']}

def engine_imports():
    path=str(Path(__file__).resolve().parent/'engine')
    if path not in sys.path:sys.path.insert(0,path)
    from build_candidate import cloned,parse,prop_bytes,floatprop
    return cloned,parse,prop_bytes,floatprop

def _pin():return json.loads(PROFILE_FILE.read_text(encoding='utf-8'))
def _key(s):return tuple(int(x,16) for x in s.split(':'))
def _sha(b):return hashlib.sha256(b).hexdigest()

def native_index(game):
    from package_io import index
    game=Path(game)
    files=[game/'SimCityData'/n for n in ('SimCity_Game.package','SimCity_App.package','SimCity_Graphics.package')]
    scripts=list((game/'SimCityUserData/EcoGame').glob('SimCity-Scripts_*.package'))
    scripts=[p for p in scripts if p.stem.split('_')[-1].isdigit()]
    if not scripts:raise ValueError('公园功能检查找不到游戏 Scripts 资源包')
    files.append(max(scripts,key=lambda p:int(p.stem.split('_')[-1])))
    native={}
    for path in files:
        if not path.is_file():raise ValueError('功能检查缺少原版资源：'+str(path))
        for entry in index(path):native[entry.key]=entry
    return native,files

def resolve(native):
    cloned,parse,prop_bytes,_=engine_imports();pin=_pin();evidence=[]
    for text,expected in pin['resourceHashes'].items():
        key=_key(text)
        if key not in native:raise ValueError('公园功能依赖缺失：'+text)
        actual=_sha(native[key].read())
        if actual!=expected:raise ValueError('公园功能依据与已研究版本不一致：'+text+'；请先核对游戏版本，不能直接套用配方')
        evidence.append({'tgi':text,'sha256':actual})
    fields={};seen=set();key=_key(pin['sourceUnit']);parents=[]
    while True:
        if key in seen or key not in native:raise ValueError('公园单位存在循环或缺失继承')
        seen.add(key);parents.append(':'.join(f'{v:08X}' for v in key))
        data=native[key].read();decoded=parse(data,True)
        for h,raw in cloned(data).items():fields.setdefault(h,raw)
        parent=decoded.get('00B2CCCB',{}).get('value')
        if not parent:break
        key=tuple(parent)
    if parents!=pin['parentChain']:raise ValueError('公园单位继承链发生变化')
    fields.pop(0x00B2CCCB,None)
    decoded=parse(prop_bytes(fields))
    # No child module or native graphics state is silently transplanted.
    children=decoded.get('07E0AD0A',{}).get('value',[])
    if children:raise ValueError('公园配方含未处理的默认子模块')
    for field in EFFECT_FIELDS:
        name=f'{field:08X}'
        if decoded[name]['value']!=pin['effectValues'][name]:raise ValueError('公园效果专化参数不符合已研究配方：'+name)
    return fields,{'profile':PARK,'label':PROFILES[PARK],'runtimeVerified':False,
        'sourceUnit':pin['sourceUnit'],'sourceParents':parents,'validatedResources':evidence,
        'candidateRuleCount':len(pin['candidateRuleIds']),'ruleCheck':'pinned compiled rule resource; static candidates, not runtime execution',
        'effectPropertyValues':pin['effectValues'],'unitCategory':decoded['0AFE27B1']['value'],
        'nativeDefaults':{'capacityField':decoded['0AD7FAC9']['value'],'jobs':decoded['0A7917C3']['value'],'minimumWorkers':decoded['0A7917C8']['value']},
        'requiresGameTest':['实际幸福度影响','访客与营业状态','停用后作用变化','保存重开与拆除'],
        'sourcePolicy':'installed core resources plus highest numeric Scripts filename; other mod overrides not simulated',
        'globalRulesCopied':False,'nativeLayoutCopied':False}

def preflight(config):
    result=summary(config['settings'])
    if result['profile']==LEGACY:return result
    engine_imports()
    native,files=native_index(config.get('gameDirectory') or 'D:/E/gameecg/SimCity：Cites of Tomorrow')
    _,result=resolve(native)
    result['sourcePackages']=[{'path':str(p),'bytes':p.stat().st_size} for p in files]
    return result

def apply(unit,native,settings):
    key=selection(settings);cloned,parse,prop_bytes,floatprop=engine_imports()
    if key==LEGACY:
        from build_candidate import key as key_record
        unit=dict(unit);unit[0x0AFE27B1]=key_record(0x0AFE27B1,(0,0,0x95A9D6FD))
        evidence=summary(settings)
    else:
        fields,evidence=resolve(native)
        for h in OMIT:fields.pop(h,None)
        for h in PROTECTED:
            if h in unit:fields[h]=unit[h]
            else:fields.pop(h,None)
        unit=fields
    unit[0x0A8A41DA]=floatprop(0x0A8A41DA,settings['cost'])
    unit[0x09AE19D7]=floatprop(0x09AE19D7,settings['maintenancePerHour'])
    evidence['expectedUnitRecords']={f'{h:08X}':raw.hex() for h,raw in unit.items() if h not in (0x08E17ED7,0x09FB78CB)}
    evidence['cost']=settings['cost'];evidence['maintenancePerHour']=settings['maintenancePerHour']
    return unit,evidence

def verify(unit,evidence):
    for h,raw in evidence['expectedUnitRecords'].items():
        if unit.get(h,{}).get('recordHex','').lower()!=raw.lower():raise ValueError('建筑功能字段回读不一致：'+h)
    if evidence['profile']==PARK:
        if '00B2CCCB' in unit:raise ValueError('公园单位不应遗留原版父级引用')
        for h in ('0BF5D2E3','0BF5D2E4'):
            if h in unit:raise ValueError('公园单位遗留原版解锁限制')
