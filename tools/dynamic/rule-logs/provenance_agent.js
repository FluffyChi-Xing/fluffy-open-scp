'use strict';
// Observe normal loading only. No NativeFunction calls or resource/cache writes.
const mod = Process.getModuleByName('SimCity.exe');
let handles = [], timer = null, config = null, stopped = true;
let sequence = 0, requestId = 0, total = 0, dropped = 0, errors = 0, queue = [];
const stacks = new Map(), channels = {};
function bytes(p, n) { return Array.from(new Uint8Array(p.readByteArray(n)), x => x.toString(16).padStart(2, '0')).join(''); }
function arg(ctx, n) { return ctx.esp.add(4 + n * 4).readPointer(); }
function key(p) { return p.isNull() ? null : {instance: p.readU32(), type: p.add(4).readU32(), group: p.add(8).readU32()}; }
function wanted(k) { return k && k.type === 0x08068ae9 && [0x40800200, 0x40800201].includes(k.group); }
function same(a, b) { return a && b && a.instance === b.instance && a.group === b.group; }
function database(p) {
    if (p.isNull()) return null;
    const vt = p.readPointer();
    const result = {pointer: p.toString(), vtableRva: vt.sub(mod.base).toString(), path: null};
    if (vt.equals(mod.base.add(0x929e20))) {
        const s = p.add(32).readPointer();
        if (!s.isNull()) {
            let value = '';
            for (let i = 0; i < 2048; i++) {
                const c = s.add(i * 2).readU16();
                if (c === 0) { result.path = value; return result; }
                value += String.fromCharCode(c);
            }
            throw Error('database_path_limit');
        }
    }
    return result;
}
function current(thread) { const s = stacks.get(thread), f = s && s[s.length - 1]; return f && f.target ? f : null; }
function emit(phase, data) {
    if (stopped) return;
    if (total >= config.maxRecords) { dropped++; channels[phase].dropped++; return; }
    // Synchronous loading can starve the timer; drain a bounded batch inline.
    if (queue.length >= 256) flush();
    queue.push({channel: 'resource.provenance', schema: 1, phase, sequence: ++sequence, time: Date.now(), ...data});
    total++; channels[phase].accepted++;
}
function flush() { if (queue.length) { send({kind: 'batch', records: queue}); queue = []; } }
function classify(success, returned, frame) {
    if (!success || !returned || returned === '0x0') return {provenance: 'failed', database: null, recordKey: null};
    const match = frame.created.find(x => x.resource === returned && x.database && x.recordKey);
    if (match) return {provenance: 'created-from-record', database: match.database, recordKey: match.recordKey};
    return {provenance: frame.cache.some(x => x === returned) ? 'cache-hit-origin-unknown' : 'origin-unobserved', database: null, recordKey: null};
}
const callbacks = {
    request: {
        enter() {
            // Every request is a nesting barrier, including unrelated keys.
            this.thread = Process.getCurrentThreadId();
            this.frame = {target: false};
            const stack = stacks.get(this.thread) || [];
            stack.push(this.frame); stacks.set(this.thread, stack);
            // Seven stack arguments; optional resourceKey supersedes origKey.
            const override = arg(this.context, 6);
            const k = key(override.isNull() ? arg(this.context, 1) : override);
            if (!wanted(k)) return;
            Object.assign(this.frame, {target: true, id: ++requestId, key: k, records: new Map(), created: [], cache: [], output: arg(this.context, 2)});
        },
        leave(rv) {
            if (!this.frame) return;
            try {
                if (!this.frame.target) return;
                const success = !!(rv.toUInt32() & 255);
                const resource = success && !this.frame.output.isNull() ? this.frame.output.readPointer().toString() : null;
                emit('request', {requestId: this.frame.id, threadId: this.thread, key: this.frame.key, success,
                    resource, ...classify(success, resource, this.frame)});
            } finally {
                const stack = stacks.get(this.thread);
                if (stack && stack[stack.length - 1] === this.frame) stack.pop();
                else { errors++; stacks.delete(this.thread); }
                if (stack && !stack.length) stacks.delete(this.thread);
            }
        },
    },
    cache: {
        enter() {
            const f = current(Process.getCurrentThreadId());
            if (!f || !same(key(arg(this.context, 0)), f.key)) return;
            this.frame = f; this.output = arg(this.context, 1);
        },
        leave(rv) {
            if (!this.frame) return;
            const success = !!(rv.toUInt32() & 255);
            const resource = success && !this.output.isNull() ? this.output.readPointer().toString() : null;
            if (resource) this.frame.cache.push(resource);
            emit('cache', {requestId: this.frame.id, success, resource});
        },
    },
    record: {
        enter() {
            const f = current(Process.getCurrentThreadId());
            if (!f) return;
            const k = key(arg(this.context, 0));
            if (!same(k, f.key) || ![0x08068aea, 0x08068aeb].includes(k.type)) return;
            this.frame = f; this.key = k; this.output = arg(this.context, 1); this.db = database(this.context.ecx);
        },
        leave(rv) {
            if (!this.frame) return;
            const success = !!(rv.toUInt32() & 255);
            const record = success && !this.output.isNull() ? this.output.readPointer().toString() : null;
            if (record && record !== '0x0') this.frame.records.set(record, {database: this.db, recordKey: this.key});
            emit('record', {requestId: this.frame.id, recordKey: this.key, database: this.db, success, record,
                materialize: !this.output.isNull()});
        },
    },
    create: {
        enter() {
            const f = current(Process.getCurrentThreadId());
            if (!f || arg(this.context, 3).toUInt32() !== 0x08068ae9) return;
            this.frame = f; this.record = arg(this.context, 0).toString(); this.output = arg(this.context, 1);
            this.origin = f.records.get(this.record) || {database: null, recordKey: null};
        },
        leave(rv) {
            if (!this.frame) return;
            const success = !!(rv.toUInt32() & 255);
            const resource = success && !this.output.isNull() ? this.output.readPointer().toString() : null;
            if (resource && resource !== '0x0') this.frame.created.push({resource, ...this.origin});
            emit('create', {requestId: this.frame.id, success, resource, record: this.record, ...this.origin});
        },
    },
};
function stop() {
    stopped = true; handles.forEach(h => h.detach()); handles = [];
    if (timer !== null) clearInterval(timer); timer = null;
    flush();
    const incompleteRequests = Array.from(stacks.values()).reduce((n, s) => n + s.filter(f => f.target).length, 0);
    stacks.clear();
    return {total, dropped, errors, channels, incompleteRequests};
}
rpc.exports = {
    identity() { return {path: mod.path, base: mod.base.toString(), arch: Process.arch, pid: Process.id}; },
    arm(cfg) {
        if (config) throw Error('already_armed');
        if (Process.arch !== 'ia32') throw Error('expected_x86');
        for (const h of Object.values(cfg.hooks)) if (bytes(mod.base.add(h.rva), h.bytes.length / 2) !== h.bytes) throw Error('entry_mismatch');
        config = cfg; stopped = false;
        try {
            for (const [name, h] of Object.entries(cfg.hooks)) {
                channels[name] = {hits: 0, accepted: 0, dropped: 0, errors: 0};
                handles.push(Interceptor.attach(mod.base.add(h.rva), {
                    onEnter() { if (stopped) return; channels[name].hits++; try { callbacks[name].enter.call(this); } catch (_) { errors++; channels[name].errors++; } },
                    onLeave(rv) { if (stopped) return; try { callbacks[name].leave.call(this, rv); } catch (_) { errors++; channels[name].errors++; } },
                }));
            }
            timer = setInterval(flush, 100); Interceptor.flush();
            return {hooks: Object.keys(cfg.hooks), scope: 'rule-resource-provenance'};
        } catch (e) { stop(); throw e; }
    },
    stop,
};
