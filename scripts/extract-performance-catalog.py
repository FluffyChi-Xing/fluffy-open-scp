"""Read stock packages and generate the identity/anchor catalog; never deploys.
Run with Blender Python (or Python 3): --game-data PATH [--output PATH].
RCI samples use names containing W<wealth>D<density>; an absent sample is reported,
not substituted with another density. Parent properties are resolved read-only.
"""
import sys,json,re,struct,hashlib,sqlite3,argparse
from pathlib import Path
root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root/'investigation/sc5-asset-packager-0.8.0/engine'))
from package_io import index
from inspect_park_route import parse
args=sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else sys.argv[1:]
parser=argparse.ArgumentParser()
parser.add_argument('--game-data',default=r'D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData')
parser.add_argument('--output',default=str(root/'src-tauri/resources/sc5/performance-catalog.json'))
options=parser.parse_args(args)
connection=sqlite3.connect('file:'+str(root/'src-tauri/resources/database_main.s3db')+'?mode=ro',uri=True)
names=dict(connection.execute('select id,name from Instances'))
files={};entries={}
for package in ['SimCity_Game.package','SimCity_Graphics.package','SimCity_DLC0.package']:
 for entry in index(Path(options.game_data)/package):
  if entry.key[0]!=0x00b1b104:continue
  entries[entry.key]=entry
  try:files[entry.key]=parse(entry.read(),True)
  except ValueError:pass
def resolve(k,seen=()):
 if k in seen:return {}
 f=files.get(tuple(k),{});out={};p=f.get('00B2CCCB',{}).get('value',[])
 if p and isinstance(p[0],int):p=[p]
 for parent in p:
  if isinstance(parent,list) and len(parent)==3:out.update(resolve(tuple(parent),seen+(k,)))
 out.update(f);return out
def val(f,h):
 v=f.get(h,{}).get('value',[])
 return v if isinstance(v,list) else []
def matrix(f,index):
 b=bytes.fromhex(f['0E1BAC62']['recordHex']);pos=16
 for _ in range(index+1):
  flags=struct.unpack_from('>H',b,pos)[0];pos+=2
  if flags==15:pos+=4
  m=list(struct.unpack_from('>12f',b,pos));pos+=48
 return m
samples=[('willis',(0x00b1b104,0x40e1c000,0x8d007745)),('elementary',(0x00b1b104,0x40e1c000,0xc264a268)),('highSchool',(0x00b1b104,0x40e1c000,0x64f750e2)),('college',(0x00b1b104,0x40e1c000,0xa826fa3e))]
for letter,category in [('R','residential'),('C','commercial'),('I','industrial')]:
 for density in [1,2,3]:
  candidates=[]
  for k in files:
   name=names.get(k[2],'')
   if (k[1]&0xffff)==0xc000 and re.match(fr'^{letter}_Gen_W\dD{density}_.*_N$',name,re.I):
    f=resolve(k)
    if val(f,'0D5A28B8'):candidates.append((name,k))
  if candidates:samples.append((f'{category}{density}',sorted(candidates)[0][1]))
  else:print('NO SAMPLE',category,density)
result=[];evidence=[]
for category,k in samples:
 f=resolve(k);loc=val(f,'0E1BAC61');ids=val(f,'0D2E8A55');defs=val(f,'0D5A28B8');sinks=val(f,'0D50908D')
 evidence.append({'category':category,'lot':[f'{v:08X}' for v in k],'name':names.get(k[2],''),'sha256':hashlib.sha256(entries[k].read()).hexdigest(),'vignettes':ids,'definitions':defs})
 for ix,(ident,definition) in enumerate(zip(ids,defs)):
  definition=tuple(definition);d=resolve(definition);used=set()
  for h in ['0D802215','0D9E4D6E','0D801143','0D36410D']:
   used.update(v[2] for v in val(d,h) if isinstance(v,list) and len(v)==3)
  anchors=[]
  for index,key in enumerate(loc):
   if key[2] in used:
    anchors.append({'id':key[2],'matrix':matrix(f,index)})
  result.append({'id':f'{category}-{ident[2]:08X}','category':category,'sourceName':names.get(k[2],''),'sourceLot':list(k),'sourceSha256':hashlib.sha256(entries[k].read()).hexdigest(),'vignetteId':ident,'definition':list(definition),'sink':sinks[ix] if ix<len(sinks) else [0,0,0x2ca33bdb],'anchors':anchors})
print([(r['id'],f"{r['definition'][2]:08X}",len(r['anchors'])) for r in result])
Path(options.output).write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
