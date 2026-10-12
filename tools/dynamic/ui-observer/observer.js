/* OpenSCP UI observer v1. ES5 for retail EAWebKit. No game calls, eval or queue reads. */
(function (root) {
    "use strict";
    if (root.__OpenSCPObserver) { return; }
    var config = root.__OpenSCPObserverConfig || {};
    var version = "1.0.0", sequence = 0, dropped = 0, errors = 0;
    var queue = [], queueBytes = 0, pending = null, xhr = null;
    var stopped = false, timer = null, busy = false, hooks = [];
    var nextSend = 0, failures = 0, lastHeartbeat = 0, acknowledged = 0;
    var page = String(new Date().getTime()) + "-" + String(Math.random()).slice(2);
    var own = Object.prototype.hasOwnProperty;

    // Do not invoke payload accessors or toJSON methods. Bound traversal and text.
    function snapshot(value) {
        var seen = [], nodes = 2048, chars = 32768;
        function copy(v, depth) {
            if (--nodes < 0 || chars <= 0) { return { $cut: "budget" }; }
            if (v === null || typeof v === "boolean" || typeof v === "number") { return v; }
            if (typeof v === "string") {
                var length = Math.min(v.length, 4096, chars);
                chars -= length;
                return length < v.length ? { $text: v.slice(0, length), $cut: "string", length: v.length } : v;
            }
            if (typeof v !== "object") { return { $type: typeof v }; }
            if (depth >= 10) { return { $cut: "depth" }; }
            for (var s = 0; s < seen.length; s++) { if (seen[s] === v) { return { $cut: "cycle" }; } }
            seen.push(v);
            var out = Object.create(null), count = 0, key, descriptor;
            if (Object.prototype.toString.call(v) === "[object Error]") {
                var errorKeys = ["name", "message", "stack"];
                for (var e = 0; e < errorKeys.length; e++) {
                    descriptor = Object.getOwnPropertyDescriptor(v, errorKeys[e]);
                    if (descriptor && own.call(descriptor, "value")) { out[errorKeys[e]] = copy(descriptor.value, depth + 1); }
                }
            }
            // Arrays use an explicit items envelope so truncation cannot look like a full array.
            var array = Object.prototype.toString.call(v) === "[object Array]";
            if (array) { out.$array = []; out.length = v.length; }
            for (key in v) {
                if (!own.call(v, key)) { continue; }
                if (++count > 128 || nodes <= 0 || chars <= 0) { out.$cut = "properties"; break; }
                descriptor = Object.getOwnPropertyDescriptor(v, key);
                var item = descriptor && own.call(descriptor, "value") ? copy(descriptor.value, depth + 1) : { $cut: "accessor" };
                if (array) { out.$array.push({ key: key, value: item }); }
                else if (key.length <= 256) { chars -= key.length; out[key] = item; }
                else { out.$cut = "key"; }
            }
            seen.pop();
            return out;
        }
        return copy(value, 0);
    }

    function record(channel, phase, name, payload, request) {
        if (stopped || busy) { return null; }
        busy = true;
        var id = ++sequence;
        try {
            var event = { sequence: id, time: new Date().getTime(), channel: channel,
                phase: phase, name: snapshot(name), payload: snapshot(payload), requestSequence: request || null };
            var line = JSON.stringify(event);
            if (line.length > 65536) {
                event.payload = { $cut: "record", characters: line.length };
                line = JSON.stringify(event);
            }
            while (queue.length && (queue.length >= 512 || queueBytes + line.length > 2097152)) {
                queueBytes -= queue.shift().length; dropped++;
            }
            queue.push(line); queueBytes += line.length;
        } catch (ignore) { errors++; }
        busy = false;
        return id;
    }

    function wrap(target, key, channel, resultIndex, errorIndex) {
        var original = target[key];
        if (typeof original !== "function") { throw new Error("missing bridge method: " + key); }
        function wrapped() {
            if (stopped) { return original.apply(this, arguments); }
            var args = Array.prototype.slice.call(arguments);
            var name = args[0];
            var id = record(channel, channel === "event" ? "receive" : "call", name,
                channel === "query" ? null : args[1]);
            function callbackAt(index, phase) {
                if (index < 0 || typeof args[index] !== "function") { return; }
                var callback = args[index];
                args[index] = function () {
                    record(channel, phase, name, Array.prototype.slice.call(arguments), id);
                    return callback.apply(this, arguments);
                };
            }
            callbackAt(resultIndex, "result");
            callbackAt(errorIndex, "error");
            try { return original.apply(this, args); }
            catch (error) {
                record(channel, "throw", name, error, id);
                throw error;
            }
        }
        target[key] = wrapped;
        hooks.push({ target: target, key: key, original: original, wrapped: wrapped });
    }

    function mode() {
        var ui = root.scrui && root.scrui.gUIManager;
        var manager = ui && ui.mRequestManager;
        return { clientAttached: !!(root.scrui && root.scrui.gClient), managerPresent: !!manager,
            nativeCallbacks: manager ? manager.mUseClientCallback : null,
            eventQueue: manager ? manager.mUseGameEventQueue : null };
    }

    function restore() {
        for (var i = hooks.length - 1; i >= 0; i--) {
            var hook = hooks[i];
            if (hook.target[hook.key] === hook.wrapped) { hook.target[hook.key] = hook.original; }
        }
    }

    function flush() {
        if (stopped || xhr || new Date().getTime() < nextSend || !config.endpoint) { return; }
        try {
            if (!pending) {
                var lines = [], size = 0;
                while (queue.length && lines.length < 32 && size + queue[0].length < 196608) {
                    var line = queue.shift(); queueBytes -= line.length; size += line.length; lines.push(line);
                }
                if (!lines.length) { return; }
                pending = JSON.stringify({ schema: "openscp.ui-trace.v1", version: version,
                    build: config.build, page: page, dropped: dropped, observerErrors: errors,
                    records: JSON.parse("[" + lines.join(",") + "]") });
            }
            var request = new root.XMLHttpRequest(), watchdog;
            xhr = request;
            function finish(ok) {
                if (xhr !== request) { return; }
                root.clearTimeout(watchdog); xhr = null;
                if (ok) { pending = null; failures = 0; acknowledged++; }
                else { failures++; nextSend = new Date().getTime() + Math.min(30000, 1000 * Math.pow(2, Math.min(failures, 5))); }
            }
            request.onreadystatechange = function () {
                try { if (request.readyState === 4) { finish(request.status === 204); } }
                catch (ignore) { finish(false); }
            };
            request.open("POST", config.endpoint, true);
            request.setRequestHeader("Content-Type", "text/plain;charset=UTF-8");
            watchdog = root.setTimeout(function () { finish(false); try { request.abort(); } catch (ignore) {} }, 3000);
            request.send(pending);
        } catch (ignore) { xhr = null; failures++; nextSend = new Date().getTime() + 30000; }
    }

    root.__OpenSCPObserver = {
        version: version,
        status: function () { return { page: page, stopped: stopped, queued: queue.length,
            dropped: dropped, observerErrors: errors, acknowledged: acknowledged, transportFailures: failures, mode: mode() }; },
        exportJSON: function () { return JSON.stringify({ schema: "openscp.ui-trace.v1", page: page,
            dropped: dropped, pending: pending ? JSON.parse(pending) : null, records: JSON.parse("[" + queue.join(",") + "]") }); },
        stop: function () {
            stopped = true; restore(); root.clearInterval(timer);
            if (xhr) { var current = xhr; xhr = null; try { current.abort(); } catch (ignore) {} }
        },
        flush: flush
    };
    try {
        wrap(root.scrui.cDataRequestManager.prototype, "PostGameCommand", "command", 2, -1);
        wrap(root.scrui.cDataRequestManager.prototype, "RequestGameData", "query", 1, 2);
        wrap(root.scrui.cUIManager.prototype, "ReceiveGameEvent", "event", -1, -1);
        record("observer", "ready", "bridge", { build: config.build, mode: mode() });
        timer = root.setInterval(function () {
            try {
                var now = new Date().getTime();
                if (now - lastHeartbeat >= 10000) { lastHeartbeat = now; record("observer", "heartbeat", "bridge", mode()); }
                flush();
            } catch (ignore) { errors++; }
        }, 250);
    } catch (error) { restore(); stopped = true; }
}(this));
