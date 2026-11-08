"""Verified native menu destinations. Menu routing is separate from Unit behavior."""
import struct
import hashlib

# key, label, game navigation, uiCategories instance, native Menu donor.
_ROWS = [
 ('education','教育','城市 / 教育',0x95A9D6FD,0x29873FA2),
 ('park','公园','城市 / 公园 / 基础公园',0x07049250,0xAC8A698B),
 ('power','电力','城市 / 电力',0x8E8B4E10,0xBC843E9B),
 ('water','供水','城市 / 供水',0x9E3C3DFA,0x7384728B),
 ('sewage','污水','城市 / 污水',0xCD1DDC35,0xBE957F21),
 ('garbage','垃圾处理','城市 / 垃圾处理',0x7CB23A4E,0x33FAE703),
 ('fire','消防','城市 / 消防',0x9FAC79C9,0x6D0235EE),
 ('health','医疗','城市 / 医疗',0xDB2D51A3,0x05456823),
 ('police','警察','城市 / 警察',0x1159ECD7,0x8E67B653),
 ('government','政府','城市 / 政府',0x674129FD,0x8251CB11),
 ('bus','公共交通 · 公交','城市 / 公共交通 / 公交',0x2A99C945,0x79F3ED13),
 ('streetcar','公共交通 · 有轨电车','城市 / 公共交通 / 有轨电车',0xA63DDC77,0xF0DBD6F2),
 ('rail','公共交通 · 客运铁路','城市 / 公共交通 / 客运铁路',0x4B277752,0xE8792D59),
 ('harbor','公共交通 · 客运码头','城市 / 公共交通 / 客运码头',0xDC8602C9,0x97C5802B),
 ('ferry','公共交通 · 渡轮','城市 / 公共交通 / 渡轮',0x6AFE622F,0xEF12DD01),
 ('airport','公共交通 · 机场','城市 / 公共交通 / 机场',0x3E9C273B,0xB655235F),
 ('landmark','文化 · 地标','城市专精 / 文化 / 地标',0x3F43E038,0xC0E5BE77),
 ('trade','贸易','城市专精 / 贸易',0x74CD2EB0,0x1BE77B24),
 ('mining','采矿','城市专精 / 采矿',0xAFA8707D,0x54B959BA),
 ('oil','石油','城市专精 / 石油',0x3A798ED7,0x47DE4F86),
 ('electronics','电子工业','城市专精 / 电子工业',0xBEE1C73E,0x95434A56),
 ('gambling','博彩','城市专精 / 博彩',0x0BFA0784,0x57FCAD9F),
]
MENU_LOCATIONS={key:{'label':label,'path':path,'categoryInstance':category,'donor':(0x00B1B104,0x09878A01,donor),'runtimeVerified':key in ('education','park')} for key,label,path,category,donor in _ROWS}
CATEGORIES={key:value['label'] for key,value in MENU_LOCATIONS.items()}

def destination(category):
    if category not in MENU_LOCATIONS:
        raise ValueError('不支持的菜单位置：'+str(category)+'。请从已支持的菜单位置中选择；住宅、商业和工业分区暂未接入。')
    return MENU_LOCATIONS[category]

def menu_record(native,category):
    """Validate the installed game's donor, then encode only routing metadata."""
    from inspect_park_route import parse
    from build_candidate import array
    d=destination(category)
    if d['donor'] not in native:raise ValueError('当前游戏缺少菜单位置的原版依据：'+d['label'])
    source_bytes=native[d['donor']].read()
    props=parse(source_bytes,True)
    field=props.get('0DB9FC63')
    if not field:raise ValueError('原版菜单缺少分类归属：'+d['label'])
    keys=field['value']
    if keys and isinstance(keys[0],int):keys=[keys]
    expected=[0,0,d['categoryInstance']]
    if expected not in keys:raise ValueError('当前游戏的菜单归属与已核对资料不一致：'+d['label'])
    # Preserve the existing, game-tested education entry's additional memberships.
    # Every newly introduced location gets exactly one native top-level destination.
    selected=keys if category=='education' else [expected]
    record=array(0x0db9fc63,32,[struct.pack('>3I',key[2],key[0],key[1]) for key in selected],12)
    evidence={'key':category,'label':d['label'],'path':d['path'],
              'nativeDonor':':'.join(f'{v:08X}' for v in d['donor']),
              'nativeCategoryKeys':keys,'exportedCategoryKeys':selected,
              'nativeDonorPackage':str(getattr(native[d['donor']],'path','test-fixture')),
              'nativeDonorSha256':hashlib.sha256(source_bytes).hexdigest(),
              'runtimeVerified':d['runtimeVerified'],'scope':'menu-location-only'}
    return record,evidence


def append_menu_position(menu,inventory):
    from build_candidate import parse,prop_bytes,PROP,require,tgi,array
    source_menu=parse(prop_bytes(menu));categories=source_menu['0DB9FC63']['value'];target_categories={tuple(k) for k in categories}
    positions=[];unreadable=[]
    for k,candidates in inventory.items():
        if k[0]!=PROP or k[1]!=0x09878a01:continue
        for entry in candidates:
            try:
                props=parse(entry.read());cats=props.get('0DB9FC63',{}).get('value',[]);orders=props.get('0DC1E3E0',{}).get('value',[])
                # Native arrays and third-party scalar properties are both valid.
                if cats and isinstance(cats[0],int):cats=[cats]
                if isinstance(orders,int):orders=[orders]
                if not any(tuple(c) in target_categories for c in cats):continue
                require(len(cats)==len(orders),'UI category/position array lengths differ')
                positions.extend(pos for cat,pos in zip(cats,orders) if tuple(cat) in target_categories)
            except (ValueError,TypeError) as exc:unreadable.append({'tgi':tgi(k),'package':str(entry.path),'error':str(exc)})
    require(positions and not unreadable,'无法确定目标菜单的安全追加位置，请检查当前游戏资源及菜单模组')
    new_position=max(positions)+100;require(new_position<2147483647,'Menu order overflow')
    menu[0x0dc1e3e0]=array(0x0dc1e3e0,9,[struct.pack('>i',new_position) for _ in categories],4)
    menu_placement={'policy':'append-after-all-indexed-category-entries','categoryKeys':categories,'maximumExistingPosition':max(positions),'newPosition':new_position,'matchingPositions':len(positions)}
    return menu_placement
