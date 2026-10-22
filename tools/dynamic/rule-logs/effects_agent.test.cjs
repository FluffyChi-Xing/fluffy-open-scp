const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const {readFileSync} = require('node:fs');
const {join} = require('node:path');

function harness(ops = [[5, 0, 1]], unitMode = false, requireInputPositive = false, resourceId = null) {
  const mem = new Map(), batches = [];
  function ptr(n) {
    n = Number(n) >>> 0;
    const read = () => { assert.ok(mem.has(n), `unmapped ${n.toString(16)}`); return mem.get(n); };
    return {add: v => ptr(n + v), equals: p => n === p.toUInt32(), isNull: () => n === 0,
      toUInt32: () => n, toInt32: () => n | 0, toString: () => '0x' + n.toString(16),
      readPointer: () => ptr(read()), readU32: () => read() >>> 0, readS32: () => read() | 0,
      readS16: () => (read() << 16) >> 16};
  }
  const ctx = {Process: {getModuleByName: () => ({base: ptr(0x400000)}), getCurrentThreadId: () => 1},
    rpc: {exports: {}}, send: b => batches.push(b), clearInterval() {}};
  vm.createContext(ctx); vm.runInContext(readFileSync(join(__dirname, 'effects_agent.js'), 'utf8'), ctx);
  vm.runInContext(`config={ruleId:null,resourceId:${resourceId},unitSlot:null,unitEntryId:null,maxRecords:10,requireInputPositive:${requireInputPositive ? 'true' : 'false'}};stopped=false;for(const n of ['globalRun','unitRun','applyOps','binFind'])channels[n]={hits:0,accepted:0,filtered:0,dropped:0,errors:0}`, ctx);
  function enter(name, args, ecx, ret = 0) {
    const stack = 0x9000 + (name === 'applyOps' ? 0x100 : name === 'binFind' ? 0x200 : 0);
    mem.set(stack, ret); args.forEach((a,i) => mem.set(stack + 4 + i * 4, a));
    const call = {context: {esp: ptr(stack), ecx: ptr(ecx)}};
    ctx.call = call; vm.runInContext(`callbacks.${name}.enter.call(call)`, ctx); return call;
  }
  function leave(name, call, result=1) { ctx.call=call;ctx.rv=ptr(result);vm.runInContext(`callbacks.${name}.leave.call(call,rv)`,ctx); }
  const game=0x10000,unit=0x7000,bins=unitMode ? unit+0x210 : game+0x478;
  mem.set(game+0x554,unit);mem.set(game+0x558,unit+0x380);mem.set(unit+0x0c,0x700c);
  mem.set(0x2000,0xc2181f55); mem.set(game+0x17a0,100);
  mem.set(0x3000,ops.length);mem.set(0x3004,0x4000);
  ops.forEach((op,i)=>op.forEach((v,j)=>mem.set(0x4000+i*12+j*4,v)));
  mem.set(bins,0x5000);mem.set(bins+4,0x500c);
  mem.set(0x5000,7);mem.set(0x5004,10);mem.set(0x5008,100);
  const parentName=unitMode?'unitRun':'globalRun';
  const parent=unitMode ? enter('unitRun',[0,368,0x2000,0x6000,game],unit) : enter('globalRun',[368,0x2000,0x6000,game],bins);
  const update=enter('applyOps',[1,0x3000,bins,game,0],0,unitMode?0x61de80:0x6189fe);
  const lookup=()=>{const c=enter('binFind',[7],bins,0x6065d0);leave('binFind',c,0);};
  const finish=(success=1)=>{leave('applyOps',update,success);leave(parentName,parent);ctx.rpc.exports.stop();return batches.flatMap(b=>Array.from(b.records))[0];};
  return {mem,ctx,lookup,finish,enter,leave,bins};
}
test('captures actual before/after and handles repeated resource operations sequentially',()=>{
  const h=harness([[5,0,1],[5,0,2]]);h.lookup();h.mem.set(0x5004,11);h.lookup();h.mem.set(0x5004,13);
  const row=h.finish();assert.deepEqual(Array.from(row.operations,o=>[o.before.amount,o.after.amount,o.delta]),[[10,11,1],[11,13,2]]);
  assert.ok(row.operations.every(o=>o.matchesInt32));assert.equal(row.nativeSuccess,true);
});
test('failure preserves a partial write and leaves later operations unobserved',()=>{
  const h=harness([[5,0,1],[5,0,2]]);h.lookup();h.mem.set(0x5004,11);
  const row=h.finish(0);assert.equal(row.nativeSuccess,false);assert.equal(row.operations[0].delta,1);assert.equal(row.operations[1].status,'not-reached');
});
test('negative output is not a negative resource mutation',()=>{
  const h=harness([[5,1,-20]]);const row=h.finish();assert.equal(row.operations[0].status,'not-mutated-range-condition');assert.equal(row.operations[0].before,undefined);
});
test('changed bin identity is an explicit error rather than a measured delta',()=>{
  const h=harness();h.lookup();h.mem.set(0x5000,8);const row=h.finish();assert.equal(row.operations[0].status,'read-error');assert.equal(row.operations[0].delta,undefined);assert.ok(row.readErrors.length);
});
test('successful return with missing lookup evidence is marked incomplete',()=>{
  const h=harness();const row=h.finish();assert.ok(row.readErrors.includes('missing_lookup'));assert.equal(row.operations[0].delta,undefined);
});
test('x86 arithmetic wraps explicitly instead of inventing a mathematical delta',()=>{
  const h=harness();assert.equal(vm.runInContext('expectedDelta(2,0,2147483647)',h.ctx),-2);h.finish();
});
test('unrelated nested updates cannot lend their bin lookups to the parent',()=>{
  const h=harness();const nested=h.enter('applyOps',[],0,0);
  h.lookup();h.leave('applyOps',nested);
  const row=h.finish();assert.equal(row.operations[0].before,undefined);assert.ok(row.readErrors.includes('missing_lookup'));
});
test('unit-local capture validates owner, preserves slot and does not invent a stamp',()=>{
  const h=harness([[5,2,0]],true);h.lookup();h.mem.set(0x5004,8);const row=h.finish();
  assert.equal(row.phase,'unit-local-resource-update');assert.equal(row.unitSlot,0);
  assert.equal(row.unitPointer,'0x7000');assert.equal(row.unitStamp,null);assert.equal(row.resourceOwner,'unit-local');
  assert.equal(row.operations[0].delta,-2);assert.equal(row.operations[0].matchesInt32,true);
});
test('unit context excludes global and zone update call sites',()=>{
  const h=harness([[5,0,1]],true);
  for(const ret of [0x6189fe,0x61def4,0x61df26,0x61df96]){
    const other=h.enter('applyOps',[],0,ret);h.lookup();h.leave('applyOps',other);
  }
  assert.equal(vm.runInContext('channels.applyOps.filtered',h.ctx),4);
  const row=h.finish();assert.equal(row.operations[0].before,undefined);assert.ok(row.readErrors.includes('missing_lookup'));
});
test('unit consumption scope filters zero-input production rules',()=>{
  const h=harness([[5,0,1]],true,true);h.lookup();const row=h.finish();
  assert.equal(row,undefined);assert.equal(vm.runInContext('channels.applyOps.filtered',h.ctx),1);
});
test('unit consumption scope accepts a positive-input operation',()=>{
  const h=harness([[5,2,1]],true,true);h.lookup();h.mem.set(0x5004,9);const row=h.finish();
  assert.equal(row.resourceOwner,'unit-local');assert.equal(row.operations[0].input,2);assert.equal(row.operations[0].delta,-1);
});
test('resource filter retains preceding lookups so the selected resource is not assigned another bin',()=>{
  const h=harness([[6,0,2],[5,0,1]],true,false,5);
  h.lookup();h.mem.set(0x5004,12);h.lookup();h.mem.set(0x5004,13);
  const row=h.finish();
  assert.deepEqual(Array.from(row.operations,o=>[o.ordinal,o.resourceId,o.delta]),[[0,'0x00000006',2],[1,'0x00000005',1]]);
  assert.equal(row.readErrors.length,0);
});
test('garbage condition followed by three companion updates retains all lookup ordinals',()=>{
  const h=harness([[5,0,-1],[6,10,0],[7,0,2],[8,0,3]],true,false,5);
  h.lookup();h.mem.set(0x5004,0);h.lookup();h.mem.set(0x5004,2);h.lookup();h.mem.set(0x5004,5);
  const row=h.finish();
  assert.equal(row.operations[0].status,'not-mutated-range-condition');
  assert.deepEqual(Array.from(row.operations.slice(1),o=>o.delta),[-10,2,3]);
  assert.equal(row.readErrors.length,0);
});
test('resource filter rejects an update with no matching resource',()=>{
  const h=harness([[6,0,2]],true,false,5);h.lookup();
  assert.equal(h.finish(),undefined);
});
