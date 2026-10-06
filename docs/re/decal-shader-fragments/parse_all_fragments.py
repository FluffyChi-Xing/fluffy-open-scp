# 片段表全量解析 + 四个 decal shader 记录解析 + 片段序列导出
# 布局（已用 EOF 精确匹配验证）：
#   VS 记录: i32 A, i32 B, 3xu8, i32 C, str1, str2, declVec(13B 条目), [C&2] optStr
#   PS 记录: i32 A, i32 B, 2xu8, i32 C, str1, str2, declVec(13B 条目),
#            i32 inst, i32 grp, 7xi32, numNames+names, [C&2] optStr
#   decl 条目: str name + 4xi16 + i32 + u8
# shader 记录（cFragmentShader::Read，flags&0x80000000 分支）：
#   循环 u8 slot（0xFF 终止）→ VS 列表(u32 count + x[24B StateDep + u16 fragIdx]) + PS 列表（同构）
import struct, json, io

data = open('tmp/fragments_0.bin', 'rb').read()
EOF = len(data)

def i32(d, p): return struct.unpack_from('>i', d, p)[0]
def u32(d, p): return struct.unpack_from('>I', d, p)[0]
def u16(d, p): return struct.unpack_from('>H', d, p)[0]

def parse_string(d, p):
    n = i32(d, p); p += 4
    if n < 0 or n > 0x100000: raise ValueError('badlen %d @%x' % (n, p-4))
    return d[p:p+n], p+n

def parse_declvec(d, p):
    n = i32(d, p); p += 4
    if n < 0 or n > 256: raise ValueError('bad declcount %d @%x' % (n, p-4))
    decls = []
    for _ in range(n):
        name, p = parse_string(d, p)
        vals = struct.unpack_from('>4h', d, p); v = i32(d, p+8); b = d[p+12]; p += 13
        decls.append((name, vals, v, b))
    return decls, p

def opt_str(d, p):
    n = i32(d, p); p += 4
    if n < 0 or n > 0x100000: raise ValueError('bad optlen %d @%x' % (n, p-4))
    if n <= 0: return (b'', p)
    return (d[p:p+n], p+n)

def parse_vs(d, p):
    A = i32(d, p); B = i32(d, p+4); C = i32(d, p+11); p += 15
    s1, p = parse_string(d, p); s2, p = parse_string(d, p)
    decls, p = parse_declvec(d, p)
    opt = b''
    if C & 2: opt, p = opt_str(d, p)
    return (A, B, C, s1, s2, decls, opt), p

def parse_ps(d, p):
    A = i32(d, p); B = i32(d, p+4); C = i32(d, p+10); p += 14
    s1, p = parse_string(d, p); s2, p = parse_string(d, p)
    decls, p = parse_declvec(d, p)
    inst = i32(d, p); grp = i32(d, p+4); p += 8
    extra = struct.unpack_from('>7i', d, p); p += 28
    nn = i32(d, p); p += 4
    if nn < 0 or nn > 64: raise ValueError('bad numNames %d @%x' % (nn, p-4))
    names = []
    for _ in range(nn):
        s, p = parse_string(d, p); names.append(s)
    opt = b''
    if C & 2: opt, p = opt_str(d, p)
    return (A, B, C, s1, s2, decls, inst, grp, extra, names, opt), p

p = 8
vs = []
for i in range(1023):
    r, p = parse_vs(data, p); vs.append(r)
psc = i32(data, p); p += 4
ps = []
for i in range(psc):
    r, p = parse_ps(data, p); ps.append(r)
assert p == EOF, 'fragments table must end at EOF'

def txt(b): return b.decode('utf-8', 'replace')

def ser_vs(r):
    return {'A': r[0], 'B': r[1], 'C': r[2], 's1': txt(r[3]), 's2': txt(r[4]),
            'decls': [[txt(d[0]), list(d[1]), d[2], d[3]] for d in r[5]], 'opt': txt(r[6])}
def ser_ps(r):
    return {'A': r[0], 'B': r[1], 'C': r[2], 's1': txt(r[3]), 's2': txt(r[4]),
            'decls': [[txt(d[0]), list(d[1]), d[2], d[3]] for d in r[5]],
            'inst': r[6], 'grp': r[7], 'extra': list(r[8]),
            'names': [txt(n) for n in r[9]], 'opt': txt(r[10])}

with io.open('tmp/vs_fragments.json', 'w', encoding='utf-8') as f:
    json.dump([ser_vs(r) for r in vs], f, ensure_ascii=False)
with io.open('tmp/ps_fragments.json', 'w', encoding='utf-8') as f:
    json.dump([ser_ps(r) for r in ps], f, ensure_ascii=False)
print('fragments saved: VS=%d PS=%d' % (len(vs), len(ps)))

# ---------- shader 记录解析 ----------
sd = open('tmp/shaders_0.bin', 'rb').read()

def parse_frag_list(d, p):
    n = u32(d, p); p += 4
    if n > 256: raise ValueError('bad fraglist count %d @%x' % (n, p-4))
    out = []
    for _ in range(n):
        sd24 = d[p:p+24]; p += 24
        fi = u16(d, p); p += 2
        out.append((sd24, fi))
    return out, p

def parse_shader(d, p):
    # cShaderBase::Read（version=8>7 无头三字段）：vsVer i32 + psVer i32 + mBehaviorFlags i32 + [flags&0x10: name]
    shader_id = u32(d, p); p += 4
    vs_ver = i32(d, p); ps_ver = i32(d, p+4); p += 8
    flags = u32(d, p); p += 4
    name = b''
    if flags & 0x10:
        name, p = parse_string(d, p)
    slots = []
    while True:
        slot = d[p]; p += 1
        if slot == 0xFF: break
        vs_list, p = parse_frag_list(d, p)
        ps_list, p = parse_frag_list(d, p)
        slots.append((slot, vs_list, ps_list))
    return {'id': shader_id, 'vs_ver': vs_ver, 'ps_ver': ps_ver, 'flags': flags,
            'name': txt(name), 'slots': slots}, p

targets = {131462: 'decalProjectSDFLitFront', 132341: 'decalProjectNeonSDF',
           133249: 'decalNeonTubeSDF', 133737: 'decalInteriorMap'}
for off, expect in targets.items():
    sh, end = parse_shader(sd, off)
    status = 'OK' if sh['name'] == expect else 'NAME MISMATCH'
    print('0x%08X @%d name=%s flags=0x%08X slots=%d %s' % (
        sh['id'], off, sh['name'], sh['flags'], len(sh['slots']), status))
    # 导出片段序列
    lines = []
    lines.append('shader 0x%08X %s flags=0x%08X vsVer=%d psVer=%d' % (
        sh['id'], sh['name'], sh['flags'], sh['vs_ver'], sh['ps_ver']))
    for slot, vs_list, ps_list in sh['slots']:
        lines.append('== state slot %d ==' % slot)
        lines.append('-- VS fragments (%d) --' % len(vs_list))
        for sd24, fi in vs_list:
            # VS fragment 1 = 文件记录 0
            rec = vs[fi-1] if 1 <= fi <= len(vs) else None
            nm = txt(rec[6]) if rec else '<builtin>'
            lines.append('  [%d] %s' % (fi, nm))
            if rec:
                if rec[3]: lines.append('    s1: ' + txt(rec[3]).replace('\n', '\\n'))
                if rec[4]: lines.append('    s2: ' + txt(rec[4]).replace('\n', '\\n'))
                if rec[5]: lines.append('    decls: ' + ', '.join(txt(d[0]) for d in rec[5]))
        lines.append('-- PS fragments (%d) --' % len(ps_list))
        for sd24, fi in ps_list:
            rec = ps[fi] if 0 <= fi < len(ps) else None
            nm = txt(rec[10]) if rec else '<oob>'
            lines.append('  [%d] %s' % (fi, nm))
            if rec:
                if rec[3]: lines.append('    s1: ' + txt(rec[3]).replace('\n', '\\n'))
                if rec[4]: lines.append('    s2: ' + txt(rec[4]).replace('\n', '\\n'))
                if rec[5]: lines.append('    decls: ' + ', '.join(txt(d[0]) for d in rec[5]))
                if rec[9]: lines.append('    names: ' + ', '.join(txt(n) for n in rec[9]))
                lines.append('    inst=0x%08X grp=0x%08X extra=%s' % (rec[6] & 0xFFFFFFFF, rec[7] & 0xFFFFFFFF, list(rec[8])))
    with io.open('tmp/shader_frags_%s.txt' % expect, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))
    print('  -> tmp/shader_frags_%s.txt' % expect)
