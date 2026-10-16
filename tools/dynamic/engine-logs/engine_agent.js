'use strict';
const cfg = CONFIG, mod = Process.getModuleByName('SimCity.exe');
const handles = [], hits = {};
let rows = [], total = 0, dropped = 0, errors = 0, sequence = 0;
function textAt(p, limit = 2048) {
    if (p.isNull()) return null;
    let n = 0;
    while (n < limit && p.add(n).readU8() !== 0) n++;
    return {text: p.readCString(n), truncated: n === limit};
}
function arg(ctx, n) { return ctx.esp.add(4 + n * 4).readPointer(); }
function helper(p) {
    if (p.isNull() || !p.readPointer().equals(mod.base.add(0x929ce8)))
        return {pointer: p.toString(), layout: 'unverified'};
    return {pointer: p.toString(), group: textAt(p.add(20).readPointer(), 256),
        level: p.add(16).readS32(), traceType: p.add(8).readU32(),
        enabled: p.add(4).readU8(), filtered: p.add(5).readU8(),
        file: textAt(p.add(24).readPointer(), 512), line: p.add(28).readU32(),
        function: textAt(p.add(32).readPointer(), 512)};
}
function emit(channel, data) {
    if (rows.length >= 1024 || total >= 10000) { dropped++; return; }
    rows.push(Object.assign({sequence: ++sequence, time: Date.now(),
        threadId: Process.getCurrentThreadId(), channel}, data)); total++;
}
function flush() { if (rows.length) { send({kind: 'batch', records: rows}); rows = []; } }
function bytes(p, n) { return Array.from(new Uint8Array(p.readByteArray(n))).map(x => x.toString(16).padStart(2, '0')).join(''); }
for (const [name, h] of Object.entries(cfg.hooks)) {
    const at = mod.base.add(h.rva);
    if (bytes(at, 12) !== h.bytes) throw Error('entry_mismatch:' + name);
    handles.push(Interceptor.attach(at, {onEnter() {
        hits[name] = (hits[name] || 0) + 1;
        try {
            if (name === 'formatted') {
                emit('engine.format', {helper: helper(arg(this.context, 0)), format: textAt(arg(this.context, 1))});
            } else if (name === 'serverTrace') {
                emit('engine.trace', {helper: helper(arg(this.context, 0)), text: textAt(arg(this.context, 1))});
            } else if (name === 'consoleOutput') {
                emit('engine.console', {text: textAt(arg(this.context, 0))});
            } else if (name !== 'getServer') {
                const p = arg(this.context, 0);
                emit('engine.report', {sink: name, record: p.toString(),
                    recordVtable: p.readPointer().toString(), recordNumber: p.add(8).readU32(),
                    helper: helper(p.add(12).readPointer()),
                    text: textAt(p.add(16).readPointer()), levelName: textAt(p.add(20).readPointer(), 128)});
            }
        } catch (e) { errors++; }
    }, onLeave(rv) {
        if (name === 'getServer' && !rv.isNull()) {
            try { emit('engine.server', {pointer: rv.toString(), vtable: rv.readPointer().toString()}); }
            catch (e) { errors++; }
        }
    }}));
}
rpc.exports = {stop() { handles.forEach(h => h.detach()); flush(); return {hits, total, dropped, errors}; }};
setInterval(flush, 100);
Interceptor.flush();
send({kind: 'ready', base: mod.base.toString(), path: mod.path, hooks: Object.keys(cfg.hooks)});
