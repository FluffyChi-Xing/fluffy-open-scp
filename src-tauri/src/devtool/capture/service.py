"""Installed capture service entry point. Protocol implementation is migrated unchanged."""
import argparse,json,os,threading
from pathlib import Path
from receiver import server

def stop_with_parent(pid):
    if not pid:return
    import ctypes
    from ctypes import wintypes
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD]
    kernel.OpenProcess.restype=wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes=[wintypes.HANDLE,wintypes.DWORD]
    kernel.CloseHandle.argtypes=[wintypes.HANDLE]
    handle=kernel.OpenProcess(0x100000,False,pid)
    if not handle:raise RuntimeError('Desktop parent process unavailable')
    def wait():
        kernel.WaitForSingleObject(handle,0xffffffff)
        kernel.CloseHandle(handle)
        os._exit(0)
    threading.Thread(target=wait,daemon=True).start()

def run():
    parser=argparse.ArgumentParser()
    parser.add_argument('--runtime',type=Path,required=True)
    parser.add_argument('--port',type=int,default=47839)
    parser.add_argument('--parent',type=int,default=0)
    args=parser.parse_args()
    config=json.loads((args.runtime/'capture.json').read_text(encoding='utf8'))
    game=Path(config['gameExecutable'])
    if not game.is_file():raise ValueError('Configured game executable is missing')
    live=args.runtime/'live';live.mkdir(parents=True,exist_ok=True)
    if not (live/'session-key').is_file():raise ValueError('Capture credential is not configured')
    pages=config['allowedPages']
    if not pages or not all(isinstance(p,str) and p.startswith('game://localhost/resource/') for p in pages):
        raise ValueError('Invalid capture resource allowlist')
    stop_with_parent(args.parent)
    service=server(live,args.port,game,allowed_pages={p.lower() for p in pages},quiet_legacy=True,exclusive=True)
    print(json.dumps({'ready':True,'port':service.server_port}),flush=True)
    try:service.serve_forever()
    finally:service.server_close()

if __name__=='__main__':run()
