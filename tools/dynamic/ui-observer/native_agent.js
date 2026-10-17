// Frida QuickJS agent, independent of the game's JavaScriptCore page context.
// No NativeFunction, page evaluation, event polling, pointer writes or replacement.
'use strict';

function createObserver(config) {
    if (Process.arch !== 'ia32') throw new Error('unsupported_arch');
    const game = Process.getModuleByName(config.exeModule);
    const webkit = Process.getModuleByName(config.dllModule);
    const invoke = game.base.add(config.invokeRva);
    const logger = webkit.base.add(config.logRva);
    const bindings = new Map(config.bindings.map(x => [x.object, x]));
    const hosts = new Map(config.bindings.map(b => [b.host, b]));
    const bindingTables = new Set(config.bindings.map(b => b.vtable));
    const pendingHosts = new Set();
    const pages = new Set(config.bindings.map(x => x.page));
    const listeners = [];
    const diagnostics = {unknownInvokeBinding: 0, unknownEventHost: 0, requestPrefixUnreadable: 0, bindingUpdates: 0};
    const mutations = new Set(['SaveLocalGame', 'SaveRegionAs', 'UpdateBoxName', 'UpdateRegionName', 'CreateRegion', 'CreateBox']);
    const regionRequests = new Set(['CreateRegion', 'CreateBox', 'GetRegionDetails', 'LatestBox', 'activeBoxId', 'ActiveCityUID', 'RegionID']);
    let sequence = 0, dropped = 0, errors = 0, hits = 0;
    let stopped = false, interval = null, timeout = null, awaiting = false;
    const errorExamples = [];
    const channels = {};
    for (const name of ['command', 'query', 'other', 'console', 'result', 'event', 'mutation', 'mutationResult', 'template', 'templateResult', 'region', 'regionResult']) {
        channels[name] = {queue: [], hits: 0, accepted: 0, dropped: 0, count: 0,
            start: Date.now(), limit: Math.min(config.channelMaxLimit || 100, Math.max(1,
                (config.channelLimits || {})[name] || (name.startsWith('region') ? 4 : name.startsWith('template') ? 2 : name === 'query' ? 40 : 80)))};
    }
    const valueTable = webkit.base.add(config.valueVtableRva);
    function unknownHost(host) {
        if (pendingHosts.size < 16) pendingHosts.add(host.toString());
    }
    function addBinding(binding) {
        if (stopped || bindings.size >= 64) return false;
        try {
            if (!bindingTables.has(binding.vtable) || !Array.isArray(binding.methods) || binding.methods.length > 512) return false;
            const object = ptr(binding.object), host = ptr(binding.host), view = ptr(binding.view), privateView = ptr(binding.private);
            if (!object.readPointer().equals(ptr(binding.vtable)) || !object.add(4).readPointer().equals(host) ||
                !host.add(0x158).readPointer().equals(object) || !host.add(0xb0).readPointer().equals(view) ||
                !view.add(4).readPointer().equals(privateView) || !privateView.readPointer().equals(view) ||
                !privateView.add(4).readPointer().equals(ptr(binding.page)) || !privateView.add(0x234).readPointer().equals(host) ||
                !object.readPointer().add(8).readPointer().equals(invoke) ||
                !host.readPointer().add(0x50).readPointer().equals(game.base.add(config.eventRva))) return false;
            const previous = bindings.get(binding.object), previousHost = hosts.get(binding.host);
            if (previous) hosts.delete(previous.host);
            if (previousHost) bindings.delete(previousHost.object);
            bindings.set(binding.object, binding); hosts.set(binding.host, binding);
            pages.clear();
            for (const current of bindings.values()) pages.add(current.page);
            diagnostics.bindingUpdates++;
            return true;
        } catch (_) { return false; }
    }
    function stats() {
        return Object.fromEntries(Object.entries(channels).map(([name, c]) =>
            [name, {hits: c.hits, accepted: c.accepted, dropped: c.dropped, queued: c.queue.length}]));
    }
    function drain(limit) {
        const rows = [];
        while (rows.length < limit) {
            const next = Object.values(channels).filter(c => c.queue.length)
                .sort((a, b) => a.queue[0].sequence - b.queue[0].sequence)[0];
            if (!next) break;
            rows.push(next.queue.shift());
        }
        return rows;
    }
    function failed(error) {
        errors++;
        if (errorExamples.length < 4) errorExamples.push(String(error).slice(0, 160));
    }
    function check(address, hex) {
        const actual = Array.from(new Uint8Array(address.readByteArray(hex.length / 2)))
            .map(x => x.toString(16).padStart(2, '0')).join('');
        if (actual !== hex) throw new Error('signature_mismatch');
    }
    function admit(channel) {
        hits++;
        const c = channels[channel], now = Date.now();
        c.hits++;
        if (now - c.start >= 1000) { c.start = now; c.count = 0; }
        if (++c.count > c.limit || c.queue.length >= (config.channelQueueLimit || 128)) {
            dropped++; c.dropped++; return false;
        }
        return true;
    }
    function append(channel, row) {
        row.sequence = ++sequence;
        row.time = Date.now();
        row.samplingChannel = channel;
        channels[channel].accepted++;
        channels[channel].queue.push(row);
        return row.sequence;
    }
    function argument(address, profile = null) {
        try {
            if (!address.readPointer().equals(valueTable)) return {type: 'unsupported', reason: 'wrapper_vtable'};
            const started = Date.now();
            const budget = profile ? {remaining: 12000, arrayLimit: 32, propertyLimit: 64, maxDepth: 12, deadline: started + 40}
                : {remaining: config.valueBudget === undefined ? 32 : config.valueBudget, arrayLimit: 8, propertyLimit: 12};
            const result = value(address.add(8), 0, budget);
            if (profile) result.observation = {profile, elapsedMs: Date.now() - started,
                valuesUsed: 12000 - budget.remaining, deadlineReached: !!budget.deadlineReached};
            return result;
        } catch (_) { return {type: 'unsupported', reason: 'unreadable'}; }
    }
    // Read only a short verified flat-string prefix before the ordinary query limiter.
    function requestPrefix(address) {
        try {
            if (!address.readPointer().equals(valueTable) || address.add(12).readU32() !== 0xfffffffb) return null;
            const cell = address.add(8).readPointer();
            if (cell.add(4).readPointer().add(8).readU8() !== 5 || cell.add(16).readU32()) return null;
            const length = cell.add(8).readU32(), impl = cell.add(12).readPointer();
            if (length > 1048576 || impl.isNull() || impl.add(4).readU32() !== length) return null;
            const text = length ? impl.add(8).readPointer().readUtf16String(Math.min(length, 32)) : '';
            const slash = text.indexOf('/');
            if (slash < 0 && length > 32) return null;
            return slash < 0 ? text : text.slice(0, slash);
        } catch (_) { return null; }
    }
    function objectValue(cell, depth, budget) {
        if (depth >= (budget.maxDepth ?? (config.objectDepth === undefined ? 3 : config.objectDepth))) return {type: 'object', status: 'depth_limit'};
        const shape = cell.add(4).readPointer(), storage = cell.add(8).readPointer();
        const capacity = shape.add(0x38).readU32();
        if (capacity > 256) return {type: 'object', status: 'capacity_limit'};
        // Never traverse prototypes or invoke property accessors.
        if (shape.add(0x44).readU8() & 2) return {type: 'object', status: 'accessors_skipped'};
        const properties = [], seen = new Set(), shapes = new Set();
        let cursor = shape, complete = false;
        function add(key, offset) {
            if (offset >= capacity) throw new Error('invalid_property_offset');
            const length = key.add(4).readU32();
            if (!length || length > 128) throw new Error('invalid_property_key');
            const name = key.add(8).readPointer().readUtf16String(length);
            if (seen.has(name)) return;
            seen.add(name);
            properties.push({name, value: value(storage.add(offset * 8), depth + 1, budget)});
        }
        for (let step = 0; step < (budget.propertyLimit + 4) && properties.length < budget.propertyLimit && budget.remaining > 0; step++) {
            if (cursor.isNull()) { complete = true; break; }
            if (shapes.has(cursor.toString())) break;
            shapes.add(cursor.toString());
            const table = cursor.add(0x34).readPointer();
            if (!table.isNull()) {
                const size = table.readU32(), used = table.add(12).readU32() + table.add(16).readU32();
                if (size < 16 || size > 512 || (size & (size - 1)) || used > size / 2)
                    return {type: 'object', status: 'invalid_property_table'};
                const entries = table.add(8).readPointer().add(size * 4);
                let i = 0;
                for (; i < used && properties.length < budget.propertyLimit && budget.remaining > 0; i++) {
                    const entry = entries.add(i * 16), key = entry.readPointer();
                    if (key.equals(ptr(1))) continue;
                    if (key.isNull()) throw new Error('null_property_key');
                    add(key, entry.add(4).readU32());
                }
                complete = i === used;
                break;
            }
            const offset = cursor.add(0x3c).readS8(), key = cursor.add(0x20).readPointer();
            if (offset < 0) { complete = true; break; }
            if (key.isNull()) return {type: 'object', status: 'missing_transition_key'};
            add(key, offset);
            cursor = cursor.add(0x1c).readPointer();
        }
        return {type: 'object', status: 'own_data', properties, truncated: !complete};
    }
    function value(payload, depth, budget) {
        try {
            if (budget.deadline && Date.now() >= budget.deadline) { budget.deadlineReached = true; return {type: 'unsupported', reason: 'time_budget'}; }
            if (--budget.remaining < 0) return {type: 'unsupported', reason: 'value_budget'};
            const tag = payload.add(4).readU32();
            if (tag === 0xffffffff) return {type: 'int32', value: payload.readS32()};
            if (tag === 0xfffffffe) return {type: 'boolean', value: !!payload.readU32()};
            if (tag === 0xfffffffd) return {type: 'null'};
            if (tag === 0xfffffffc) return {type: 'undefined'};
            if (tag < 0xfffffff9) {
                const number = payload.readDouble();
                return {type: 'double', value: Number.isFinite(number) ? number : String(number)};
            }
            if (tag !== 0xfffffffb) return {type: 'unsupported', reason: 'non_cell', tag};
            const cell = payload.readPointer();
            if (config.arrayVtableRva !== undefined &&
                cell.readPointer().equals(webkit.base.add(config.arrayVtableRva))) {
                if (depth >= (budget.maxDepth ?? (config.arrayDepth === undefined ? 2 : config.arrayDepth))) return {type: 'array', status: 'depth_limit'};
                const capacity = cell.add(0x30).readU32(), storage = cell.add(0x38).readPointer();
                const length = storage.readU32();
                if (length > 1048576 || capacity > 1048576)
                    return {type: 'unsupported', reason: 'invalid_array_length'};
                if (!storage.add(8).readPointer().isNull() || !storage.add(12).readPointer().isNull())
                    return {type: 'array', status: 'sparse_or_lazy', length};
                const items = [];
                for (let i = 0; i < Math.min(length, capacity, budget.arrayLimit); i++) {
                    const slot = storage.add(0x18 + i * 8);
                    items.push(slot.add(4).readU32() === 0xfffffffa ?
                        {type: 'hole'} : value(slot, depth + 1, budget));
                }
                return {type: 'array', status: 'dense', length, items, truncated: items.length < length};
            }
            if (config.objectVtableRva !== undefined &&
                cell.readPointer().equals(webkit.base.add(config.objectVtableRva)))
                return objectValue(cell, depth, budget);
            if (cell.add(4).readPointer().add(8).readU8() !== 5)
                return {type: 'unsupported', reason: 'non_string_cell'};
            const length = cell.add(8).readU32();
            if (length > 1048576) return {type: 'unsupported', reason: 'invalid_length', length};
            if (cell.add(16).readU32()) return {type: 'string', status: 'rope', length};
            const impl = cell.add(12).readPointer();
            if (impl.isNull() || impl.add(4).readU32() !== length)
                return {type: 'unsupported', reason: 'string_length_mismatch'};
            return {type: 'string', status: 'flat', length, truncated: length > 2048,
                text: length ? impl.add(8).readPointer().readUtf16String(Math.min(length, 2048)) : ''};
        } catch (_) { return {type: 'unsupported', reason: 'unreadable'}; }
    }
    function string16(reference) {
        const value = reference.readPointer();
        if (value.isNull()) return {text: '', truncated: false};
        const length = value.add(4).readU32();
        if (length > 1048576) throw new Error('invalid_string_length');
        return {text: length ? value.add(8).readPointer().readUtf16String(Math.min(length, 512)) : '',
                truncated: length > 512};
    }
    function methodName(address) {
        let name = '';
        for (let index = 0; index < 96; index++) {
            const byte = address.add(index).readU8();
            if (!byte) return name;
            if (byte > 127) throw new Error('invalid_method_name');
            name += String.fromCharCode(byte);
        }
        throw new Error('unterminated_method_name');
    }
    function flush() {
        if (awaiting) return;
        awaiting = true;
        const records = drain(config.batchLimit || 64);
        send({kind: 'batch', records, hits, dropped, errors, errorExamples, channels: stats(), diagnostics, candidateHosts: Array.from(pendingHosts)});
        pendingHosts.clear();
        recv('ack', function () { awaiting = false; });
    }
    function stop(reason) {
        if (stopped) return {hits, dropped, errors, stopped: true};
        stopped = true;
        if (interval !== null) clearInterval(interval);
        if (timeout !== null) clearTimeout(timeout);
        for (const listener of listeners) listener.detach();
        Interceptor.flush();
        // One final bounded message, including records not yet acknowledged.
        send({kind: 'stopped', reason, records: drain(config.finalBatchLimit || 128 * Object.keys(channels).length), hits, dropped, errors, errorExamples, channels: stats(), diagnostics});
        return {hits, dropped, errors, stopped: true};
    }
    check(invoke, config.invokeBytes);
    check(logger, config.logBytes);
    for (const binding of bindings.values()) {
        const object = ptr(binding.object);
        if (!object.readPointer().equals(ptr(binding.vtable)) ||
            !object.add(4).readPointer().equals(ptr(binding.host))) throw new Error('binding_changed');
    }
    if (config.eventRva !== undefined) {
        check(game.base.add(config.eventRva), config.eventBytes);
        for (const site of config.eventSites) check(game.base.add(site.rva), site.bytes);
    }
    try {
        if (config.eventRva !== undefined) {
            const callers = new Set(config.eventSites.map(site => game.base.add(site.returnRva).toString()));
            listeners.push(Interceptor.attach(game.base.add(config.eventRva), {
                onEnter() {
                    try {
                        const binding = hosts.get(this.context.ecx.toString());
                        if (!callers.has(this.returnAddress.toString())) return;
                        if (!binding) { diagnostics.unknownEventHost++; unknownHost(this.context.ecx); return; }
                        if (!admit('event')) return;
                        const stack = this.context.esp;
                        const argumentRef = stack.add(12).readPointer();
                        const event = argumentRef.isNull() ? {type: 'unavailable'} : argument(argumentRef.readPointer());
                        const id = event.type === 'array' ? (event.items || [])[0] : event;
                        append('event', {channel: 'event', name: 'ReceiveGameEventFromClient',
                            callerRva: this.returnAddress.sub(game.base).toString(),
                            binding: binding.object, page: binding.page, threadId: this.threadId,
                            eventID: id && ['int32', 'double'].includes(id.type) ? id.value : null, event});
                    } catch (error) { failed(error); }
                }
            }));
        }
        listeners.push(Interceptor.attach(invoke, {
            onEnter() {
                try {
                    const object = this.context.ecx;
                    const binding = bindings.get(object.toString());
                    if (!binding || !object.add(4).readPointer().equals(ptr(binding.host))) { diagnostics.unknownInvokeBinding++; unknownHost(object.add(4).readPointer()); return; }
                    const stack = this.context.esp;
                    const count = stack.add(12).readU32();
                    if (count > 256) throw new Error('invalid_argument_count');
                    const name = methodName(stack.add(4).readPointer());
                    if (!binding.methods.includes(name)) throw new Error('unregistered_method');
                    const args = stack.add(8).readPointer();
                    const request = name === 'RequestGameData' && count ? requestPrefix(args) : null;
                    if (name === 'RequestGameData' && count && request === null) diagnostics.requestPrefixUnreadable++;
                    const mutation = name === 'RequestGameData' && mutations.has(request);
                    const template = name === 'RequestGameData' && request === 'RegionTemplates';
                    const region = name === 'RequestGameData' && regionRequests.has(request);
                    const profile = template ? 'region-templates' : region ? 'region-lifecycle' : null;
                    const channel = template ? 'template' : region ? 'region' : mutation ? 'mutation' : name === 'PostGameCommand' ? 'command' :
                        name === 'RequestGameData' ? 'query' : 'other';
                    if (!admit(channel)) return;
                    const decoded = [];
                    for (let i = 0; i < Math.min(count, 4); i++) decoded.push(argument(args.add(i * 24)));
                    const callSequence = append(channel, {channel: 'invoke', arguments: decoded, argumentsTruncated: count > 4, name, argumentCount: count, requestName: request, operationKind: mutation ? 'mutation' : undefined,
                            threadId: this.threadId, binding: binding.object, page: binding.page});
                    if (name === 'RequestGameData') this.queryResult = {
                        resultChannel: template ? 'templateResult' : region ? 'regionResult' : mutation ? 'mutationResult' : 'result', requestName: request, mutation, profile,
                        address: stack.add(16).readPointer(), callSequence, name,
                        binding: binding.object, page: binding.page};
                } catch (error) { failed(error); }
            },
            onLeave(retval) {
                const pending = this.queryResult;
                if (!pending || stopped) return;
                try {
                    if (!admit(pending.resultChannel)) return;
                    const success = (retval.toUInt32() & 255) !== 0;
                    append(pending.resultChannel, {channel: 'result', name: pending.name, requestName: pending.requestName, operationKind: pending.mutation ? 'mutation' : undefined,
                        callSequence: pending.callSequence, binding: pending.binding,
                        page: pending.page, threadId: this.threadId, success,
                        result: success ? argument(pending.address, pending.profile) : {type: 'unavailable', reason: 'invoke_false'}});
                } catch (error) { failed(error); }
            }
        }));
        listeners.push(Interceptor.attach(logger, {
            onEnter() {
                try {
                    const page = this.context.ecx.add(4).readPointer().toString();
                    if (!pages.has(page) || !admit('console')) return;
                    const stack = this.context.esp;
                    append('console', {channel: 'console', threadId: this.threadId, page,
                            level: stack.add(12).readU32(), line: stack.add(20).readU32(),
                            message: string16(stack.add(16).readPointer()),
                            source: string16(stack.add(24).readPointer())});
                } catch (error) { failed(error); }
            }
        }));
        Interceptor.flush();
        interval = setInterval(flush, 250);
        timeout = setTimeout(() => stop('deadline'), config.seconds * 1000);
        send({kind: 'ready', hooks: listeners.length, bindings: bindings.size, runtime: 'QJS'});
        return {stop, addBinding};
    } catch (error) {
        stop('setup_failed');
        throw error;
    }
}

// Configuration is supplied by the fingerprint-checking Python launcher.
const observer = createObserver(globalThis.OPEN_SCP_NATIVE_CONFIG);
rpc.exports = {stop: () => observer.stop('requested'), addbinding: binding => observer.addBinding(binding)};
