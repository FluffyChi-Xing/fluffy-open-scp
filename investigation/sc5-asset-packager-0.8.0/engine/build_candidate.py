"""Build a standalone park candidate entirely in the workspace. Never installs.

All game paths are opened for reading only. All writes pass the output-root guard.
Candidate remains gameReady=false until actual runtime validation.
"""
from pathlib import Path
import io,json,struct,hashlib,sys
from PIL import Image,ImageOps,ImageDraw,ImageFont
from package_io import index,package_bytes
from inspect_park_route import parse
from rw4_bridge import RW4,require
from collision_proxy import Collision

ROOT=Path(__file__).resolve().parent
OUT=(ROOT/'第三阶段候选包').resolve()
OUTPUT_ROOT=ROOT
GAME=Path('D:/E/gameecg/SimCity：Cites of Tomorrow').resolve()
PROP=0xb1b104;MODEL=0x2f4e681b;RASTER=0x2f4e681c

def write(name,data):
    p=(OUT/name).resolve()
    require(p.is_relative_to(OUT) and not p.is_relative_to(GAME),'Output path escaped workspace')
    p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);return p

def doc(name,value):return write(name,json.dumps(value,ensure_ascii=False,indent=2).encode('utf8'))
def sha(b):return hashlib.sha256(b).hexdigest()
def tgi(k):return ':'.join(f'{x:08X}' for x in k)
def game_snapshot():
    return {str(p.relative_to(GAME)):[p.stat().st_size,p.stat().st_mtime_ns] for p in GAME.rglob('*') if p.is_file()}

def scalar(h,typ,payload):return struct.pack('>IHH',h,typ,0x8000)+payload
def array(h,typ,items,item_size,flags=0x809c):
    return struct.pack('>IHHII',h,typ,flags,len(items),item_size)+b''.join(items)
def key(h,k):return scalar(h,32,struct.pack('>3I',k[2],k[0],k[1]))
def text(h,table,ident):return array(h,34,[struct.pack('>II',table,ident)],520,0x801c)
def vector(h,values,typ=48):return scalar(h,typ,struct.pack('>'+'f'*len(values),*values))
def floatprop(h,value):return scalar(h,13,struct.pack('>f',value))
def intprop(h,value):return scalar(h,9,struct.pack('>i',value))
def cloned(data):return {int(h,16):bytes.fromhex(v['recordHex']) for h,v in parse(data,True).items()}
def prop_bytes(records):
    from native_property_codec import validate_native_profile
    b=struct.pack('>I',len(records))+b''.join(v for _,v in sorted(records.items()));validate_native_profile(b);return b

def raster(n=128):
    # All four selectors zero: use the asset's base material, not white albedo.
    mips=[]
    while n:
        raw=b'\0'*(n*n*4);mips.append(struct.pack('>I',len(raw))+raw);n//=2
    return struct.pack('>6I',2,128,128,len(mips),8,21)+b''.join(mips)

def main(model_path=None,revision='v1',output_dir=None,title='SC5 公园小屋',package_name='SC5_ParkHouse_candidate.DO_NOT_INSTALL.package',append_menu=False,material_factory=None,locale_group=0x02fabf01,preview_path=None,description=None,asset_bool_overrides=None,property_profile_factory=None,menu_icon_path=None):
    global OUT
    if output_dir is not None:OUT=Path(output_dir).resolve()
    require(OUT.is_relative_to(OUTPUT_ROOT) and not OUT.is_relative_to(GAME),'Unsafe project output location')
    require(Path(package_name).name==package_name and package_name.endswith('.package'),'Invalid package output name')
    before=game_snapshot()
    paths=sorted(GAME.rglob('*.package'));inventory={};instance_ids=set();scan_errors=[];duplicate_source_entries=0;indexed_packages=0
    for path in paths:
        try:
            local_seen=set()
            for e in index(path,allow_duplicates=True):
                if (*e.key,e.instance_high) in local_seen:duplicate_source_entries+=1
                local_seen.add((*e.key,e.instance_high))
                instance_ids.add(e.key[2])
                if e.instance_high==0:inventory.setdefault(e.key,[]).append(e)
            indexed_packages+=1
        except (ValueError,struct.error,OSError) as exc:scan_errors.append({'path':str(path),'error':str(exc)})
    eco=max((GAME/'SimCityUserData/EcoGame').glob('SimCity-Scripts_*.package'),key=lambda p:int(p.stem.split('_')[-1]))
    selected=[GAME/'SimCityData/SimCity_Game.package',GAME/'SimCityData/SimCity_App.package',GAME/'SimCityData/SimCity_Graphics.package',eco]
    native={};hashes={}
    for path in selected:
        hashes[str(path)]=sha(path.read_bytes())
        for e in index(path):native[e.key]=e
    def original(k):return native[k].read()
    allocated=set()
    def allocate(label):
        for attempt in range(10000):
            data=('sc5_asset_editor/park_house_candidate/'+revision+'/'+label+('/'+str(attempt) if attempt else '')).encode()
            v=0x811c9dc5
            for c in data:v=((v*0x01000193)^c)&0xffffffff
            if v not in instance_ids and v not in allocated:allocated.add(v);return v
        raise ValueError('Cannot allocate unique identity')
    main_id=allocate('identity');lods=[allocate('lod'+str(i)) for i in range(4)];mask_id=allocate('ground-mask');table_id=allocate('locale')
    menu_key=(PROP,0x09878a01,main_id);unit_key=(PROP,0x40e0c000,main_id);asset_key=(PROP,0x40e1c000,main_id)
    icon_key=(0x2f7d0004,0x40e02400,main_id);image_key=(0x3f8662ea,0,main_id);locale_key=(0x0a98eaf0,locale_group,table_id);mask_key=(RASTER,0,mask_id)
    resources={};changes={}
    input_model=Path(model_path or ROOT/'第二阶段/静态小屋_匹配碰撞.experimental.rw4').read_bytes()
    input_points,_=RW4(input_model).decode()
    model_bounds=[min(p[j] for p in input_points) for j in range(3)]+[max(p[j] for p in input_points) for j in range(3)]
    menu_source=(PROP,0x09878a01,0xac8a698b);unit_source=(PROP,0x40e0c000,0x59292c11);asset_source=(PROP,0x40e1c000,0x59292c11)
    menu=cloned(original(menu_source));unit=cloned(original(unit_source));asset=cloned(original(asset_source))
    profile=None
    if property_profile_factory:
        profile=property_profile_factory(native,model_bounds)
        menu,unit,asset=(profile[k] for k in ('menu','unit','asset'))
    menu_placement={'policy':'copied-native-position'}
    if append_menu:
        from menu_locations import append_menu_position
        menu_placement=append_menu_position(menu,inventory)
    menu[0x0975695f]=key(0x0975695f,(0,0,main_id))
    for h in (0x0977aa8f,0x0c65d729,*range(0x09756955,0x0975695b)):menu[h]=key(h,icon_key)
    menu[0x0ddeee56]=key(0x0ddeee56,image_key)
    menu[0x0a09f5fa]=text(0x0a09f5fa,table_id,1);menu[0x0a09f5fb]=text(0x0a09f5fb,table_id,2)
    unit[0x08e17ed7]=key(0x08e17ed7,asset_key)
    unit[0x09fb78cb]=text(0x09fb78cb,table_id,1)
    # Keep original native base-park behavior, gates, fees and footprint.
    asset[0x09fb78cb]=text(0x09fb78cb,table_id,1)
    for field,value in (asset_bool_overrides or {}).items():
        require(field in (0x0f169c46,0x0e0be186) and isinstance(value,bool),'Unsupported asset boolean override')
        asset[field]=scalar(field,1,bytes([int(value)]))
    for i,h in enumerate(range(0x00f9efbb,0x00f9efbf)):asset[h]=key(h,(MODEL,0,lods[i]))
    asset[0x00f9efba]=array(0x00f9efba,57,[struct.pack('>6f',*model_bounds)],24,0x801c)
    # State mappings may reference the old live asset; map that entry only.
    decoded=parse(original(asset_source))
    h=0x0ca5cf4a
    if not profile and f'{h:08X}' in decoded:
        vals=decoded[f'{h:08X}']['value'];changed=[]
        for k in vals:
            changed.append(asset_key if k[1]==asset_source[1] and k[2]==asset_source[2] else k)
        asset[h]=array(h,32,[struct.pack('>3I',k[2],k[0],k[1]) for k in changed],12)
    asset[0x0ccb7fd5]=key(0x0ccb7fd5,mask_key)
    asset[0x0ccb7fd4]=key(0x0ccb7fd4,(MODEL,0,0xa0da9e6c))
    asset[0x0d0082f1]=key(0x0d0082f1,(MODEL,0,0x60e7805d))
    asset[0x0ccb7fd6]=intprop(0x0ccb7fd6,profile.get('baseTile',1) if profile else 1)
    asset[0x0ccb7fd0]=vector(0x0ccb7fd0,profile.get('period',(8,8)) if profile else (8,8))
    # Native 56x48 m parcel and its attachments remain as an explicit prototype constraint.
    for k,records,source in ((menu_key,menu,menu_source),(unit_key,unit,unit_source),(asset_key,asset,asset_source)):
        resources[k]=prop_bytes(records)
        if profile:changes[tgi(k)]={'construction':'fresh property dictionary from materialized generic rules and explicit owned fields','genericRuleSources':profile['manifest']['genericSources']}
        else:
            old=cloned(original(source));changes[tgi(k)]={'source':tgi(source),'changedFields':[f'{h:08X}' for h in sorted(records) if records[h]!=old.get(h)],'retainedFields':[f'{h:08X}' for h in sorted(old) if records.get(h)==old[h]]}
    model=input_model;material_info=None
    if material_factory:
        model,extra,material_info=material_factory(model,allocate,native)
        require(not set(extra)&set(resources),'Material resource identity collision');resources.update(extra)
    rw=RW4(model);rw.decode()
    from material_internal_refs import validate_model as validate_material_internals
    validate_material_internals(rw)
    cc=rw.of_type(0x80003)[0];Collision(cc.payload,cc.pos).box_proxy()
    require(not rw.of_type(0x70001) and not rw.of_type(0x7000c),'Expected static model')
    lod_models=(material_info or {}).get('lodModels',[model]*4)
    require(len(lod_models)==4,'lodModels must contain four models')
    for ident,lod_model in zip(lods,lod_models):resources[(MODEL,0,ident)]=lod_model
    resources[mask_key]=profile['mask'] if profile else raster()
    strings={'0x00000001':title,'0x00000002':description or '自制公园建筑测试资产，沿用基础公园规则。尚未完成游戏内验证。','0xffffffff':'0xffffffff'}
    body=json.dumps(strings,ensure_ascii=False,indent=4).replace('\n','\r\n')
    resources[locale_key]=b'\xef\xbb\xbf '+body.encode('utf8')
    picture=Image.open(preview_path or ROOT/'实验输出/RW4回读几何预览.png').convert('RGB')
    icon=ImageOps.fit(picture,(128,128),centering=(.5,.45));draw=ImageDraw.Draw(icon)
    draw.rectangle((0,106,128,128),fill=(30,85,52));draw.text((8,109),profile.get('iconLabel','SC5 PARK') if profile else 'SC5 PARK',fill='white')
    if menu_icon_path:
        from menu_icons import icon_png
        icon=Image.open(io.BytesIO(icon_png(menu_icon_path)))
    buf=io.BytesIO();icon.save(buf,format='PNG');resources[icon_key]=buf.getvalue();write('预览/菜单图标.png',buf.getvalue())
    preview=ImageOps.fit(picture,(454,263));buf=io.BytesIO();preview.save(buf,format='JPEG',quality=92);resources[image_key]=buf.getvalue();write('预览/菜单大图.jpg',buf.getvalue())
    collisions=set(resources)&set(inventory);require(not collisions,'Candidate would override an existing TGI')
    artifact=package_bytes(resources);path=write(package_name,artifact)
    reread={e.key:e.read() for e in index(path)};require(reread==resources,'Package payload roundtrip mismatch')
    # Existing separate DBPF reader checks the generated package independently.
    sys.path.insert(0,str(ROOT.parents[1]/'纽约立面插件'))
    from extract_game_ground_1_22 import entries,refpack
    other={}
    for k,data,off,stored,size,comp in entries(path):other[k]=refpack(data[off:off+stored],size) if comp else data[off:off+size]
    require(other==resources,'Independent package-reader mismatch')
    for k,b in resources.items():
        if k[0]==PROP:parse(b)
        if k[0]==MODEL:
            rr=RW4(b)
            if rr.file_type==1:rr.decode()
    # Enumerate key dependencies; distinguish typed resources from runtime symbols.
    refs=[]
    for k,b in resources.items():
        if k[0]!=PROP:continue
        for h,p in parse(b).items():
            if p['type']!=32:continue
            vals=p['value'] if p['flags']&0x30 else [p['value']]
            for ref in vals:
                ref=tuple(ref)
                if ref[2]==0:status='null'
                elif ref in resources:status='candidate-resource'
                elif ref in inventory:status='installed-resource'
                elif ref[0]==0 and ref[1]!=0 and (PROP,ref[1],ref[2]) in resources:status='candidate-property-with-implied-type'
                elif ref[0]==0 and ref[1]!=0 and (PROP,ref[1],ref[2]) in inventory:status='installed-property-with-implied-type'
                elif ref[0]==ref[1]==0:status='runtime-symbol-or-contextual-reference'
                else:status='unresolved'
                if h=='0975695F' and ref==(0,0,main_id):status='candidate-unit-by-menu-semantics'
                refs.append({'owner':tgi(k),'field':h,'target':tgi(ref),'status':status})
    # Main entry chain and mandatory native parents/atlases must resolve.
    required=[(menu_key,'0975695F',unit_key),(unit_key,'08E17ED7',asset_key)]
    for i,h in enumerate(range(0x00f9efbb,0x00f9efbf)):required.append((asset_key,f'{h:08X}',(MODEL,0,lods[i])))
    required.append((asset_key,'0CCB7FD5',mask_key))
    for owner,h,target in required:
        value=parse(resources[owner])[h]['value']
        require(value[2]==target[2] and target in resources,'Broken owned resource chain')
    for owner in (menu_key,unit_key,asset_key):
        fields=parse(resources[owner])
        if profile:require('00B2CCCB' not in fields,'Independent profile must not retain an external parent')
        else:
            parent=tuple(fields['00B2CCCB']['value']);require(parent in inventory,'Missing native parent')
    require((MODEL,0,0xa0da9e6c) in inventory and (MODEL,0,0x60e7805d) in inventory,'Missing native ground atlases')
    bounds=parse(resources[asset_key])['00F9EFBA']['value'][0]
    require(bounds==model_bounds,'Model bounds mismatch')
    fees=parse(original((PROP,0x48e0c000,0x565b72b9)))
    if profile:fees=parse(resources[unit_key])
    native_fees={'construction':fees['0A8A41DA']['value'],'maintenancePerHour':fees['09AE19D7']['value'],'source':'own explicit Unit fields' if profile else '48E0C000:565B72B9'}
    # Inspect shader-def at the known Float36 material location; no new texture bindings.
    material=rw.of_type(0x2000b)[0].payload;vf=rw.of_type(0x20004)[0].size
    at=32+vf;marker,kind,shader=struct.unpack_from('<3I',material,at)
    require(marker==45,'Unexpected static shader layout')
    shader_resources=[tgi(k) for k in inventory if k[2]==shader]
    texture_dependencies=[]
    for material_section in rw.of_type(0x2000b):
        material=material_section.payload
        require(len(material)>=at+7*24,'Truncated native material')
        records=[struct.unpack_from('<6I',material,at+i*24) for i in range(7)]
        if [r[0]&255 for r in records]==[45,0,1,2,3,4,5]:
            for record in records[1:]:
                ident=record[2];keys=((MODEL,0,ident),(RASTER,0,ident));matches=[tgi(k) for k in keys if k in inventory];own=[tgi(k) for k in keys if k in resources]
                texture_dependencies.append({'slot':record[0],'instance':f'{ident:08X}','matchingInstalledResources':matches,'matchingCandidateResources':own,'resolved':bool(matches or own) or ident==0})
    after=game_snapshot();metadata_unchanged=before==after
    source_hashes_unchanged=all(sha(Path(p).read_bytes())==digest for p,digest in hashes.items())
    require(metadata_unchanged and source_hashes_unchanged,'Game installation changed during build; stop and investigate')
    report={'schema':'sc5.offline-park-candidate/1','gameReady':False,'installed':False,'gameStarted':False,
        'package':path.name,'bytes':len(artifact),'sha256':sha(artifact),'resourceCount':len(resources),
        'identities':{'menu':tgi(menu_key),'unit':tgi(unit_key),'asset':tgi(asset_key),'lods':[tgi((MODEL,0,i)) for i in lods],'mask':tgi(mask_key),'locale':tgi(locale_key)},'displayName':title,'menuPlacement':menu_placement,
        'resources':[{'tgi':tgi(k),'bytes':len(v),'sha256':sha(v)} for k,v in sorted(resources.items())],
        'sourceProperties':changes,'sourcePackageHashes':hashes,'scriptSelection':str(eco),
        'checks':{'candidatePayloadRoundtrip':True,'independentPackageReader':True,'mainReferenceChain':True,'fourLODReferences':True,'matchingCollision':True,'nativeParentsAvailable':True,'existingTGICollisionsInIndexedResources':0,'scannedPackageFiles':len(paths),'indexedPackageFiles':indexed_packages,'indexedResourceIdentities':len(inventory),'duplicateSourceIndexEntriesReadWithoutModification':duplicate_source_entries,'indexScanErrors':scan_errors,'gameFileMetadataUnchanged':metadata_unchanged,'gameFilesObserved':len(before),'fourSourcePackageHashesUnchanged':source_hashes_unchanged},
        'staticShader':{'instance':f'{shader:08X}','matchingInstalledResources':shader_resources},'rw4TextureDependencies':texture_dependencies,'authoredMaterial':({k:v for k,v in (material_info or {}).items() if k!='lodModels'} if material_info else material_info),
        'dependencies':refs,'unresolvedTypedReferences':[r for r in refs if r['status']=='unresolved'],
        'limitations':['No runtime verification; never installed','Highest-numbered local EcoGame script selected for analysis; active runtime version not verified','Original park footprint 56x48m, native attachments and state mappings retained except live self-reference','All four LODs currently use the same simple house mesh',f'Native shader {shader:08X} runtime selection remains unverified; absence from resource index is not proof of a missing shader','Native material retained; custom six-slot/window material compilation pending','Runtime symbols and contextual references are not proven resolved by index scan','Inherited park behavior and fees not tested; land value and happiness effects unverified','Non-DBPF .package cache containers excluded from resource-identity checks; see indexScanErrors'],
        'inheritedFees':native_fees}
    if material_info:
        report['limitations']=[x for x in report['limitations'] if x!='Native material retained; custom six-slot/window material compilation pending']
        report['limitations'].append('Authored material profile: interiorProjectionEnabled='+str(material_info.get('interiorProjectionEnabled',False))+'; nightEmissionEnabled='+str(material_info.get('nightEmissionEnabled',False))+'; new profile appearance requires user game test')
    if profile:
        report['propertyProfile']=profile['manifest']
        report['checks'].pop('nativeParentsAvailable',None)
        report['checks']['parentFreeSchemasValidated']=True
        report['checks']['genericSchemaSourcesResolved']=True
        report['limitations']=[x for x in report['limitations'] if not x.startswith('Original park footprint') and not x.startswith('Inherited park behavior')]
        report['limitations'].append('Independent parcel and parent-free property dictionaries; generic park rules materialized, runtime registration and behavior await user test')
    doc('候选包验证报告.json',report)
    doc('游戏目录只读核验.json',{'beforeFileCount':len(before),'afterFileCount':len(after),'metadataUnchanged':metadata_unchanged,'selectedSourceHashesUnchanged':source_hashes_unchanged,'sourceHashes':hashes,'note':'Metadata of all observed game files plus SHA-256 of four selected source packages; not a whole-install byte hash'})
    # Export payloads for inspection; these stay exclusively under the guarded workspace.
    for k,b in resources.items():write('资源/'+tgi(k).replace(':','_')+('.rw4' if k[0]==MODEL else '.bin'),b)
    print(json.dumps({k:report[k] for k in ('package','bytes','resourceCount','sha256','checks','unresolvedTypedReferences','staticShader')},ensure_ascii=True,indent=2))

if __name__=='__main__':main()
