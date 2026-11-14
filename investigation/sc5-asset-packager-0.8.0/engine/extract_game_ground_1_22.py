"""Read original DBPF/RW4 resources; never modify the installed game."""
import struct,json,zlib,hashlib,io
from pathlib import Path
from PIL import Image,ImageDraw

ROOT=Path(__file__).resolve().parent
GAME=Path('D:/E/gameecg/SimCity：Cites of Tomorrow/SimCityData')
OUT=ROOT/'ny_facade/assets/ground'

def refpack(data,size):
 p=2+(4 if data[0]&128 else 3)*(2 if data[0]&1 else 1);out=bytearray()
 while p<len(data):
  b=data[p];p+=1;stop=False;count=distance=0
  if b>=0xe0:
   if b>=0xfc:literal=b&3;stop=True
   else:literal=((b&31)+1)*4
  elif b>=0xc0:
   x,y,z=data[p:p+3];p+=3;literal=b&3;count=((b&12)<<6|z)+5;distance=(((b&16)<<4|x)<<8|y)+1
  elif b>=0x80:
   x,y=data[p:p+2];p+=2;literal=x>>6;count=(b&63)+4;distance=((x&63)<<8|y)+1
  else:
   x=data[p];p+=1;literal=b&3;count=((b&28)>>2)+3;distance=((b&96)<<3|x)+1
  out.extend(data[p:p+literal]);p+=literal
  if count:
   assert 0<distance<=len(out)
   for _ in range(count):out.append(out[-distance])
  if stop:break
 assert len(out)==size,(len(out),size)
 return bytes(out)

def entries(path):
 data=path.read_bytes();assert data[:4]==b'DBPF';count=struct.unpack_from('<I',data,0x24)[0];p=struct.unpack_from('<I',data,0x40)[0]
 flags=struct.unpack_from('<I',data,p)[0];p+=4;shared={}
 for i in range(3):
  if flags&(1<<i):shared[i]=struct.unpack_from('<I',data,p)[0];p+=4
 for _ in range(count):
  ids=[]
  for i in range(3):
   if i in shared:ids.append(shared[i])
   else:ids.append(struct.unpack_from('<I',data,p)[0]);p+=4
  instance,offset,stored,size,compressed,other=struct.unpack_from('<IIIIhH',data,p);p+=20
  yield (ids[0],ids[1],instance),data,offset,stored&0x7fffffff,size,compressed

def read_resource(instance):
 for name in ('SimCity_Graphics.package','SimCity_App.package','SimCity_Game.package'):
  for key,data,off,stored,size,comp in entries(GAME/name):
   if key[2]==instance and key[0]==0x2f4e681b:
    raw=data[off:off+stored] if comp else data[off:off+size]
    if comp:raw=refpack(raw,size)
    return key,raw,name
 raise ValueError(hex(instance))

def texture(raw):
 assert raw[:4]==b'\x89RW4';count=struct.unpack_from('<I',raw,32)[0]
 idx=struct.unpack_from('<I',raw,48)[0];end=struct.unpack_from('<I',raw,68)[0]
 sections=[]
 for i in range(count):
  pos,_,size,align,ind,kind=struct.unpack_from('<6I',raw,idx+24*i)
  if kind==0x10030:pos+=end
  sections.append((pos,size,kind))
 pos,size,_=next(v for v in sections if v[2]==0x20003)
 fmt,_,unk,w,h,mip,zero,zero2,blob=struct.unpack_from('<IIIHHIIII',raw,pos)
 start,length,kind=sections[blob];assert kind==0x10030;pixels=raw[start:start+length]
 if fmt in (0x31545844,0x35545844):
  head=bytearray(128);head[:4]=b'DDS ';struct.pack_into('<7I',head,4,124,0x81007,h,w,len(pixels),0,1)
  struct.pack_into('<II4s',head,76,32,4,struct.pack('<I',fmt));struct.pack_into('<I',head,108,0x1000)
  im=Image.open(io.BytesIO(bytes(head)+pixels)).convert('RGBA')
 elif fmt==21:im=Image.frombytes('RGBA',(w,h),pixels[:w*h*4],'raw','BGRA')
 else:raise ValueError(fmt)
 return im

if __name__=='__main__':
 manifest={'schema':'openscp.original-ground/1','sources':[],'baseTiles':[],'patterns':[]}
 for role,instance in [('surface',0xa0da9e6c),('normal',0x60e7805d)]:
  key,raw,package=read_resource(instance);(OUT/(role+'.rw4')).write_bytes(raw)
  im=texture(raw)
  if role=='normal':im.putalpha(255)
  im.save(OUT/(role+'.png'))
  manifest['sources'].append({'role':role,'package':package,'tgi':[f'{v:08X}' for v in key],'sha256':hashlib.sha256(raw).hexdigest(),'size':list(im.size)})
  w,h=im.width//4,im.height//4
  sheet=Image.new('RGB',(4*180,4*200),(40,44,50));draw=ImageDraw.Draw(sheet)
  for index in range(16):
   cell=im.crop(((index%4)*w,(index//4)*h,(index%4+1)*w,(index//4+1)*h))
   cell.save(OUT/f'{role}_{index:02d}.png')
   sheet.paste(cell.resize((176,176)),((index%4)*180,(index//4)*200));draw.text(((index%4)*180+5,(index//4)*200+179),f'{role} {index:02d}',fill='white')
   manifest['baseTiles' if role=='surface' else 'patterns'].append({'index':index,'image':f'{role}_{index:02d}.png'})
  sheet.save(OUT/(role+'_contact.png'))
 (OUT/'catalog.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
 print(json.dumps(manifest['sources'],ensure_ascii=False,indent=2))
