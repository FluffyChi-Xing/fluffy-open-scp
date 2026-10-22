'use strict';
// The only mutations to the target are Frida's two bounded interceptors.
const mod = Process.getModuleByName('SimCity.exe');
let handles = [], timer = null, config = null, stopped = true, sequence = 0;
let queue = [], total = 0, dropped = 0, errors = 0;
const channels = {}, targets = {};
function bytes(p, n) {
    return Array.from(new Uint8Array(p.readByteArray(n)), x => x.toString(16).padStart(2, '0')).join('');
}
function vector(at, stride, index) {
    const begin = at.readPointer(), end = at.add(4).readPointer();
    const size = end.toUInt32() - begin.toUInt32();
    if (size < 0 || size % stride || size / stride > 1000000 || index < 0 || index >= size / stride || begin.isNull())
        throw Error('vector_bounds:at=' + at + ':begin=' + begin + ':end=' + end + ':stride=' + stride + ':index=' + index);
    return begin.add(index * stride);
}
function ruleMapping(game, owner, stateIndex, kind) {
    const ruleIndex = vector(owner.add(kind === 'global' ? 0x98 : 0x1cc), 4, stateIndex).readU32();
    const scripts = game.add(0x10).readPointer();
    const expectedLookup = mod.base.add(kind === 'global' ? 0x1f6ae0 : 0x1f76b0);
    if (!scripts.readPointer().add(kind === 'global' ? 0xd8 : 0xd0).readPointer().equals(expectedLookup)) throw Error('scripts_vtable_mismatch');
    const setIndex = ruleIndex >>> 20, localIndex = ruleIndex & 0xfffff;
    const set = vector(scripts.add(0x5c), 4, setIndex).readPointer();
    const rule = kind === 'global' ? vector(set.add(0x54), 0x68, localIndex) : unitRule(set, localIndex);
    const ruleId = rule.readU32();
    const idMap = set.add(kind === 'global' ? 0x9c : 0x74);
    const begin = idMap.readPointer(), end = idMap.add(4).readPointer();
    const size = end.toUInt32() - begin.toUInt32();
    if (size < 0 || size % 8 || size / 8 > 1000000 || (size && begin.isNull())) throw Error('rule_id_map_bounds');
    let lo = 0, hi = size / 8;
    while (lo < hi) {
        const mid = Math.floor((lo + hi) / 2);
        if (begin.add(mid * 8).readU32() < ruleId) lo = mid + 1; else hi = mid;
    }
    const baseRuleWithinSet = lo < size / 8 && begin.add(lo * 8).readU32() === ruleId ? begin.add(lo * 8 + 4).readS32() : null;
    if (baseRuleWithinSet !== null && baseRuleWithinSet >= 0) {
        const baseRule = kind === 'global' ? vector(set.add(0x54), 0x68, baseRuleWithinSet) : unitRule(set, baseRuleWithinSet);
        if (baseRule.readU32() !== ruleId) throw Error('rule_id_roundtrip_mismatch');
    }
    // EcoRulesFromResource subtracts 0x144; Resource::mKey is at +8.
    // This is the loaded abstract resource key, NOT a winning package path.
    const resource = set.add(0x144);
    const resourceKey = {instance: resource.add(8).readU32(), type: resource.add(12).readU32(), group: resource.add(16).readU32()};
    if (resourceKey.type !== 0x08068ae9 || resourceKey.group !== (setIndex === 0 ? 0x40800200 : 0x40800201)) throw Error('rule_resource_key_mismatch');
    return {ruleIndex, ruleSetIndex: setIndex, ruleWithinSet: localIndex, resourceKey,
        ruleId, ruleIdHex: '0x' + ruleId.toString(16).padStart(8, '0'), baseRuleWithinSet,
        rulePointer: rule.toString(), scriptsPointer: scripts.toString(), ruleSetPointer: set.toString()};
}
function unitRule(set, index) {
    const start = set.add(0x20), finish = set.add(0x30);
    function iterator(at) {
        const current = at.readPointer(), begin = at.add(4).readPointer(), end = at.add(8).readPointer(), node = at.add(12).readPointer();
        const offset = current.toUInt32() - begin.toUInt32();
        if (begin.isNull() || node.isNull() || end.toUInt32() - begin.toUInt32() !== 624 || offset < 0 || offset >= 624 || offset % 156) throw Error('deque_iterator_bounds');
        return {node, offset: offset / 156};
    }
    const a = iterator(start), b = iterator(finish), distance = b.node.toUInt32() - a.node.toUInt32();
    if (distance < 0 || distance % 4 || distance > 1000000) throw Error('deque_map_bounds');
    const count = distance + b.offset - a.offset;
    if (index < 0 || index >= count) throw Error('deque_index_bounds');
    const offset = a.offset + index;
    return a.node.add(Math.floor(offset / 4) * 4).readPointer().add((offset % 4) * 156);
}
function selectedUnits(run) {
    const begin = run.add(4).readPointer(), end = run.add(8).readPointer();
    const size = end.toUInt32() - begin.toUInt32();
    if (size < 0 || size % 4 || size / 4 > 1000000 || (size && begin.isNull())) throw Error('selected_units_bounds');
    const values = [];
    for (let i = 0; i < Math.min(size / 4, 16); i++) values.push(begin.add(i * 4).readS32());
    return {values, count: size / 4, truncated: size / 4 > 16};
}
function globalDefinition(mapping) {
    const rule = ptr(mapping.rulePointer), set = ptr(mapping.ruleSetPointer);
    const scripts = ptr(mapping.scriptsPointer);
    if (!scripts.readPointer().add(0x40).readPointer().equals(mod.base.add(0x1f69c0))) throw Error('store_lookup_mismatch');
    // StoreForRule returns set+4; DataStore begins with its byte vector.
    const begin = set.add(4).readPointer(), end = set.add(8).readPointer();
    const size = end.toUInt32() - begin.toUInt32();
    const offset = rule.add(12).readS32(), count = rule.add(16).readS32();
    if (size < 0 || size > 67108864 || count < 0 || count > 1000000 ||
        (count && (begin.isNull() || offset < 0 || offset > size || count * 12 > size - offset))) throw Error('resource_ops_bounds');
    const values = [];
    for (let i = 0; i < Math.min(count, 16); i++) {
        const op = begin.add(offset + i * 12);
        values.push({resourceId: '0x' + op.readU32().toString(16).padStart(8, '0'), input: op.add(4).readS32(), output: op.add(8).readS32()});
    }
    return {flags: rule.add(4).readU32(), simInfoOffset: rule.add(8).readS32(),
        resources: {offset, count, values, truncated: count > 16},
        fromBinOps: {offset: rule.add(20).readS32(), count: rule.add(24).readS32()},
        minApply: rule.add(28).readS32(), maxApply: rule.add(32).readS32(),
        applyResourceId: '0x' + rule.add(36).readU32().toString(16).padStart(8, '0')};
}
function flush() {
    if (queue.length) { send({kind: 'batch', records: queue}); queue = []; }
}
function stop() {
    stopped = true;
    handles.forEach(h => h.detach()); handles = [];
    if (timer !== null) clearInterval(timer); timer = null;
    flush();
    return {total, dropped, errors, channels, targets};
}
function record(kind, context) {
    if (stopped) return;
    const count = channels[kind]; count.hits++;
    const game = context.ecx, stack = context.esp;
    try {
        const stateIndex = stack.add(kind === 'global' ? 4 : 8).readS32();
        const unitSlot = kind === 'unit' ? stack.add(4).readS32() : null;
        if (stateIndex < 0 || (unitSlot !== null && unitSlot < 0)) throw Error('negative_index');
        if ((config.stateIndex !== null && stateIndex !== config.stateIndex) ||
            (kind === 'unit' && config.unitSlot !== null && unitSlot !== config.unitSlot)) { count.filtered++; return; }
        // Pin to the first matching object and rule, not every unit/tick in the city.
        const key = game.toString() + ':' + unitSlot + ':' + stateIndex;
        if (!config.allTargets && targets[kind] && targets[kind] !== key) { count.filtered++; return; }
        if (total >= config.maxRecords || queue.length >= 256) { dropped++; count.dropped++; return; }
        const row = {channel: 'simulation.rule', schema: 1, phase: 'log-entry', scope: kind,
            sequence: ++sequence, time: Date.now(), threadId: Process.getCurrentThreadId(),
            game: game.toString(), stateIndex, unitSlot, unitStamp: null, readErrors: []};
        function read(name, operation) {
            try { row[name] = operation(); } catch (e) { errors++; row.readErrors.push(name + ':' + String(e)); }
        }
        read('simTicks', () => game.add(0x17a0).readU32());
        read('deltaStream', () => game.add(0x18f8).readPointer().toString());
        read('mapping', () => {
            const owner = kind === 'global' ? game.add(0x478) : vector(game.add(0x554), 0x380, unitSlot);
            return ruleMapping(game, owner, stateIndex, kind);
        });
        if (kind === 'unit') {
            read('unitEntryIndex', () => vector(game.add(0x554), 0x380, unitSlot).readS32());
            read('unitEntryId', () => '0x' + vector(game.add(0x554), 0x380, unitSlot).add(0x0C).readU32().toString(16).padStart(8, '0'));
        }
        if (kind === 'unit' && config.unitEntryId != null && row.unitEntryId !== ('0x' + config.unitEntryId.toString(16).padStart(8, '0'))) { count.filtered++; return; }
        if (config.ruleId != null && (!row.mapping || row.mapping.ruleId !== config.ruleId)) { count.filtered++; return; }
        if (!targets[kind]) targets[kind] = key;
        if (kind === 'global') {
            read('applyCount', () => vector(game.add(0x520), 0x2c, stateIndex).add(0x1c).readS32());
            if (row.mapping) read('ruleDefinition', () => globalDefinition(row.mapping));
        } else {
            const run = stack.add(12).readPointer();
            read('applyCount', () => run.readS32());
            read('maxApply', () => run.add(0x14).readS32());
            read('maxPossibleMapApply', () => run.add(0x18).readS32());
            read('selectedUnits', () => selectedUnits(run));
        }
        queue.push(row); total++; count.accepted++;
    } catch (_) { errors++; count.errors++; }
}
rpc.exports = {
    identity() { return {path: mod.path, base: mod.base.toString(), arch: Process.arch, pid: Process.id}; },
    arm(cfg) {
        if (config) throw Error('already_armed');
        if (Process.arch !== 'ia32') throw Error('expected_x86');
        // Validate EVERY entry before installing ANY interceptor.
        for (const h of Object.values(cfg.hooks)) {
            if (bytes(mod.base.add(h.rva), h.bytes.length / 2) !== h.bytes) throw Error('entry_mismatch');
        }
        config = cfg; stopped = false;
        try {
            for (const [kind, h] of Object.entries(cfg.hooks)) {
                channels[kind] = {hits: 0, accepted: 0, filtered: 0, dropped: 0, errors: 0};
                handles.push(Interceptor.attach(mod.base.add(h.rva), {onEnter() { record(kind, this.context); }}));
            }
            timer = setInterval(flush, 100);
            Interceptor.flush();
            return {hooks: Object.keys(cfg.hooks), phase: 'log-entry'};
        } catch (e) { stop(); throw e; }
    },
    stop,
};
