'use strict';
// Observe global eligibility checks, effective resource-count checks and native bin lookups.
// Never execute rules, call game functions or write game data.
const mod = Process.getModuleByName('SimCity.exe');
let config = null, stopped = true, handles = [], timer = null, queue = [];
let total = 0, reserved = 0, dropped = 0, errors = 0, sequence = 0, targetGame = null;
const checks = new Map(), counts = new Map(), channels = {};
function bytes(p, n) { return Array.from(new Uint8Array(p.readByteArray(n)), x => x.toString(16).padStart(2, '0')).join(''); }
function arg(ctx, n) { return ctx.esp.add(4 + n * 4).readPointer(); }
function push(map, tid, f) { const s = map.get(tid) || []; s.push(f); map.set(tid, s); }
function top(map, tid) { const s = map.get(tid); return s && s[s.length - 1]; }
function pop(map, tid, f) { const s = map.get(tid); if (!s || s[s.length - 1] !== f) { errors++; map.delete(tid); return; } s.pop(); if (!s.length) map.delete(tid); }
function flush() { if (queue.length) { send({kind: 'batch', records: queue}); queue = []; } }
function predictCount(operations) {
    let count = -1;
    for (const op of operations) {
        if (op.status === 'bin-not-found') return {count: 0, reason: 'bin-missing', ordinal: op.ordinal};
        if (op.status !== 'observed') return null;
        const {amount, capacity} = op.bin, input = op.input, output = op.output;
        // Outside nonnegative bin invariants, preserve snapshots without guessing signed-overflow semantics.
        if (amount < 0 || capacity < amount || output === -2147483648) return null;
        if (output < 0) {
            if (amount < input) return {count: 0, reason: 'range-below', ordinal: op.ordinal};
            if (amount >= -output) return {count: 0, reason: 'range-above', ordinal: op.ordinal};
        } else {
            if (input > 0) {
                const limit = Math.trunc(amount / input);
                if (count < 0 || count > limit) count = limit;
                if (count === 0) return {count, reason: 'input-shortfall', ordinal: op.ordinal};
            }
            if (output > input) {
                const net = output - input;
                if (net > 2147483647) return null;
                const limit = Math.trunc((capacity - amount) / net);
                if (count < 0 || count > limit) count = limit;
                if (count === 0) return {count, reason: 'capacity-shortfall', ordinal: op.ordinal};
            }
        }
    }
    return {count, reason: null};
}
function diagnose(row) {
    if (row.readErrors.length) return {reason: 'read-error'};
    if (row.nativeAllowed) return {reason: 'allowed'};
    if (!row.control.matches && row.finalApplyCount === 0) return {reason: 'control-flags'};
    if (row.countCapture !== 'captured') return {reason: 'unresolved'};
    const p = predictCount(row.operations);
    if (!p || p.count !== row.resourceCount) return {reason: 'unresolved'};
    if (p.count === 0 && row.finalApplyCount === 0 && p.reason) return p;
    if (p.count > 0 && p.count < row.minApply && row.finalApplyCount === 0) return {reason: 'minimum-apply', count: p.count};
    return {reason: 'unresolved'};
}
function snapshot(bins, index, resourceIndex) {
    const begin = bins.readPointer(), end = bins.add(4).readPointer(), size = end.toUInt32() - begin.toUInt32();
    if (begin.isNull() || size < 0 || size % 12 || size / 12 > 65535 || index < 0 || index >= size / 12) throw Error('bin_bounds');
    const bin = begin.add(index * 12);
    if (bin.readS16() !== resourceIndex) throw Error('bin_identity_mismatch');
    return {pointer: bin.toString(), binIndex: index, resourceIndex, amount: bin.add(4).readS32(), capacity: bin.add(8).readS32()};
}
const callbacks = {
    globalCheck: {
        enter() {
            this.tid = Process.getCurrentThreadId(); this.frame = {active: false}; push(checks, this.tid, this.frame);
            const game = arg(this.context, 3), rule = arg(this.context, 1), bins = this.context.ecx;
            if (!bins.equals(game.add(0x478)) || (targetGame && game.toString() !== targetGame)) { channels.globalCheck.filtered++; return; }
            const id = rule.readU32();
            if (config.ruleId !== null && id !== config.ruleId) { channels.globalCheck.filtered++; return; }
            if (total + reserved >= config.maxRecords) { dropped++; channels.globalCheck.dropped++; return; }
            const actual = [bins.add(0xb8).readU32(), bins.add(0xbc).readU32()];
            const mask = [rule.add(40).readU32(), rule.add(44).readU32()];
            const expected = [rule.add(48).readU32(), rule.add(52).readU32()];
            const row = {channel: 'simulation.condition', schema: 1, phase: 'global-eligibility', time: Date.now(), threadId: this.tid,
                game: game.toString(), ruleId: '0x' + id.toString(16).padStart(8, '0'), ruleIndex: arg(this.context, 0).toUInt32(),
                simTicks: game.add(0x17a0).readU32(), control: {actual, mask, expected, matches: actual.every((v,i) => ((v & mask[i]) >>> 0) === expected[i])},
                minApply: rule.add(28).readS32(), configuredMaxApply: rule.add(32).readS32(),
                applyResourceId: '0x' + rule.add(36).readU32().toString(16).padStart(8, '0'),
                fromBinOpsOffset: rule.add(20).readS32(), countCapture: 'not-observed', operations: [], readErrors: []};
            Object.assign(this.frame, {active: true, bins, state: arg(this.context, 2), row}); targetGame = game.toString(); reserved++;
        },
        leave(rv) {
            const f = this.frame; if (!f) return;
            try {
                if (!f.active) return;
                f.row.nativeAllowed = !!(rv.toUInt32() & 255);
                try { f.row.finalApplyCount = f.state.add(28).readS32(); } catch (e) { errors++; f.row.readErrors.push(String(e)); }
                f.row.diagnosis = diagnose(f.row); f.row.sequence = ++sequence;
                if (queue.length >= 256) flush(); queue.push(f.row); total++; channels.globalCheck.accepted++;
            } finally { if (f.active) reserved--; pop(checks, this.tid, f); }
        },
    },
    resourceCount: {
        enter() {
            this.tid = Process.getCurrentThreadId(); this.frame = {active: false}; push(counts, this.tid, this.frame);
            const p = top(checks, this.tid);
            if (!p || !p.active || !this.context.esp.readPointer().equals(mod.base.add(0x218826))) return;
            if (!arg(this.context, 1).equals(p.bins)) throw Error('count_bins_mismatch');
            if (p.row.countCapture !== 'not-observed') throw Error('duplicate_count_call');
            const ops = arg(this.context, 0), count = ops.readS32(), array = ops.add(4).readPointer();
            if (count < 0 || (count && array.isNull())) throw Error('operation_bounds');
            if (count > 16) { p.row.countCapture = 'over-limit'; channels.resourceCount.filtered++; return; }
            const operations = [];
            for (let i = 0; i < count; i++) {
                const at = array.add(i * 12);
                operations.push({ordinal: i, resourceId: '0x' + at.readU32().toString(16).padStart(8,'0'), input: at.add(4).readS32(), output: at.add(8).readS32(), status: 'not-observed'});
            }
            p.row.operations = operations; p.row.countCapture = 'pending';
            Object.assign(this.frame, {active: true, parent: p, cursor: 0}); channels.resourceCount.accepted++;
        },
        leave(rv) { const f = this.frame; if (!f) return; try { if (f.active) { f.parent.row.resourceCount = rv.toInt32(); f.parent.row.countCapture = 'captured'; } } finally { pop(counts,this.tid,f); } },
    },
    binFind: {
        enter() {
            const f = top(counts, Process.getCurrentThreadId());
            if (!f || !f.active || !this.context.esp.readPointer().equals(mod.base.add(0x2063b3))) return;
            if (!this.context.ecx.equals(f.parent.bins)) throw Error('lookup_bins_mismatch');
            if (f.cursor >= f.parent.row.operations.length) throw Error('extra_lookup');
            this.frame = f; this.op = f.parent.row.operations[f.cursor++]; this.resourceIndex = arg(this.context, 0).toInt32();
        },
        leave(rv) { if (!this.frame) return; const index = rv.toInt32(); if (index < 0) { this.op.status = 'bin-not-found'; return; } this.op.bin = snapshot(this.frame.parent.bins,index,this.resourceIndex); this.op.status = 'observed'; },
    },
};
function stop() {
    stopped = true; handles.forEach(h=>h.detach()); handles=[]; if(timer!==null)clearInterval(timer);timer=null;flush();
    const incompleteRequests=reserved;checks.clear();counts.clear();return {total,dropped,errors,channels,targetGame,incompleteRequests};
}
rpc.exports = {
    identity() { return {path:mod.path,base:mod.base.toString(),arch:Process.arch,pid:Process.id}; },
    arm(cfg) {
        if(config)throw Error('already_armed');if(Process.arch!=='ia32')throw Error('expected_x86');
        for(const h of Object.values(cfg.hooks))if(bytes(mod.base.add(h.rva),h.bytes.length/2)!==h.bytes)throw Error('entry_mismatch');
        config=cfg;stopped=false;
        function fault(name,e) { errors++;channels[name].errors++;const f=top(checks,Process.getCurrentThreadId());if(f&&f.active)f.row.readErrors.push(name+':'+String(e)); }
        try {
            for(const [name,h] of Object.entries(cfg.hooks)) {
                channels[name]={hits:0,accepted:0,filtered:0,dropped:0,errors:0};
                handles.push(Interceptor.attach(mod.base.add(h.rva),{
                    onEnter(){if(stopped)return;channels[name].hits++;try{callbacks[name].enter.call(this);}catch(e){fault(name,e);}},
                    onLeave(rv){if(stopped)return;try{callbacks[name].leave.call(this,rv);}catch(e){fault(name,e);}},
                }));
            }
            timer=setInterval(flush,100);Interceptor.flush();return {hooks:Object.keys(cfg.hooks),scope:'global-eligibility'};
        }catch(e){stop();throw e;}
    },stop,
};
