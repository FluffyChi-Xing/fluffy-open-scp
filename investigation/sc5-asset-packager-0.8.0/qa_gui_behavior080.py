from pathlib import Path
import os,sys,time,json
runtime=Path(sys.executable).parent
os.environ['TCL_LIBRARY']=str(runtime/'tcl/tcl8.6');os.environ['TK_LIBRARY']=str(runtime/'tcl/tk8.6')
from sc5_packager_gui import App
import behavior_profiles as b
from PIL import Image

def capture_window(app,target):
    # Capture only our own application window, even when another app is above it.
    import ctypes as c
    from ctypes import wintypes as w
    user=c.WinDLL('user32');gdi=c.WinDLL('gdi32')
    user.GetAncestor.argtypes=[w.HWND,w.UINT];user.GetAncestor.restype=w.HWND
    user.GetWindowDC.argtypes=[w.HWND];user.GetWindowDC.restype=w.HDC
    user.ReleaseDC.argtypes=[w.HWND,w.HDC]
    gdi.CreateCompatibleDC.argtypes=[w.HDC];gdi.CreateCompatibleDC.restype=w.HDC
    gdi.CreateCompatibleBitmap.argtypes=[w.HDC,c.c_int,c.c_int];gdi.CreateCompatibleBitmap.restype=w.HBITMAP
    gdi.SelectObject.argtypes=[w.HDC,w.HANDLE];gdi.SelectObject.restype=w.HANDLE
    gdi.DeleteObject.argtypes=[w.HANDLE];gdi.DeleteDC.argtypes=[w.HDC]
    user.GetWindowRect.argtypes=[w.HWND,c.POINTER(w.RECT)]
    user.PrintWindow.argtypes=[w.HWND,w.HDC,w.UINT]
    gdi.GetDIBits.argtypes=[w.HDC,w.HBITMAP,w.UINT,w.UINT,c.c_void_p,c.c_void_p,w.UINT]
    hwnd=user.GetAncestor(app.winfo_id(),2);r=w.RECT();user.GetWindowRect(hwnd,c.byref(r))
    width,height=r.right-r.left,r.bottom-r.top
    dc=user.GetWindowDC(hwnd);memory=gdi.CreateCompatibleDC(dc);bitmap=gdi.CreateCompatibleBitmap(dc,width,height)
    old=gdi.SelectObject(memory,bitmap)
    try:
        assert user.PrintWindow(hwnd,memory,2)
        import struct
        info=c.create_string_buffer(struct.pack('<IiiHHIIiiII',40,width,-height,1,32,0,width*height*4,0,0,0,0))
        pixels=c.create_string_buffer(width*height*4)
        assert gdi.GetDIBits(memory,bitmap,0,height,pixels,info,0)==height
        Image.frombuffer('RGB',(width,height),pixels.raw,'raw','BGRX',0,1).save(target)
    finally:gdi.SelectObject(memory,old);gdi.DeleteObject(bitmap);gdi.DeleteDC(memory);user.ReleaseDC(hwnd,dc)
root=Path(__file__).resolve().parent;project=root/'Projects/BehaviorQA080'
original=(project/'project.json').read_bytes()
app=App()
try:
    app.project=project;app._load_config();assert app.behavior_profile.get()==b.PARK
    assert len(app.behavior_combo.cget('values'))==2
    app.behavior_display.set(b.PROFILES[b.LEGACY]);app.choose_behavior();app._save_config()
    app._load_config();assert app.behavior_profile.get()==b.LEGACY
    app.behavior_display.set(b.PROFILES[b.PARK]);app.choose_behavior()
    app.category.set('education');app._save_config();app._load_config()
    assert app.category.get()=='education' and app.behavior_profile.get()==b.PARK
    app.category.set('park');app._save_config()
    app.viewer.load_project(project);deadline=time.monotonic()+30
    while (not app.viewer.gpu or app.viewer.gpu.scene is None) and time.monotonic()<deadline:
        app.update();time.sleep(.02)
    assert app.viewer.gpu and app.viewer.gpu.scene is not None,app.viewer.info.cget('text')
    app.update()
    capture_window(app,root/'release/0.8.0/功能配置界面.png')
    report={'profileSelectionPersists':True,'legacyMigration':True,'menuIndependent':True,'choices':2,
            'hintVisible':app.behavior_hint.winfo_ismapped(),'comboVisible':app.behavior_combo.winfo_ismapped(),
            'props':app.viewer.scene.get('propCount'),'lights':app.viewer.scene.get('lightCount'),
            'glError':app.viewer.gpu.gl.glGetError()}
    (root/'release/0.8.0/gui-behavior-qa.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
finally:
    app.destroy();(project/'project.json').write_bytes(original)
