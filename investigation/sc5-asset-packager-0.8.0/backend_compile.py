"""Project-driven LOTM9 compiler. Reads installed resources; never installs output."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import sys
import uuid
from asset_transforms import rotation_degrees, rotate_lot, rotate_xyz


def progress(project, step, label):
    folder=project/'reports';folder.mkdir(exist_ok=True)
    target=folder/'compile-progress.json';temporary=folder/'compile-progress.tmp'
    temporary.write_text(json.dumps({'step':step,'total':9,'label':label},ensure_ascii=False),encoding='utf-8')
    temporary.replace(target)
    print(f'[{step}/9] {label}',flush=True)


def compile_project(project: Path):
    from sc5_packager import load_project, inspect_inputs
    project = project.resolve()
    config = load_project(project)
    settings = config['settings']
    angle = rotation_degrees(settings.get('rotationDegrees',180))
    policy = settings.get('nightEmissionPolicy','night-only')
    if policy not in ('night-only','preserve-source'):
        raise ValueError('未知内景状态策略')
    category = config['category']
    from menu_locations import destination,menu_record
    destination(category)
    import behavior_profiles
    behavior_key=behavior_profiles.selection(settings)
    for field in ('cost', 'maintenancePerHour'):
        value = settings[field]
        if not isinstance(value, (int, float)) or not math.isfinite(value) or not 0 <= value <= 1e9:
            raise ValueError(f'无效费用：{field}')
    if settings.get('geometryStrategy') != 'stable_single_mesh':
        raise ValueError('当前编译器仅支持 stable_single_mesh 几何策略')
    if settings.get('materialStrategy') not in ('stable_baseline', 'compact_2048'):
        raise ValueError('当前编译器尚未实现此材质策略')
    progress(project,1,'检查输入与文件指纹')
    inspection = inspect_inputs(project)
    if inspection['errors'] or inspection['warnings']:
        raise ValueError('输入检查未通过：' + '; '.join(inspection['errors'] + inspection['warnings']))
    paths = [Path(item['path']) if item.get('external') else project / item['path'] for item in inspection['files']]
    models = [p for p in paths if p.suffix.lower() == '.lotm']
    if len(models) != 1:
        raise ValueError('请确保项目中只有一组建筑资产（恰好一个 LOTM）')
    model = models[0]
    ground = model.with_suffix('.lot.package')
    if ground not in paths:
        raise ValueError(f'缺少与 LOTM 同名的地块文件：{ground.name}')
    engine = Path(__file__).resolve().parent / 'engine'
    game = Path(config.get('gameDirectory') or 'D:/E/gameecg/SimCity：Cites of Tomorrow').resolve()
    if project.is_relative_to(game):
        raise ValueError('项目不能放在游戏安装目录内')
    for name in ('SimCity_Game.package', 'SimCity_App.package', 'SimCity_Graphics.package'):
        if not (game / 'SimCityData' / name).is_file():
            raise ValueError('找不到游戏资源，请在 project.json 的 gameDirectory 设置游戏根目录')
    # Fail mismatched native recipes before expensive geometry/material work.
    behavior_check=behavior_profiles.preflight(config)
    (project/'reports'/'behavior-preflight.json').write_text(json.dumps(behavior_check,ensure_ascii=False,indent=2),encoding='utf-8')
    run = project / 'build' / ('run-' + uuid.uuid4().hex[:12])
    run.mkdir(parents=True)
    (project/'reports').mkdir(exist_ok=True)
    snapshot = run / 'input'
    snapshot.mkdir()
    progress(project,2,'保存本次输入快照')
    # Use exact captured bytes throughout conversion; edits during a build do not mix versions.
    for path in paths:
        target = snapshot / path.name
        if target.exists():
            raise ValueError('同名输入文件有歧义：' + path.name)
        shutil.copy2(path, target)
        expected = next(i['sha256'] for i in inspection['files'] if Path(i['path']).name == path.name)
        if hashlib.sha256(target.read_bytes()).hexdigest() != expected:
            raise ValueError('构建过程中输入文件发生改变，请重试：' + path.name)
    menu_icon=None
    if config.get('menuIconImage'):
        from menu_icons import icon_png
        menu_icon=run/'menu-icon.png'
        menu_icon.write_bytes(icon_png(Path(config['menuIconImage'])))
    sys.path.insert(0, str(engine))
    import numpy as np
    from PIL import Image, ImageDraw
    import probe_tower
    import prepare_asset
    import compile_tower_asset as materials
    import build_candidate as builder
    import interactive_park_profile as interaction
    from authored_materials import raster_rgba
    from native_property_codec import LOT_TYPES, encode_lot_numeric
    from package_io import index
    from inspect_park_route import parse
    from rw4_bridge import RW4, require

    # Reject unsupported formats before allocating atlases or reading binary records.
    source = snapshot / model.name
    header = source.read_bytes()[:12]
    require(header[:4] == b'LOTM' and struct.unpack_from('<I', header, 4)[0] == 9, 'Only LOTM9 is supported')
    chunks, mats, _, diagnostic = probe_tower.read(source)
    from prop_contract import read_manifest,compile_fields
    prop_doc,_=read_manifest(source,diagnostic)
    import light_contract
    light_doc,_=light_contract.read_manifest(source,diagnostic)
    prop_fields={}
    require(0 < len(chunks) == len(mats) <= 128, 'Unsupported or empty LOTM material groups')
    for g, b in chunks:
        attrs = g['meshes'][0]['primitives'][0]['attributes']
        require(all(k in attrs for k in ('POSITION', 'NORMAL', 'TANGENT', 'TEXCOORD_0', 'TEXCOORD_1', 'TEXCOORD_3')), 'Missing native UV or tangent attributes')
    prepare_asset.SOURCE = source
    prepare_asset.OUT = run
    progress(project,3,'转换几何、朝向、材质和房间状态')
    prepare_asset.prepare(rotation_degrees=angle,night_only=policy=='night-only')
    progress(project,4,'生成紧凑表面图和数值缩略层')
    # Shrink only surface maps; channel-independent filtering preserves numeric
    # RGB even where alpha is zero. Normalized UVs and relief gain stay intact.
    if settings.get('materialStrategy') == 'compact_2048':
        for name in ('tint', 'normal', 'surface'):
            path = run / (name + '.png')
            a = np.array(Image.open(path).convert('RGBA'))
            if a.shape[0] > 2048 or a.shape[1] > 2048:
                ratio = min(2048/a.shape[0], 2048/a.shape[1])
                size = (int(a.shape[1]*ratio), int(a.shape[0]*ratio))
                filtered = np.stack([np.asarray(Image.fromarray(a[:,:,i]).resize(size, Image.Resampling.BOX)) for i in range(4)], axis=2)
                Image.fromarray(filtered).save(path)
    materials.OUT = run
    builder.GAME = game
    builder.OUTPUT_ROOT = project
    interaction.OUT = run

    def profile(native, bounds):
        p = interaction.make_profile(native, bounds)
        # Preserve the complete, user-tested interaction and lifecycle assembly.
        unit = builder.cloned((engine / 'tested-unit.bin').read_bytes())
        unit,behavior_evidence=behavior_profiles.apply(unit,native,settings)
        p['unit'] = unit
        p['manifest']['behavior']=behavior_evidence
        p['menu'][0x0db9fc63],menu_evidence=menu_record(native,category)
        p['manifest']['menuLocation']=menu_evidence
        fields,prop_evidence=compile_fields(prop_doc,angle,native)
        prop_fields.update(fields);p['asset'].update(fields);p['manifest']['props']=prop_evidence
        light_fields,light_evidence=light_contract.compile_fields(light_doc,angle)
        prop_fields.update(light_fields);p['asset'].update(light_fields);p['manifest']['lights']=light_evidence
        resources = {e.key: e.read() for e in index(snapshot / ground.name)}
        lots = [parse(b, True) for k, b in resources.items() if k[0] == builder.PROP]
        require(len(lots) == 1, 'Expected one ground Property')
        lot = lots[0]
        for h, field in lot.items():
            if int(h, 16) in LOT_TYPES:
                p['asset'][int(h, 16)] = encode_lot_numeric(int(h, 16), field)
            else:
                require(h in ('0CCB7FD4', '0CCB7FD5', '0D0082F1'), 'Unsupported source ground property ' + h)
        width, depth = lot['0CCB7FC8']['value']
        require(width > 0 and depth > 0, 'Invalid parcel dimensions')
        raw = resources[tuple(lot['0CCB7FD5']['value'])]
        version, w, h, mips, kind, fmt = struct.unpack_from('>6I', raw)
        require(version == 2 and w == h and 16 <= w <= 2048 and w & (w-1) == 0 and fmt == 21 and kind == 8, 'Unsupported ground Raster')
        require(struct.unpack_from('>I', raw, 24)[0] == w*h*4 and len(raw) >= 28+w*h*4, 'Truncated ground raster')
        mask = np.frombuffer(raw[28:28+w*h*4], np.uint8).reshape(h,w,4)[:,:,[2,1,0,3]]
        mask,size,offset,period=rotate_lot(mask,[width,depth],lot['0CCB7FC9']['value'],lot['0CCB7FD0']['value'],angle)
        width,depth=size
        p['asset'][0x0ccb7fc8]=builder.vector(0x0ccb7fc8,size)
        p['asset'][0x0ccb7fc9]=builder.vector(0x0ccb7fc9,offset)
        p['asset'][0x0bbbcded]=builder.floatprop(0x0bbbcded,width)
        p['asset'][0x0bbbce2a]=builder.floatprop(0x0bbbce2a,depth)
        p['asset'][0x0fe01484]=builder.array(0x0fe01484,57,[struct.pack('>6f',offset[0]-width/2,offset[1]-depth/2,0,offset[0]+width/2,offset[1]+depth/2,bounds[5])],24,0x801c)
        p['mask'] = raster_rgba(mask)
        p['baseTile'] = lot['0CCB7FD6']['value']
        p['period'] = period
        p['iconLabel'] = 'SC5 '+category.upper()[:11]
        p['manifest'].update(profile='project-native-display-v1', parcelMeters=[width,depth],
            cost=settings['cost'], maintenancePerHour=settings['maintenancePerHour'],
            menuCategory=category, sourceGround=ground.name,
            simulation=('Native park behavior recipe; runtime effects require game test' if behavior_key==behavior_profiles.PARK else 'Tested park lifecycle retained; no school capacity or custom service claim'), groundRotationDegrees=angle)
        progress(project,6,'地块与单位属性已生成，正在打包资源')
        return p

    def factory(model_data, allocate, native):
        result, resources, info = materials.material_factory(model_data, allocate, native)
        converted = json.loads((run / '几何材质转换.json').read_text(encoding='utf-8'))
        info.update(profile='project-native-single-v1', materialRows=len(mats), sourceGroups=len(mats),
                    parameterWidth=converted['parameterWidth'])
        return result, resources, info

    previews = [p for p in paths if p.suffix.lower() == '.png' and not p.name.endswith('.lotmask.png')]
    if previews:
        preview = snapshot / previews[0].name
    else:
        preview = run / 'preview.png'
        im = Image.new('RGB', (454,263), '#23344b')
        ImageDraw.Draw(im).text((40,120), config.get('nameEn') or 'SC5 ASSET', fill='white')
        im.save(preview)
    name = ''.join(c if c.isascii() and (c.isalnum() or c in '-_') else '_' for c in config.get('nameEn','SC5_Asset')) or 'SC5_Asset'
    version = ''.join(c if c.isalnum() or c in '.-_' else '_' for c in config['version'])
    output_name = f'{name}_{version}.package'
    progress(project,5,'索引原生依赖并生成自有菜单')
    behavior_revision='' if behavior_key==behavior_profiles.LEGACY else '|'+behavior_key
    builder.main(run/'希望大厦_单网格兼容.rw4', revision='project-v04-'+hashlib.sha256((str(project)+str(angle)+config['version']+behavior_revision).encode()).hexdigest()[:20],
        output_dir=run/'candidate', title=config.get('nameEn') or name, description=config.get('description') or ('Custom park building. Native park behavior recipe.' if behavior_key==behavior_profiles.PARK else 'Custom display building. No school services.'),
        package_name=output_name, append_menu=True, material_factory=factory, preview_path=preview, menu_icon_path=menu_icon,
        asset_bool_overrides={0x0f169c46:True}, property_profile_factory=profile)
    report = json.loads((run/'candidate/候选包验证报告.json').read_text(encoding='utf-8'))
    package = run/'candidate'/output_name
    progress(project,7,'回读 Package、引用、材质与碰撞')
    import runpy
    saved_argv = sys.argv
    try:
        sys.argv = ['verify_package.py', str(run/'candidate')]
        runpy.run_path(str(engine/'verify_package.py'))
    finally:
        sys.argv = saved_argv
    built = {e.key: e.read() for e in index(package)}
    key = lambda s: tuple(int(x,16) for x in s.split(':'))
    if menu_icon:
        icon_resources=[v for k,v in built.items() if k[0]==0x2f7d0004 and k[1]==0x40e02400]
        require(menu_icon.read_bytes() in icon_resources,'Custom menu icon roundtrip mismatch')
    emitted_asset=parse(built[key(report['identities']['asset'])],True)
    for h,record in prop_fields.items():
        require(emitted_asset[f'{h:08X}']['recordHex'].lower()==record.hex(),'Prop field roundtrip mismatch '+f'{h:08X}')
    unit = parse(built[key(report['identities']['unit'])],True)
    emitted_menu=parse(built[key(report['identities']['menu'])])
    menu_evidence=report['propertyProfile']['menuLocation']
    expected_category=[0,0,destination(category)['categoryInstance']]
    require(expected_category in emitted_menu['0DB9FC63']['value'],'Menu location roundtrip mismatch')
    require(emitted_menu['0DB9FC63']['value']==menu_evidence['exportedCategoryKeys'],'Exported menu memberships changed')
    require(len(emitted_menu['0DB9FC63']['value'])==len(emitted_menu['0DC1E3E0']['value']),'Menu membership and order counts differ')
    behavior_profiles.verify(unit,report['propertyProfile']['behavior'])
    require(unit['0A8A41DA']['value'] == float(settings['cost']), 'Cost roundtrip mismatch')
    require(unit['09AE19D7']['value'] == float(settings['maintenancePerHour']), 'Maintenance roundtrip mismatch')
    model_back = RW4(built[key(report['identities']['lods'][0])])
    expected_triangles = sum(g['accessors'][g['meshes'][0]['primitives'][0]['indices']]['count']//3 for g,b in chunks)
    require(len(model_back.decode()[1]) == expected_triangles, 'Triangle loss')
    require(len(model_back.of_type(0x20009)) == 1, 'Expected stable single mesh')
    require(not report['unresolvedTypedReferences'], 'Unresolved typed resource dependencies')
    # Verify the emitted interior atlas itself, rather than only the pre-pack image.
    stats=json.loads((run/'材质采样变换.json').read_text(encoding='utf-8'))
    interior=RW4(built[key(report['authoredMaterial']['slots'][5]['tgi'])])
    section=interior.of_type(0x20003)[0].payload
    _,_,_,iw,ih,_,_,_,blob=struct.unpack_from('<IIIHHIIII',section)
    interior_pixels=np.frombuffer(interior.sections[blob].payload,np.uint8).reshape(ih,iw,4)[:,:,[2,1,0,3]]
    require(np.array_equal(interior_pixels,np.array(Image.open(run/'interior.png').convert('RGBA'))),'Packed room pixels differ from converted source')
    evidence=[]
    for item in stats:
        if not item['nightPolicyApplied']:continue
        strip=interior_pixels[item['roomStrip']*64:(item['roomStrip']+1)*64,:1024]
        for state in item['roomStates']:
            tile=strip[:,state['state']*64:(state['state']+1)*64]
            require(int(tile[:,:,3].sum())==state['alphaAfter'],'Packed interior emission changed')
            if state['state']%4!=1:require(not tile[:,:,3].any(),'Day/off emission must be zero')
            else:require(state['nightUnchanged'],'Night cell unexpectedly modified')
            require(state['rgbUnchanged'],'Room RGB changed')
        evidence.append({'group':item['group'],'dayAlphaBefore':item['roomStates'][0]['alphaBefore'],
                         'dayAlphaAfter':int(strip[:,:64,3].sum()),'nightPreserved':True})
    progress(project,8,'核对房间状态、朝向、三角形和费用')
    expected_positions=np.concatenate([rotate_xyz(materials.accessor(g,b,g['meshes'][0]['primitives'][0]['attributes']['POSITION']),angle) for g,b in chunks])
    actual_positions=np.array(model_back.decode()[0])
    expected_bounds=np.concatenate([expected_positions.min(axis=0),expected_positions.max(axis=0)])
    actual_bounds=np.concatenate([actual_positions.min(axis=0),actual_positions.max(axis=0)])
    require(np.allclose(expected_bounds,actual_bounds,atol=1e-4,rtol=0),'Export orientation bounds mismatch')
    output = project/'output'
    output.mkdir(exist_ok=True)
    target = output/output_name
    shutil.copy2(package, target)
    result = {'schema':'sc5.project-compile-result/1', 'package':str(target), 'bytes':target.stat().st_size,
              'sha256':hashlib.sha256(target.read_bytes()).hexdigest(), 'sourceFiles':inspection['files'],
              'materialGroups':len(mats), 'triangles':expected_triangles, 'category':category,
              'menuLocation':menu_evidence,'props':report['propertyProfile']['props'],'lights':report['propertyProfile']['lights'],
              'behavior':report['propertyProfile']['behavior'],
              'menuIcon':{'source':config.get('menuIconImage'),'size':[128,128],'sha256':hashlib.sha256(menu_icon.read_bytes()).hexdigest() if menu_icon else None},
              'settings':settings, 'nativeReport':str(run/'candidate/候选包验证报告.json'),
              'rotationDegrees':angle,'nightEmissionPolicy':policy,'roomStateEvidence':evidence,
              'orientationBounds':actual_bounds.tolist(),
              'runtimeVerified':False, 'gameWrites':False}
    (project/'reports'/'native-backend-result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    progress(project,9,'导出及离线核验完成')
    return result


if __name__ == '__main__':
    try:
        print(json.dumps(compile_project(Path(sys.argv[1])), ensure_ascii=False, indent=2))
    except Exception:
        import traceback
        traceback.print_exc()
        raise SystemExit(1)
