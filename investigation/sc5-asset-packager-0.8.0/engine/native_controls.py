"""Numeric native-export controls; preview brightness never enters these maps."""
import numpy as np
FLAT=23/255
def relief_field(shader,strength,rect,atlas_scale=.5):
 out=shader.copy();strength=max(0,float(strength))
 # Height is an atlas-domain distance. Keep UI depth relative to a window,
 # including the extra flat Base strip appended by the native exporter.
 scale=min(abs(float(rect[0]))*atlas_scale,abs(float(rect[1])))
 gain=strength*scale/.16
 out[:,:,0]=np.clip(FLAT+(shader[:,:,0]-FLAT)*gain,0,1)
 # Preserve both positive and negative heights; never scale only R > flat.
 out[:,:,1]=np.clip(shader[:,:,1]/max(gain,1e-6),0,1)
 return out
def palette_srgb(rgb,srgb):
 # Blender decoded the entire sRGB image, including numeric rows. Undo that
 # decode for numeric bytes too; they are not newly tinted or lit here.
 out=rgb.copy();out[:14]=srgb(out[:14]);out[14:]=srgb(out[14:]);return out
def room_emission(alpha,glow):
 # Original game alpha is already authored for the native HDR multiplier.
 return np.clip(alpha*max(0,float(glow))*2,0,1)

def office_room_sheet(source):
 """Four room variants with day/night/off/dark states from actual room rows."""
 if source.ndim!=3 or source.shape[0]<64 or source.shape[1]<256:raise ValueError('Office room source is too small')
 out=np.empty((512,512,4),dtype=source.dtype)
 for row in range(16):
  for v in range(4):
   for state in range(4):
    x=((v%2)*4+state)*32;y=(v//2)*32
    tile=source[y:y+32,x:x+32].copy()
    if state!=1:tile[:,:,3]=0
    out[row*32:(row+1)*32,(v*4+state)*32:(v*4+state+1)*32]=tile
 return out
