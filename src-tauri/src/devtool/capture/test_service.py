"""Installed capture service integration checks; no game process is launched or controlled."""
from pathlib import Path
import tempfile,subprocess,sys,json,urllib.request,urllib.error,os,shutil
SOURCE=Path(__file__).parent
PAGE='game://localhost/resource/0x40464200!0x82c29d9b.0xdd6233d6'
def run():
 with tempfile.TemporaryDirectory() as temp:
  root=Path(temp);(root/'live').mkdir();(root/'live/session-key').write_text('a'*48)
  (root/'capture.json').write_text(json.dumps({'gameExecutable':sys.executable,'allowedPages':[PAGE]}))
  installed=root/'service';shutil.copytree(SOURCE,installed,ignore=shutil.ignore_patterns('__pycache__'))
  process=subprocess.Popen([sys.executable,'-u',str(installed/'service.py'),'--runtime',str(root),'--port','0','--parent',str(os.getpid())],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   ready=json.loads(process.stdout.readline());port=ready['port'];assert ready['ready']
   def post(data,key='a'*48):
    req=urllib.request.Request(f'http://127.0.0.1:{port}/{key}/demand',data=json.dumps(data).encode())
    try:
     with urllib.request.urlopen(req,timeout=5) as r:return r.status
    except urllib.error.HTTPError as e:return e.code
   frame={'op':'bounds','page':PAGE,'time':100,'sequence':1,'viewport':[400,240], 'nodes':[{'i':0,'parent':-1,'tag':'html'},{'i':1,'parent':0,'tag':'button'}], 'boxes':[{'i':1,'parent':0,'tag':'button','rect':[20,20,150,70]}]}
   assert post(frame)==204
   target=root/f'live/overlay-{os.getpid()}.json';result=json.loads(target.read_text());assert result['nodes']==frame['nodes'] and result['boxes']==frame['boxes']
   assert post(frame,'wrong')==404
   assert post(dict(frame,page='game://localhost/resource/not-allowed'))==403
   bad=dict(frame,viewport=[0,240]);assert post(bad)==400
   assert post(dict(frame,time=50,sequence=0))==204;assert json.loads(target.read_text())['time']==100
   second=subprocess.run([sys.executable,str(installed/'service.py'),'--runtime',str(root),'--port',str(port)],capture_output=True,timeout=10)
   assert second.returncode!=0 and process.poll() is None
   print('PASS installed service: peer identity, validated DOM publication, wrong credential/page/geometry rejection, stale frame retention, port conflict isolation')
  finally:
   process.terminate();process.wait(timeout=10)
if __name__=='__main__':run()
