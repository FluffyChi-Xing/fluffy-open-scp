// Executes only extracted rollover definitions in an isolated VM, without game access.
// Usage: node tools/audit-fire-rollover.cjs <extracted template_registry.js>
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const sourceBytes = fs.readFileSync(process.argv[2]);
const source = sourceBytes.toString('utf8');
const simcity = {};
// Restore literal constants, without executing the application's startup code.
for (const match of source.matchAll(/simcity\.([\w.]+)=(\d+|"[^"\r\n]*")(?=[;,])/g)) {
  const path = match[1].split('.');
  let parent = simcity;
  for (const name of path.slice(0, -1)) parent = parent[name] ??= {};
  parent[path.at(-1)] = JSON.parse(match[2]);
}
const assertions = [];
let replies = [];
const scrui = {
  cLocaleString: function (...args) { this.args = args; },
  Assert: (value, message) => { if (!value) assertions.push(message); },
  BuildRequestQuery: (query) => query,
  RequestGameData: (_, callback) => callback(replies),
  IDFromName: (name) => {
    let hash = 0x811c9dc5;
    for (const c of name.toLowerCase()) hash = (Math.imul(hash, 0x1000193) ^ c.charCodeAt(0)) >>> 0;
    return hash;
  },
};
const context = vm.createContext({ simcity, scrui, goog: { object: { extend: Object.assign } } });
const start = source.indexOf('simcity.RolloverItems={};');
const end = source.indexOf('simcity.InitRolloverItemDLC=function', start);
assert(start >= 0 && end > start);
vm.runInContext(source.slice(start, end), context, { timeout: 3000 });
const dlcStart = source.indexOf('simcity.GetDLCRolloverData=function', end);
const dlcEnd = source.indexOf('simcity.RolloverItems.kBlockAmusementParkUpgrade=', dlcStart);
assert(dlcStart >= 0 && dlcEnd > dlcStart);
vm.runInContext(source.slice(dlcStart, dlcEnd), context, { timeout: 3000 });
const buildStart = source.indexOf('simcity.Private_BuildDataBindingResults=function');
const buildEnd = source.indexOf('simcity.AddSimpleQueries=', buildStart);
assert(buildStart >= 0 && buildEnd > buildStart);
vm.runInContext(source.slice(buildStart, buildEnd), context, { timeout: 3000 });
const ids = [1324441806, 660140289, 2556773060, 1487209961, 2371164976, 2751945318, 574426973, 2017979723, 2327020226];
const names = new Map(Object.keys(simcity.RolloverItems).map(name => [scrui.IDFromName(name), name]));
const bindings = simcity.GenerateRolloverItemDataBindingMap();
const list = ids.map(id => {
  assert(bindings[id], `missing rollover ID ${id}`);
  const binding = bindings[id];
  return { id: id.toString(16), name: names.get(id), allowNullResults: !!binding.allowNullResults,
    initQueries: binding.initQueries?.('1,1', '1,1') ?? {},
    updateQueries: binding.updateQueries?.('1,1', '1,1') ?? {} };
});
let called = 0;
const binding = simcity.RolloverItems.kMaintenance;
replies = [null];
simcity.RequestDataBindingData(['maintenance'], [binding], [{ maintCost: [] }], false, () => called++);
assert.equal(called, 0, 'null prevents the whole callback');
assert(assertions.some(message => message.includes('Null data binding data: maintCost')));
replies = [{ amount: 0 }];
simcity.RequestDataBindingData(['maintenance'], [binding], [{ maintCost: [] }], false, () => called++);
assert.equal(called, 1, 'zero is valid data');
// Confirm that a valid neighbouring widget cannot rescue the rejected batch.
replies = [{ amount: 400 }, null];
simcity.RequestDataBindingData(['valid', 'missing'], [binding, binding],
  [{ maintCost: [] }, { maintCost: [] }], false, () => called++);
assert.equal(called, 1, 'one null cancels a batch containing valid results');
console.log(JSON.stringify({ source: process.argv[2],
  sha256: crypto.createHash('sha256').update(sourceBytes).digest('hex'),
  fireStationExtendedBindings: list, nullRejectsCallback: true, zeroAllowsCallback: true,
  oneNullRejectsMixedBatch: true, assertions }, null, 2));
