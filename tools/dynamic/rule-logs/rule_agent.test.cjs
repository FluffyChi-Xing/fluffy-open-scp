const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');

function harness(failSecond = false, file = 'rule_agent.js') {
  const source = readFileSync(join(__dirname, file), 'utf8');
  const state = { attached: 0, detached: 0, batches: [] };
  const context = {
    Process: { arch: 'ia32', getModuleByName: () => ({ base: { add: () => ({ readByteArray: () => new Uint8Array([0xaa]).buffer }) } }) },
    Interceptor: {
      attach() {
        state.attached++;
        if (failSecond && state.attached === 2) throw Error('second hook failed');
        return { detach() { state.detached++; } };
      },
      flush() {},
    },
    rpc: { exports: {} }, setInterval, clearInterval, send(batch) { state.batches.push(batch); },
  };
  vm.createContext(context);
  vm.runInContext(source, context);
  return { api: context.rpc.exports, state, context };
}

for (const file of ['rule_agent.js', 'provenance_agent.js', 'effects_agent.js', 'conditions_agent.js']) {
test(`${file}: rejects a mismatched second fingerprint before installing any hook`, () => {
  const { api, state } = harness(false, file);
  assert.throws(() => api.arm({ hooks: { global: { rva: 1, bytes: 'aa' }, unit: { rva: 2, bytes: 'bb' } } }), /entry_mismatch/);
  assert.equal(state.attached, 0);
});

test(`${file}: removes the first hook when installing the second hook fails`, () => {
  const { api, state } = harness(true, file);
  assert.throws(() => api.arm({ hooks: { global: { rva: 1, bytes: 'aa' }, unit: { rva: 2, bytes: 'aa' } } }), /second hook failed/);
  assert.equal(state.detached, 1);
  api.stop();
  assert.equal(state.detached, 1);
});
}

test('provenance does not attribute a discarded new object to a returned cache object', () => {
  const { context } = harness(false, 'provenance_agent.js');
  context.frame = { created: [{ resource: '0x100', database: { path: 'candidate.package' }, recordKey: { type: 0x08068aeb } }], cache: ['0x200'] };
  const result = vm.runInContext("classify(true, '0x200', frame)", context);
  assert.equal(result.provenance, 'cache-hit-origin-unknown');
  assert.equal(result.database, null);
  assert.equal(vm.runInContext("classify(true, '0x100', frame).provenance", context), 'created-from-record');
  assert.equal(vm.runInContext("classify(false, '0x100', frame).database", context), null);
});

test('unrelated nested requests hide parent provenance frames', () => {
  const { context } = harness(false, 'provenance_agent.js');
  vm.runInContext("stacks.set(1, [{target: true, id: 7}, {target: false}])", context);
  assert.equal(vm.runInContext('current(1)', context), null);
  vm.runInContext('stacks.get(1).pop()', context);
  assert.equal(vm.runInContext('current(1).id', context), 7);
});

test('synchronous provenance bursts flush bounded batches and preserve the total cap', () => {
  const { api, state, context } = harness(false, 'provenance_agent.js');
  api.arm({ maxRecords: 600, hooks: { record: { rva: 1, bytes: 'aa' } } });
  vm.runInContext("for (let i = 0; i < 650; i++) emit('record', {requestId: i + 1})", context);
  const summary = api.stop();
  assert.deepEqual(state.batches.map(b => b.records.length), [256, 256, 88]);
  assert.equal(summary.total, 600);
  assert.equal(summary.dropped, 50);
  assert.equal(summary.channels.record.dropped, 50);
  const rows = state.batches.flatMap(b => Array.from(b.records));
  assert.deepEqual(rows.map(r => r.sequence), Array.from({length: 600}, (_, i) => i + 1));
});

test('non-target record calls never dereference their key arguments', () => {
  const { context } = harness(false, 'provenance_agent.js');
  context.Process.getCurrentThreadId = () => 1;
  // Missing context would throw if arguments were accessed before filtering.
  assert.doesNotThrow(() => vm.runInContext('callbacks.record.enter.call({})', context));
});

test('global definitions decode bounded signed resource operations without game calls', () => {
  const { context } = harness();
  const memory = new Map();
  function pointer(value) {
    const address = Number(value);
    return {
      add: n => pointer(address + n), isNull: () => address === 0,
      equals: other => address === other.toUInt32(), toUInt32: () => address,
      readPointer: () => pointer(memory.get(address)),
      readU32: () => { assert.ok(memory.has(address), `unmapped ${address}`); return memory.get(address) >>> 0; },
      readS32: () => { assert.ok(memory.has(address), `unmapped ${address}`); return memory.get(address) | 0; },
    };
  }
  context.ptr = pointer;
  vm.runInContext('mod.base = ptr(0x400000)', context);
  memory.set(3000, 4000); memory.set(4000 + 0x40, 0x5f69c0);
  memory.set(2004, 5000); memory.set(2008, 5012);
  [0, 0, -1, 0, 1, -1, 0, 1, 1, 0].forEach((v, i) => memory.set(1000 + 4 * i, v));
  memory.set(5000, 0x98ec1f24); memory.set(5004, 4); memory.set(5008, -12);
  context.mapping = {rulePointer: '1000', ruleSetPointer: '2000', scriptsPointer: '3000'};
  const read = () => vm.runInContext('globalDefinition(mapping)', context);
  assert.equal(read().resources.values[0].output, -12);
  assert.equal(read().resources.values[0].resourceId, '0x98ec1f24');
  memory.set(1016, 17);
  assert.throws(read, /resource_ops_bounds/);
  memory.set(1016, 0); memory.set(1012, -1);
  assert.equal(read().resources.values.length, 0);
  memory.set(4000 + 0x40, 0);
  assert.throws(read, /store_lookup_mismatch/);
});
