from pathlib import Path
import sys,json
root=Path(__file__).resolve().parent;sys.path.insert(0,str(root/'engine'))
from menu_locations import MENU_LOCATIONS,menu_record,append_menu_position
from package_io import index
from build_candidate import prop_bytes
from inspect_park_route import parse
GAME=Path('D:/E/gameecg/SimCity：Cites of Tomorrow/SimCityData')
native={}
for name in ('SimCity_Game.package','SimCity_App.package','SimCity_Graphics.package'):
 for e in index(GAME/name):
  if e.key[0]==0xb1b104 and e.key[1]==0x09878a01:native[e.key]=e
results=[]
for key in MENU_LOCATIONS:
 try:
  record,evidence=menu_record(native,key)
  menu={0x0db9fc63:record}
  placement=append_menu_position(menu,{k:[entry] for k,entry in native.items()})
  readback=parse(prop_bytes(menu))
  assert len(readback['0DB9FC63']['value'])==len(readback['0DC1E3E0']['value'])
  evidence['placement']=placement
  assert readback['0DB9FC63']['value']==evidence['exportedCategoryKeys']
  results.append({'ok':True,**evidence})
 except Exception as exc:results.append({'key':key,'ok':False,'error':str(exc)})
out=root/'release/0.5.0';out.mkdir(parents=True,exist_ok=True)
(out/'native-menu-audit.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(results,ensure_ascii=False,indent=2))
assert all(r['ok'] for r in results)
