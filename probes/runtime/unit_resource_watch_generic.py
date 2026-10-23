"""Generic external read-only UnitEntry resource snapshot watcher."""
import argparse, json, struct, time, hashlib
from datetime import datetime, timezone
from pathlib import Path
from school_resource_watch import Reader, EXE_HASH, vector

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--pid',type=int,required=True); ap.add_argument('--game',type=lambda x:int(x,0),required=True); ap.add_argument('--unit-entry',type=lambda x:int(x,0),required=True); ap.add_argument('--out',type=Path,required=True); ap.add_argument('--seconds',type=int,default=600); ap.add_argument('--ticks',type=int,default=360); args=ap.parse_args()
 args.out.mkdir(parents=True,exist_ok=False); r=Reader(args.pid); st=dict(pid=args.pid,game=hex(args.game),unitEntryId=hex(args.unit_entry),mode='external-read-only',intervalSeconds=.5,requestedSeconds=args.seconds,requestedTicks=args.ticks,samples=0,errors=[],limitations=['non-atomic snapshots','net changes not rule attribution','resource IDs require semantic mapping'])
 try:
  mod=next(m for m in r.modules if m['name'].lower()=='simcity.exe'); st['exeSha256']=hashlib.sha256(Path(mod['path']).read_bytes()).hexdigest()
  if st['exeSha256']!=EXE_HASH: raise ValueError('unsupported_exe')
  scripts=r.u32(args.game+16); st['scripts']=hex(scripts)
  rb,re,_=vector(r,scripts+0x90,8,20000); resource_map=list(struct.iter_unpack('<Ii',r.read(rb,re-rb))); (args.out/'resource-map.json').write_text(json.dumps(resource_map),encoding='utf8')
  first=None; deadline=time.monotonic()+args.seconds
  with (args.out/'snapshots.jsonl').open('w',encoding='utf8') as out:
   while time.monotonic()<deadline:
    tick=r.u32(args.game+0x17a0); ub,ue,h=vector(r,args.game+0x554,0x380,100000); units=[]
    for slot in range((ue-ub)//0x380):
     up=ub+slot*0x380
     if r.u32(up+12)!=args.unit_entry: continue
     bb,be,bh=vector(r,up+0x210,12,65535); bins=[dict(resourceIndex=i,rawLink=l,amount=a,capacity=c) for i,l,a,c in struct.iter_unpack('<hHii',r.read(bb,be-bb))]
     units.append(dict(unitSlot=slot,unitPointer=hex(up),bins=bins))
    if first is None: first=tick; st['firstTick']=tick
    after=r.u32(args.game+0x17a0); row=dict(utc=datetime.now(timezone.utc).isoformat(),tickBefore=tick,tickAfter=after,sameTick=tick==after,stableIdentity=True,units=units)
    out.write(json.dumps(row)+'\n');out.flush();st['samples']+=1;st['lastTick']=after
    if after-first>=args.ticks: st['stopReason']='tick_target_reached';break
    time.sleep(.5)
   else: st['stopReason']='wall_time_limit'
 except Exception as e: st['errors'].append(str(e));st['stopReason']='error'
 finally:
  r.close(); st['finishedUtc']=datetime.now(timezone.utc).isoformat(); (args.out/'status.json').write_text(json.dumps(st,indent=2),encoding='utf8'); print(json.dumps(st))
 return int(bool(st['errors']))
if __name__=='__main__': raise SystemExit(main())
