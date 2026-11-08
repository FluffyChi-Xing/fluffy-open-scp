"""Offline software geometry preview, sharing orientation with the compiler.

PBR preview colors are sampled from the LOTM's embedded glTF. This is not a
simulation of SimCity's native parallax, HDR or night shaders.
"""
from pathlib import Path
import io
import math
import struct
import sys
import numpy as np
from PIL import Image, ImageDraw
from asset_transforms import rotate_xyz, rotate_lot


def engine_path():
    root = Path(__file__).resolve().parent/'engine'
    if str(root) not in sys.path:
        sys.path.insert(0,str(root))


def accessor(g,b,i):
    a=g['accessors'][i]; v=g['bufferViews'][a['bufferView']]
    n={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[a['type']]
    dtype=np.dtype({5126:'<f4',5125:'<u4',5123:'<u2',5121:'u1'}[a['componentType']])
    count=a['count']; start=v.get('byteOffset',0)+a.get('byteOffset',0)
    stride=v.get('byteStride',n*dtype.itemsize)
    if count and start+(count-1)*stride+n*dtype.itemsize>len(b):
        raise ValueError('几何缓冲区越界')
    return np.ndarray((count,n),dtype=dtype,buffer=b,offset=start,strides=(stride,dtype.itemsize)).copy()


def load_scene(model):
    engine_path()
    from probe_tower import read
    from package_io import index
    from inspect_park_route import parse
    model=Path(model)
    with model.open('rb') as stream:
        header=stream.read(12)
    if len(header)!=12 or header[:4]!=b'LOTM' or struct.unpack_from('<I',header,4)[0]!=9:
        raise ValueError('三维预览需要插件导出的 LOTM9 文件')
    chunks,mats,_,diag=read(model)
    triangles=[]; colors=[]; vertices=0; warnings=[]; meshes=[]
    for g,b in chunks:
        pr=g['meshes'][0]['primitives'][0]; attrs=pr['attributes']
        pos=accessor(g,b,attrs['POSITION']); ix=accessor(g,b,pr['indices']).reshape(-1,3)
        if not np.isfinite(pos).all() or ix.max()>=len(pos):
            raise ValueError('模型包含无效坐标或索引')
        vertices+=len(pos); tri=pos[ix]; triangles.append(tri)
        color=np.full((len(ix),3),175,np.uint8)
        uv=np.zeros((len(pos),2),np.float32); image=None
        try:
            mat=g['materials'][pr['material']]
            tex=mat['pbrMetallicRoughness']['baseColorTexture']
            uv=accessor(g,b,attrs['TEXCOORD_'+str(tex.get('texCoord',0))])
            imdoc=g['images'][g['textures'][tex['index']]['source']]
            view=g['bufferViews'][imdoc['bufferView']]; at=view.get('byteOffset',0)
            image=np.array(Image.open(io.BytesIO(b[at:at+view['byteLength']])).convert('RGB'))
            sample=np.mod(uv[ix].mean(axis=1),1)
            # glTF texture coordinates are top-down; native chunk UVs already follow it.
            xx=np.floor(sample[:,0]*image.shape[1]).astype(int)
            yy=np.floor(sample[:,1]*image.shape[0]).astype(int)
            color=image[yy,xx]
        except (KeyError,ValueError,IndexError) as exc:
            warnings.append('部分预览颜色使用默认值：'+str(exc))
        colors.append(color)
        normals=accessor(g,b,attrs['NORMAL']) if 'NORMAL' in attrs else np.zeros_like(pos)
        meshes.append({'positions':pos.astype(np.float32),'normals':normals.astype(np.float32),'uv':uv.astype(np.float32),'indices':ix.astype(np.uint32),'image':image})
    tris=np.concatenate(triangles).astype(np.float32)
    lo=tris.min(axis=(0,1)); hi=tris.max(axis=(0,1))
    scene={'meshes':meshes,'triangles':tris,'colors':np.concatenate(colors),'low':lo,'high':hi,
           'vertices':vertices,'materialGroups':len(mats),'triangleCount':len(tris),
           'path':str(model),'profile':diag.get('nativeControlProfile','未知'),
           'warnings':list(dict.fromkeys(warnings)),'ground':None}
    ground=model.with_suffix('.lot.package')
    if ground.is_file():
        resources={e.key:e.read() for e in index(ground)}
        lots=[parse(v) for k,v in resources.items() if k[0]==0xb1b104]
        if len(lots)!=1:
            raise ValueError('地块属性数量不正确')
        lot=lots[0];raw=resources[tuple(lot['0CCB7FD5']['value'])]
        _,w,h,_,_,_=struct.unpack_from('>6I',raw)
        mask=np.frombuffer(raw[28:28+w*h*4],np.uint8).reshape(h,w,4)[:,:,[2,1,0,3]]
        scene['ground']={'mask':mask.copy(),'size':lot['0CCB7FC8']['value'],
                         'offset':lot['0CCB7FC9']['value'],'period':lot['0CCB7FD0']['value'],
                         'colors':np.array([lot[f'{0x0d02d586+i:08X}']['value'][:3] for i in range(4)])}
    else:
        scene['warnings'].append('缺少同名 .lot.package，无法预览或编译地块')
    from prop_contract import read_manifest,preview_instances
    doc,_=read_manifest(model,diag)
    templates,instances,bounds=preview_instances(doc)
    scene.update(propTemplates=templates,propInstances=instances,propCount=len(instances),propBounds=bounds)
    import light_contract
    light_doc,_=light_contract.read_manifest(model,diag)
    scene.update(lightInstances=light_doc['instances'],lightCount=len(light_doc['instances']))
    return scene


def render_scene(scene, width=760, height=480, rotation=180, yaw=-.8, pitch=.35,
                 zoom=1., show_building=True, show_ground=True):
    width=max(200,width);height=max(180,height)
    tris=rotate_xyz(scene['triangles'],rotation)
    lo=tris.min(axis=(0,1));hi=tris.max(axis=(0,1));center=(lo+hi)/2
    extent=max(float(np.linalg.norm(hi-lo)),1)
    scale=min(width,height)*.82/extent*zoom
    right=np.array([math.cos(yaw),math.sin(yaw),0])
    forward=np.array([-math.sin(yaw)*math.cos(pitch),math.cos(yaw)*math.cos(pitch),-math.sin(pitch)])
    up=np.cross(right,forward)
    def project(p):
        q=np.asarray(p)-center
        return np.stack([q@right*scale+width/2,height*.52-q@up*scale],axis=-1),q@forward
    alltris=[];allcolors=[]
    if show_building:
        normal=np.cross(tris[:,1]-tris[:,0],tris[:,2]-tris[:,0])
        normal/=np.maximum(np.linalg.norm(normal,axis=1,keepdims=True),1e-9)
        light=np.abs(normal@np.array([.35,-.45,.82]))*.42+.58
        alltris.append(tris);allcolors.append(np.clip(scene['colors']*light[:,None],0,255).astype(np.uint8))
    size=[max(hi[0]-lo[0],5),max(hi[1]-lo[1],5)];offset=[0,0]
    if scene['ground']:
        ground=scene['ground'];mask,size,offset,_=rotate_lot(ground['mask'],ground['size'],ground['offset'],ground['period'],rotation)
        if show_ground:
            n=24;edgesx=np.linspace(-size[0]/2+offset[0],size[0]/2+offset[0],n+1)
            edgesy=np.linspace(size[1]/2+offset[1],-size[1]/2+offset[1],n+1)
            a=[];c=[]
            for y in range(n):
                for x in range(n):
                    v=[[edgesx[x],edgesy[y],-.03],[edgesx[x+1],edgesy[y],-.03],[edgesx[x+1],edgesy[y+1],-.03],[edgesx[x],edgesy[y+1],-.03]]
                    weights=mask[min(int((y+.5)/n*len(mask)),len(mask)-1),min(int((x+.5)/n*mask.shape[1]),mask.shape[1]-1),[2,1,0,3]]/255
                    color=np.array([.33,.35,.33])
                    for weight,col in zip(weights,ground['colors']):color=color*(1-weight)+col*weight
                    color=np.where(color<=.0031308,color*12.92,1.055*np.maximum(color,0)**(1/2.4)-.055)
                    a.extend([[v[0],v[1],v[2]],[v[0],v[2],v[3]]]);c.extend([np.clip(color*255,0,255)]*2)
            alltris.append(np.array(a));allcolors.append(np.array(c,dtype=np.uint8))
    out=Image.new('RGB',(width,height),'#243347');draw=ImageDraw.Draw(out)
    if alltris:
        t=np.concatenate(alltris);cols=np.concatenate(allcolors)
        xy,depth=project(t);order=np.argsort(depth.mean(axis=1))[::-1]
        for i in order:
            p=xy[i]
            if p[:,0].max()<0 or p[:,0].min()>width or p[:,1].max()<0 or p[:,1].min()>height:continue
            draw.polygon([tuple(v) for v in p],fill=tuple(map(int,cols[i])))
    # Fixed game road side is independent of the selected model rotation.
    length=max(size)*.5
    road=np.array([[offset[0]-length,offset[1]+size[1]*.5+2,0],
                   [offset[0]+length,offset[1]+size[1]*.5+2,0]])
    roadxy,_=project(road);draw.line([tuple(v) for v in roadxy],fill='#53d6f6',width=5)
    anchor=np.array([[0,0,0],[0,max(size)*.8,0]])
    xy,_=project(anchor);start,end=xy
    draw.line([tuple(start),tuple(end)],fill='#53d6f6',width=3)
    delta=end-start;delta/=max(np.linalg.norm(delta),1);perp=np.array([-delta[1],delta[0]])
    draw.polygon([tuple(end),tuple(end-delta*15+perp*7),tuple(end-delta*15-perp*7)],fill='#53d6f6')
    png=io.BytesIO();out.save(png,format='PNG')
    return png.getvalue(), {'roadLabel':end.tolist(),'size':list(map(float,size)),
                            'modelDimensions':(hi-lo).tolist(),'rotationDegrees':rotation}
