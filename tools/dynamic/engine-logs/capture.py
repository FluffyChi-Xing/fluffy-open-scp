"""Build-bound startup probe for EA::Trace and ArgScript console output.

Does not change log levels or execute game functions. Leaves the spawned game
running after capture; exit it before another experiment.
"""
import argparse,hashlib,json,pathlib,struct,time,threading,subprocess
import frida
root=pathlib.Path(__file__).parent
exe=pathlib.Path(r'D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCity/SimCity.exe')
ap=argparse.ArgumentParser();ap.add_argument('--out',required=True);ap.add_argument('--seconds',type=int,default=35);ap.add_argument('--exe',default=str(exe));args=ap.parse_args()
exe=pathlib.Path(args.exe)
if args.seconds <= 0:raise ValueError('seconds must be positive')
running=subprocess.run(['powershell.exe','-NoProfile','-NonInteractive','-Command',
 '(Get-Process -Name SimCity -ErrorAction SilentlyContinue | Measure-Object).Count'],
 capture_output=True,text=True,check=True,creationflags=subprocess.CREATE_NO_WINDOW)
if int(running.stdout.strip()) != 0:raise RuntimeError('existing_SimCity_process')
out=pathlib.Path(args.out);out.mkdir(parents=True,exist_ok=False)
b=exe.read_bytes();sha=hashlib.sha256(b).hexdigest()
assert sha=='2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab'
pe=struct.unpack_from('<I',b,60)[0];opt=struct.unpack_from('<H',b,pe+20)[0];sections=[]
for i in range(struct.unpack_from('<H',b,pe+6)[0]):
 o=pe+24+opt+40*i;_,va,rs,rp=struct.unpack_from('<4I',b,o+8);sections.append((va,rs,rp))
def read(rva,n):
 for va,rs,rp in sections:
  if va<=rva and rva+n<=va+rs:return b[rp+rva-va:rp+rva-va+n]
 raise ValueError(rva)
hooks={k:{'rva':va-0x400000,'bytes':read(va-0x400000,12).hex()} for k,va in {'formatted':0x8e5630,'serverTrace':0x917380,'fileReporter':0x916370,'consoleReporter':0x46be30,'consoleOutput':0x46b9e0,'getServer':0x915d90}.items()}
cfg={'hooks':hooks};(out/'config.json').write_text(json.dumps(cfg,indent=2),encoding='utf-8')
status={'exe':str(exe),'sha256':sha,'frida':frida.__version__,'startedUtc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'seconds':args.seconds,'spawned':False,'ready':False,'resumed':False,'detached':False,'errors':[]}
ready=threading.Event();gone=threading.Event();lock=threading.Lock()
def message(m,data):
 with lock:
  if m['type']=='send':
   p=m['payload']
   if p['kind']=='ready':status['ready']=p;ready.set()
   if p['kind']=='batch':
    with (out/'trace.jsonl').open('a',encoding='utf-8') as f:
     for row in p['records']:f.write(json.dumps(row,ensure_ascii=False)+'\n')
  else:status['errors'].append(m);ready.set()
device=frida.get_local_device();pid=session=script=None
try:
 pid=device.spawn([str(exe)],cwd=str(exe.parent));status['pid']=pid;status['spawned']=True
 session=device.attach(pid);session.on('detached',lambda *v: (status.update({'detachEvent':str(v)}),gone.set()))
 script=session.create_script((root/'engine_agent.js').read_text(encoding='utf-8').replace('CONFIG',json.dumps(cfg),1),runtime='qjs');script.on('message',message);script.load()
 if not ready.wait(10) or not status['ready']:raise RuntimeError('observer_not_ready')
 device.resume(pid);status['resumed']=True
 print(json.dumps({'pid':pid,'ready':True}),flush=True)
 gone.wait(args.seconds)
 if not gone.is_set():status['summary']=script.exports_sync.stop()
finally:
 if script and not gone.is_set():
  try:script.unload()
  except Exception as e:status['errors'].append(str(e))
 if session and not gone.is_set():
  try:session.detach();status['detached']=True
  except Exception as e:status['errors'].append(str(e))
 if pid and not status['resumed']:
  try:device.kill(pid)
  except Exception:pass
 (out/'status.json').write_text(json.dumps(status,ensure_ascii=False,indent=2),encoding='utf-8')
 print(json.dumps({'statusFile':str(out/'status.json'),'pid':pid,'errors':status['errors']}),flush=True)
