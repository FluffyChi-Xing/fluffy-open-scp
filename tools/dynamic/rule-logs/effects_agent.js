'use strict';
// Observe normal global/unit-local RunRule -> ApplyBinRule -> bin lookup calls only.
// No NativeFunction calls, resource writes, or guessed resource-index maps.
const mod = Process.getModuleByName('SimCity.exe');
let config = null, stopped = true, handles = [], timer = null, queue = [];
let total = 0, reserved = 0, dropped = 0, errors = 0, sequence = 0, target = null;
const parents = new Map(), updates = new Map(), channels = {};
function bytes(p, n) { return Array.from(new Uint8Array(p.readByteArray(n)), x => x.toString(16).padStart(2, '0')).join(''); }
function arg(ctx, n) { return ctx.esp.add(4 + n * 4).readPointer(); }
function push(map, tid, frame) { const s = map.get(tid) || []; s.push(frame); map.set(tid, s); }
function top(map, tid) { const s = map.get(tid); return s && s[s.length - 1]; }
function pop(map, tid, frame) {
    const s = map.get(tid);
    if (!s || s[s.length - 1] !== frame) { errors++; map.delete(tid); return; }
    s.pop(); if (!s.length) map.delete(tid);
}
function flush() { if (queue.length) { send({kind: 'batch', records: queue}); queue = []; } }
function expectedDelta(count, input, output) {
    // Match x86 NEG/SAR/AND, IMUL and subtraction, including INT_MIN wrap.
    const consumed = input & (((-input) | 0) >> 31);
    return Math.imul(count, (output - consumed) | 0);
}
function snapshot(bins, index, resourceIndex) {
    const begin = bins.readPointer(), end = bins.add(4).readPointer();
    const size = end.toUInt32() - begin.toUInt32();
    if (begin.isNull() || size < 0 || size % 12 || size / 12 > 65535 || index < 0 || index >= size / 12) throw Error('bin_bounds');
    const bin = begin.add(index * 12);
    if (bin.readS16() !== resourceIndex) throw Error('bin_identity_changed');
    return {pointer: bin.toString(), resourceIndex, binIndex: index, amount: bin.add(4).readS32(), capacity: bin.add(8).readS32()};
}
function finishPending(f) {
    const op = f.pending;
    if (!op) return;
    f.pending = null;
    try {
        op.after = snapshot(f.bins, op.before.binIndex, op.before.resourceIndex);
        if (op.after.pointer !== op.before.pointer) throw Error('bin_storage_changed');
        op.delta = op.after.amount - op.before.amount;
        op.expectedDeltaInt32 = expectedDelta(f.row.applyCount, op.input, op.output);
        op.matchesInt32 = op.after.amount === ((op.before.amount + op.expectedDeltaInt32) | 0);
        op.status = 'observed';
    } catch (e) { delete op.after; errors++; f.row.readErrors.push(String(e)); op.status = 'read-error'; }
}
const callbacks = {
    unitRun: {
        enter() {
            this.tid = Process.getCurrentThreadId(); this.frame = {target: false};
            push(parents, this.tid, this.frame);
            const unitSlot = arg(this.context, 0).toInt32();
            if (config.unitSlot !== null && config.unitSlot !== unitSlot) return;
            const game = arg(this.context, 4), unit = this.context.ecx, rule = arg(this.context, 2);
            const begin = game.add(0x554).readPointer(), end = game.add(0x558).readPointer();
            const size = end.toUInt32() - begin.toUInt32();
            if (begin.isNull() || size < 0 || size % 0x380 || size / 0x380 > 1000000 || unitSlot < 0 || unitSlot >= size / 0x380 || !unit.equals(begin.add(unitSlot * 0x380))) throw Error('unit_identity_bounds');
            const id = rule.readU32(), ruleIndex = arg(this.context, 1).toUInt32();
            if (config.ruleId !== null && id !== config.ruleId) return;
            const unitEntryId = unit.add(0x0C).readU32();
            if (config.unitEntryId !== null && unitEntryId !== config.unitEntryId) return;
            const key = game.toString() + ':' + unitSlot + ':' + unit.toString() + ':' + ruleIndex + ':' + id;
            if (!config.allTargets && target && key !== target) return;
            Object.assign(this.frame, {target: !config.requireInputPositive, kind: 'unit-local', key, game, unit, unitSlot, rule, ruleIndex, ruleId: id, unitEntryId});
        },
        leave() { if (this.frame) pop(parents, this.tid, this.frame); },
    },
    globalRun: {
        enter() {
            this.tid = Process.getCurrentThreadId(); this.frame = {target: false};
            push(parents, this.tid, this.frame);
            const game = arg(this.context, 3), rule = arg(this.context, 1);
            if (!this.context.ecx.equals(game.add(0x478))) return;
            const id = rule.readU32(), ruleIndex = arg(this.context, 0).toUInt32();
            if (config.ruleId !== null && id !== config.ruleId) return;
            const key = game.toString() + ':' + ruleIndex + ':' + id;
            if (!config.allTargets && target && key !== target) return;
            Object.assign(this.frame, {target: true, kind: 'global', key, game, rule, ruleIndex, ruleId: id});
        },
        leave() { if (this.frame) pop(parents, this.tid, this.frame); },
    },
    applyOps: {
        enter() {
            this.tid = Process.getCurrentThreadId(); this.frame = {active: false};
            push(updates, this.tid, this.frame);
            const p = top(parents, this.tid), f = this.frame;
            if (!p || (!p.target && !(p.kind === 'unit-local' && config.requireInputPositive))) { channels.applyOps.filtered++; return; }
            const isUnit = p.kind === 'unit-local';
            if (!this.context.esp.readPointer().equals(mod.base.add(isUnit ? 0x21de80 : 0x2189fe))) { channels.applyOps.filtered++; return; }
            const bins = arg(this.context, 2), game = arg(this.context, 3);
            if (!game.equals(p.game) || !bins.equals(isUnit ? p.unit.add(0x210) : game.add(0x478))) throw Error('resource_owner_mismatch');
            if (!p.target && p.kind === 'unit-local' && config.requireInputPositive) {
                const probeOps = arg(this.context, 1), probeCount = probeOps.readS32(), probeArray = probeOps.add(4).readPointer();
                if (probeCount < 0 || probeCount > 16 || (probeCount && probeArray.isNull())) { channels.applyOps.filtered++; return; }
                let consumes = false;
                for (let i = 0; i < probeCount; i++) {
                    const op = probeArray.add(i * 12);
                    if ((config.resourceId === null || op.readU32() === config.resourceId) && op.add(4).readS32() > 0) consumes = true;
                }
                if (!consumes) { channels.applyOps.filtered++; return; }
                p.target = true;
                if (!config.allTargets) target = p.key;
            }
            if (!config.allTargets && target && target !== p.key) { channels.applyOps.filtered++; return; }
            if (total + reserved >= config.maxRecords) { dropped++; channels.applyOps.dropped++; return; }
            const ops = arg(this.context, 1), count = ops.readS32(), array = ops.add(4).readPointer();
            if (count < 0 || count > 16 || (count && array.isNull())) { channels.applyOps.filtered++; return; }
            const operations = [];
            for (let i = 0; i < count; i++) {
                const at = array.add(i * 12), output = at.add(8).readS32();
                const resourceId = at.readU32();
                operations.push({ordinal: i, resourceId: '0x' + resourceId.toString(16).padStart(8, '0'), input: at.add(4).readS32(), output, status: output < 0 ? 'not-mutated-range-condition' : 'not-reached'});
            }
            // Filter whole updates, never the operation sequence: native bin
            // lookups still visit every non-negative-output operation. Pruning
            // this array mislabels bins or reports spurious extra_lookup errors.
            if (!operations.length || (config.resourceId !== null && !operations.some(op =>
                op.resourceId === '0x' + config.resourceId.toString(16).padStart(8, '0')))) {
                channels.applyOps.filtered++; return;
            }
            const row = {channel: 'simulation.effect', schema: 1, phase: isUnit ? 'unit-local-resource-update' : 'global-resource-update',
                time: Date.now(), threadId: this.tid, game: game.toString(),
                ruleId: '0x' + p.ruleId.toString(16).padStart(8, '0'), ruleIndex: p.ruleIndex,
                simTicks: game.add(0x17a0).readU32(), applyCount: arg(this.context, 0).toInt32(),
                operations, readErrors: []};
            if (isUnit) Object.assign(row, {unitSlot: p.unitSlot, unitPointer: p.unit.toString(), unitStamp: null, resourceOwner: 'unit-local'});
            Object.assign(f, {active: true, bins, row, cursor: 0, pending: null});
            if (!config.allTargets) target = p.key; reserved++;
        },
        leave(rv) {
            const f = this.frame;
            if (!f) return;
            try {
                if (!f.active) return;
                finishPending(f);
                f.row.nativeSuccess = !!(rv.toUInt32() & 255);
                if (f.row.nativeSuccess && f.row.operations.some(op => op.status === 'not-reached')) {
                    errors++; f.row.readErrors.push('missing_lookup');
                }
                f.row.sequence = ++sequence;
                if (queue.length >= 256) flush();
                queue.push(f.row); total++; channels.applyOps.accepted++;
            } finally {
                if (f.active) reserved--;
                pop(updates, this.tid, f);
            }
        },
    },
    binFind: {
        enter() {
            const f = top(updates, Process.getCurrentThreadId());
            if (!f || !f.active || !this.context.esp.readPointer().equals(mod.base.add(0x2065d0))) return;
            if (!this.context.ecx.equals(f.bins)) throw Error('lookup_bins_mismatch');
            finishPending(f);
            while (f.cursor < f.row.operations.length && f.row.operations[f.cursor].output < 0) f.cursor++;
            if (f.cursor >= f.row.operations.length) throw Error('extra_lookup');
            this.frame = f; this.op = f.row.operations[f.cursor++];
            this.resourceIndex = arg(this.context, 0).toInt32();
        },
        leave(rv) {
            if (!this.frame) return;
            const index = rv.toInt32();
            if (index < 0) { this.op.status = 'bin-not-found'; return; }
            this.op.before = snapshot(this.frame.bins, index, this.resourceIndex);
            this.op.status = 'pending'; this.frame.pending = this.op;
        },
    },
};
function stop() {
    stopped = true; handles.forEach(h => h.detach()); handles = [];
    if (timer !== null) clearInterval(timer); timer = null;
    flush(); const incompleteRequests = reserved;
    parents.clear(); updates.clear();
    return {total, dropped, errors, channels, target, incompleteRequests};
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
                channels[name] = {hits: 0, accepted: 0, filtered: 0, dropped: 0, errors: 0};
                handles.push(Interceptor.attach(mod.base.add(h.rva), {
                    onEnter() { if (stopped) return; channels[name].hits++; try { callbacks[name].enter.call(this); } catch (e) { errors++; channels[name].errors++; const f = top(updates, Process.getCurrentThreadId()); if (f && f.active) f.row.readErrors.push(name + ':' + String(e)); } },
                    onLeave(rv) { if (stopped) return; try { callbacks[name].leave.call(this, rv); } catch (e) { errors++; channels[name].errors++; const f = top(updates, Process.getCurrentThreadId()); if (f && f.active) f.row.readErrors.push(name + ':' + String(e)); } },
                }));
            }
            timer = setInterval(flush, 100); Interceptor.flush();
            return {hooks: Object.keys(cfg.hooks), scope: cfg.requireInputPositive ? 'unit-resource-consumption' : (cfg.hooks.unitRun ? 'unit-local-resource-effects' : 'global-resource-effects')};
        } catch (e) { stop(); throw e; }
    }, stop,
};
