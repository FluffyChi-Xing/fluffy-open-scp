import struct
def parse(data,include_raw=False):
    pos=4;result={}
    def val(typ,flags):
        nonlocal pos
        start=pos;sizes={1:1,9:4,10:4,13:4,32:12,34:8,48:8,49:12,50:12,51:16,52:16,57:24}
        if typ in sizes:pos+=sizes[typ]
        elif typ in (18,19):
            n=struct.unpack_from('>I',data,pos)[0];pos+=4
            if typ==18 and n==0 and flags&0x100:n=data[pos];pos+=1
            pos+=n*(2 if typ==19 else 1)
        elif typ==56:
            f=struct.unpack_from('>H',data,pos)[0];pos+=2+(4 if f==15 else 0)+4*({12:3,13:4}.get(f,12))
        else:raise ValueError(f'Unsupported property type {typ}')
        if pos>len(data):raise ValueError('Truncated property')
        b=data[start:pos]
        if typ==32:
            i,t,g=struct.unpack('>3I',b);return [t,g,i]
        if typ in (9,10):return struct.unpack('>i' if typ==9 else '>I',b)[0]
        if typ==13:return struct.unpack('>f',b)[0]
        if typ==34:
            table,ident=struct.unpack('>2I',b);return {'table':table,'id':ident}
        if typ==57:return list(struct.unpack('>6f',b))
        if typ in (48,49,50,51,52):return list(struct.unpack('>'+'f'*(len(b)//4),b))
        return {'rawBytes':len(b)}
    for _ in range(struct.unpack_from('>I',data)[0]):
        record_start=pos
        h,t,f=struct.unpack_from('>IHH',data,pos);pos+=8
        if not f&0x30:v=val(t,f)
        elif not f&0x40:
            n,size=struct.unpack_from('>2I',data,pos);pos+=8
            if n>65536:raise ValueError('Oversized array')
            v=[val(t,0) for _ in range(n)]
        else:raise ValueError('Unsupported property flags')
        result[f'{h:08X}']={'type':t,'flags':f,'value':v}
        if include_raw:result[f'{h:08X}']['recordHex']=data[record_start:pos].hex()
    if pos!=len(data):raise ValueError('Unconsumed property bytes')
    return result

