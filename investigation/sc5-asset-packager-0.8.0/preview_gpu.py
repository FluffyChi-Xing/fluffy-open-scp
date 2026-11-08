"""Windows WGL viewer: persistent GPU textures and indexed arrays, no frame PNGs."""
import ctypes as c
from ctypes import wintypes as w
import math
import numpy as np
from PIL import Image

class PFD(c.Structure):
    _fields_=[('nSize',w.WORD),('nVersion',w.WORD),('dwFlags',w.DWORD),('iPixelType',c.c_ubyte),('cColorBits',c.c_ubyte),('cRedBits',c.c_ubyte),('cRedShift',c.c_ubyte),('cGreenBits',c.c_ubyte),('cGreenShift',c.c_ubyte),('cBlueBits',c.c_ubyte),('cBlueShift',c.c_ubyte),('cAlphaBits',c.c_ubyte),('cAlphaShift',c.c_ubyte),('cAccumBits',c.c_ubyte),('cAccumRedBits',c.c_ubyte),('cAccumGreenBits',c.c_ubyte),('cAccumBlueBits',c.c_ubyte),('cAccumAlphaBits',c.c_ubyte),('cDepthBits',c.c_ubyte),('cStencilBits',c.c_ubyte),('cAuxBuffers',c.c_ubyte),('iLayerType',c.c_ubyte),('bReserved',c.c_ubyte),('dwLayerMask',w.DWORD),('dwVisibleMask',w.DWORD),('dwDamageMask',w.DWORD)]

class GPUPreview:
    def __init__(self,canvas):
        self.canvas=canvas;self.gl=c.WinDLL('opengl32');self.gdi=c.WinDLL('gdi32');self.user=c.WinDLL('user32');self.meshes=[];self.textures=[];self.scene=None
        def bind(lib,name,result,args):
            f=getattr(lib,name);f.restype=result;f.argtypes=args;return f
        ptr=c.c_void_p;I=c.c_int;U=c.c_uint;F=c.c_float;D=c.c_double
        bind(self.user,'GetDC',ptr,[ptr]);bind(self.user,'ReleaseDC',I,[ptr,ptr])
        bind(self.gdi,'ChoosePixelFormat',I,[ptr,c.POINTER(PFD)]);bind(self.gdi,'SetPixelFormat',I,[ptr,I,c.POINTER(PFD)]);bind(self.gdi,'SwapBuffers',I,[ptr])
        bind(self.gl,'wglCreateContext',ptr,[ptr]);bind(self.gl,'wglMakeCurrent',I,[ptr,ptr]);bind(self.gl,'wglDeleteContext',I,[ptr])
        signatures={'glPushMatrix':(None,[]),'glPopMatrix':(None,[]),'glMultMatrixf':(None,[ptr]),'glAlphaFunc':(None,[U,F]),'glEnable':(None,[U]),'glDisable':(None,[U]),'glEnableClientState':(None,[U]),'glVertexPointer':(None,[I,U,I,ptr]),'glNormalPointer':(None,[U,I,ptr]),'glTexCoordPointer':(None,[I,U,I,ptr]),'glDrawElements':(None,[U,I,U,ptr]),'glGenTextures':(None,[I,c.POINTER(U)]),'glDeleteTextures':(None,[I,c.POINTER(U)]),'glBindTexture':(None,[U,U]),'glTexParameteri':(None,[U,U,I]),'glPixelStorei':(None,[U,I]),'glTexImage2D':(None,[U,I,I,I,I,I,U,U,ptr]),'glClearColor':(None,[F,F,F,F]),'glClear':(None,[U]),'glViewport':(None,[I,I,I,I]),'glMatrixMode':(None,[U]),'glLoadIdentity':(None,[]),'glOrtho':(None,[D,D,D,D,D,D]),'glLoadMatrixf':(None,[ptr]),'glRotatef':(None,[F,F,F,F]),'glColor3f':(None,[F,F,F]),'glBegin':(None,[U]),'glEnd':(None,[]),'glVertex3f':(None,[F,F,F]),'glLineWidth':(None,[F]),'glGetString':(c.c_char_p,[U]),'glFinish':(None,[]),'glGetError':(U,[])}
        for name,(ret,args) in signatures.items():bind(self.gl,name,ret,args)
        self.hwnd=canvas.winfo_id();self.dc=self.user.GetDC(self.hwnd)
        p=PFD();p.nSize=c.sizeof(PFD);p.nVersion=1;p.dwFlags=0x25;p.cColorBits=32;p.cAlphaBits=8;p.cDepthBits=24
        fmt=self.gdi.ChoosePixelFormat(self.dc,c.byref(p))
        if not fmt or not self.gdi.SetPixelFormat(self.dc,fmt,c.byref(p)):raise RuntimeError('无法初始化显卡像素格式')
        self.rc=self.gl.wglCreateContext(self.dc)
        if not self.rc or not self.gl.wglMakeCurrent(self.dc,self.rc):raise RuntimeError('无法建立 OpenGL 预览')
        self.renderer=self.gl.glGetString(0x1f01).decode(errors='replace')
        self.gl.glEnable(0x0b71);self.gl.glDisable(0x0b44)
        for state in (0x8074,0x8075,0x8078):self.gl.glEnableClientState(state)
        self.gl.glClearColor(.141,.2,.278,1)
    def texture(self,pixels):
        g=self.gl;t=c.c_uint();g.glGenTextures(1,c.byref(t));self.textures.append(t.value);g.glBindTexture(0xde1,t.value)
        g.glPixelStorei(0xcf5,1);g.glTexParameteri(0xde1,0x2800,0x2601);g.glTexParameteri(0xde1,0x2801,0x2703)
        im=Image.fromarray(pixels).convert('RGBA');level=0
        while True:
            a=np.ascontiguousarray(im,dtype=np.uint8);g.glTexImage2D(0xde1,level,0x1908,im.width,im.height,0,0x1908,0x1401,a.ctypes.data)
            if im.size==(1,1):break
            im=im.resize((max(1,im.width//2),max(1,im.height//2)),Image.Resampling.BOX);level+=1
        return t.value
    def upload(self,scene):
        self.gl.wglMakeCurrent(self.dc,self.rc)
        if self.textures:self.gl.glDeleteTextures(len(self.textures),(c.c_uint*len(self.textures))(*self.textures))
        self.textures=[];self.meshes=[];self.scene=scene
        for m in scene['meshes']:
            mesh=dict(m);mesh['texture']=self.texture(m['image']) if m['image'] is not None else 0;self.meshes.append(mesh)
        self.prop_meshes={}
        for name,m in scene.get('propTemplates',{}).items():
            mesh=dict(m);mesh['texture']=self.texture(m['image']);self.prop_meshes[name]=mesh
        self.ground=None
        if scene['ground']:
            d=scene['ground'];mask=d['mask']/255.;rgb=np.full((*mask.shape[:2],3),.34)
            for i,col in enumerate(d['colors']):
                weight=mask[:,:,[2,1,0,3][i],None];rgb=rgb*(1-weight)+col*weight
            rgb=np.where(rgb<=.0031308,rgb*12.92,1.055*np.maximum(rgb,0)**(1/2.4)-.055)
            sx,sy=d['size'];ox,oy=d['offset']
            self.ground={'positions':np.array([[-sx/2+ox,sy/2+oy,-.03],[sx/2+ox,sy/2+oy,-.03],[sx/2+ox,-sy/2+oy,-.03],[-sx/2+ox,-sy/2+oy,-.03]],np.float32),'normals':np.array([[0,0,1]]*4,np.float32),'uv':np.array([[0,0],[1,0],[1,1],[0,1]],np.float32),'indices':np.array([0,1,2,0,2,3],np.uint32),'texture':self.texture(np.uint8(np.clip(rgb*255,0,255)))}
    def draw_mesh(self,m):
        g=self.gl;g.glColor3f(1,1,1)
        cutoff=m.get('alphaCutoff')
        if cutoff is not None:g.glEnable(0x0bc0);g.glAlphaFunc(0x0204,cutoff)
        else:g.glDisable(0x0bc0)
        if m['texture']:g.glEnable(0xde1);g.glBindTexture(0xde1,m['texture'])
        else:g.glDisable(0xde1);g.glColor3f(.68,.68,.68)
        g.glVertexPointer(3,0x1406,0,m['positions'].ctypes.data);g.glNormalPointer(0x1406,0,m['normals'].ctypes.data);g.glTexCoordPointer(2,0x1406,0,m['uv'].ctypes.data)
        g.glDrawElements(4,m['indices'].size,0x1405,m['indices'].ctypes.data)
    def render(self,rotation,yaw,pitch,zoom,building=True,ground=True,props=True,lights=True):
        g=self.gl;g.wglMakeCurrent(self.dc,self.rc);width=max(1,self.canvas.winfo_width());height=max(1,self.canvas.winfo_height());g.glViewport(0,0,width,height);g.glClear(0x4100)
        s=self.scene;lo=s['low'].copy();hi=s['high'].copy()
        if self.ground:
            lo=np.minimum(lo,self.ground['positions'].min(axis=0));hi=np.maximum(hi,self.ground['positions'].max(axis=0))
        if props and s.get('propBounds') is not None:
            lo=np.minimum(lo,s['propBounds'][0]);hi=np.maximum(hi,s['propBounds'][1])
        from asset_transforms import rotate_xyz
        corners=rotate_xyz(np.array([[x,y,z] for x in (lo[0],hi[0]) for y in (lo[1],hi[1]) for z in (lo[2],hi[2])]),rotation)
        lo=corners.min(axis=0);hi=corners.max(axis=0)
        center=(lo+hi)/2;extent=max(float(np.linalg.norm(hi-lo)),1);span=extent*.6/zoom;aspect=width/height
        g.glMatrixMode(0x1701);g.glLoadIdentity();g.glOrtho(-span*aspect,span*aspect,-span,span,-extent*10,extent*10)
        right=np.array([math.cos(yaw),math.sin(yaw),0]);forward=np.array([-math.sin(yaw)*math.cos(pitch),math.cos(yaw)*math.cos(pitch),-math.sin(pitch)]);up=np.cross(right,forward)
        matrix=np.eye(4,dtype=np.float32);matrix[:3,:3]=np.stack([right,up,-forward]);matrix[:3,3]=-matrix[:3,:3]@center
        a=np.ascontiguousarray(matrix.T);g.glMatrixMode(0x1700);g.glLoadMatrixf(a.ctypes.data);g.glRotatef(rotation,0,0,1)
        if ground and self.ground:self.draw_mesh(self.ground)
        if building:
            for m in self.meshes:self.draw_mesh(m)
        if props:
            for instance in s.get('propInstances',[]):
                transform=np.ascontiguousarray(instance['matrix'].T);g.glPushMatrix();g.glMultMatrixf(transform.ctypes.data)
                self.draw_mesh(self.prop_meshes[instance['preset']]);g.glPopMatrix()
        g.glDisable(0x0bc0)
        if lights:
            g.glDisable(0xde1);g.glLineWidth(2)
            for lamp in s.get('lightInstances',[]):
                m=np.eye(4,dtype=np.float32);m[:3,:3]=np.array(lamp['matrix'][:9],np.float32).reshape(3,3).T;m[:3,3]=lamp['matrix'][9:]
                transform=np.ascontiguousarray(m.T);g.glPushMatrix();g.glMultMatrixf(transform.ctypes.data);g.glColor3f(*lamp['color']);g.glBegin(1)
                radius=lamp['radius'];length=lamp['length'];steps=24
                for i in range(steps):
                    t0=i*2*math.pi/steps;t1=(i+1)*2*math.pi/steps
                    if lamp['type']=='SPOT':
                        for t in (t0,t1):g.glVertex3f(radius*math.cos(t),length,radius*math.sin(t))
                        if i%6==0:g.glVertex3f(0,0,0);g.glVertex3f(radius*math.cos(t0),length,radius*math.sin(t0))
                    else:
                        for axis in range(3):
                            for t in (t0,t1):
                                q=[radius*math.cos(t),radius*math.sin(t),0];q=q[axis:]+q[:axis];g.glVertex3f(*q)
                g.glVertex3f(0,0,0);g.glVertex3f(0,length if lamp['type']=='SPOT' else radius,0);g.glEnd();g.glPopMatrix()
        # World-space road arrow stays fixed while export geometry rotates.
        g.glLoadMatrixf(a.ctypes.data);g.glDisable(0xde1);g.glDisable(0xb71);g.glColor3f(.32,.84,.97);g.glLineWidth(3)
        size=s['ground']['size'] if s['ground'] else hi-lo;length=max(size[:2])*.75
        g.glBegin(1)
        for point in ((0,0,0),(0,length,0),(0,length,0),(-length*.08,length*.85,0),(0,length,0),(length*.08,length*.85,0)) :g.glVertex3f(*point)
        g.glEnd();g.glEnable(0xb71);self.gdi.SwapBuffers(self.dc)
    def close(self):
        if getattr(self,'rc',None):
            self.gl.wglMakeCurrent(None,None);self.gl.wglDeleteContext(self.rc);self.user.ReleaseDC(self.hwnd,self.dc);self.rc=None
