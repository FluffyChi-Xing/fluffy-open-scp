const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
test('mutation requests and results survive saturated query channels', () => {
  const memory = new Map(), hooks = [], sent = [];
  class Pointer {
    constructor(n) { this.n = Number(n); }
    add(n) { return new Pointer(this.n + n); }
    equals(p) { return this.n === p.n; }
    toString() { return '0x' + this.n.toString(16); }
    toUInt32() { return this.n; }
    isNull() { return !this.n; }
    readU32() { if (!memory.has(this.n)) throw Error('unreadable'); return memory.get(this.n); }
    readU8() { return this.readU32(); }
    readPointer() { return new Pointer(this.readU32()); }
    readS32() { return this.readU32(); }
    readUtf16String(n) { return this.readU32().slice(0,n); }
    readByteArray(n) { return new Uint8Array(n).buffer; }
  }
  const ptr = n => new Pointer(n);
  const config = { exeModule: 'fixture', dllModule: 'fixture', invokeRva: 10, logRva: 20,
    invokeBytes: '00', logBytes: '00', valueVtableRva: 30, seconds: 60,
    channelLimits: { query: 1, result: 1 },
    bindings: [{object: '0x64', vtable: '0xc8', host: '0x12c', page: '0x190', methods: ['RequestGameData']}] };
  memory.set(100,200); memory.set(104,300);
  const context = vm.createContext({globalThis: {OPEN_SCP_NATIVE_CONFIG: config},
    Process: { arch: 'ia32', getModuleByName: () => ({base:ptr(1000)}) }, ptr,
    Interceptor: {attach: (p, cb) => {hooks.push(cb);return {detach(){}};},flush(){}},
    send: x => sent.push(x),recv(){},setInterval: () => 1,setTimeout: () => 2,clearInterval(){},clearTimeout(){},rpc:{exports:{}} });
  vm.runInContext(fs.readFileSync(__dirname+'/native_agent.js','utf8'),context);
  // Valid wrapper -> flat JSString -> StringImpl -> UTF16 buffer.
  for (const [a,v] of [[5004,6000],[5008,7000],[5012,1],[5016,8000],[7000,1030],[7008,9000],[7012,0xfffffffb],[9004,10000],[9012,11000],[9016,0],[10008,5],[11008,12000],[8000,1030],[8008,1],[8012,0xffffffff]]) memory.set(a,v);
  [...'RequestGameData\0'].forEach((x,i)=>memory.set(6000+i,x.charCodeAt(0)));
  function invoke(text) {
    memory.set(9008,text.length);memory.set(11004,text.length);memory.set(12000,text);
    const call={context:{ecx:ptr(100),esp:ptr(5000)},threadId:1};
    hooks[0].onEnter.call(call);hooks[0].onLeave.call(call,ptr(1));
  }
  for(let i=0;i<100;i++)invoke('resourceAmount/123');
  for(const name of ['SaveLocalGame','UpdateBoxName/71/test','SaveRegionAs/9/copy/true','UpdateRegionName/9/test'])invoke(name);
  invoke('SaveLocalGameExtra');
  // A 14-element template array is complete, while normal query limits stay unchanged.
  config.arrayVtableRva = 500;
  for (const [a,v] of [[8008,15000],[8012,0xfffffffb],[15000,1500],[15000+0x30,14],[15000+0x38,16000],[16000,14],[16008,0],[16012,0]]) memory.set(a,v);
  for(let i=0;i<14;i++){memory.set(16000+0x18+i*8,i);memory.set(16000+0x1c+i*8,0xffffffff);}
  invoke('RegionTemplates');
  invoke('CreateRegion/test/248/true');
  invoke('CreateBox/9/71/test');
  invoke('GetRegionDetails/9');
  invoke('ActiveCityUID');
  invoke('LatestBox'); // independent region quota is also bounded
  invoke('CreateRegionExtra');
  // Rebinding rejects a stale reverse pointer, then accepts a coherent replacement.
  config.eventRva = 40;
  const replacement = {object:'0x1f4',vtable:'0xc8',host:'0x12c',view:'0x4b0',private:'0x514',page:'0x190',methods:['RequestGameData']};
  assert.equal(context.rpc.exports.addbinding(replacement),false);
  for(const [a,v] of [[500,200],[504,300],[300+0x158,500],[300+0xb0,1200],[1204,1300],[1300,1200],[1304,400],[1300+0x234,300],[208,1010],[300,1400],[1400+0x50,1040]]) memory.set(a,v);
  assert.equal(context.rpc.exports.addbinding(replacement),true);
  memory.set(504,999);
  assert.equal(context.rpc.exports.addbinding(replacement),false);
  context.rpc.exports.stop();
  const final = sent.find(x=>x.kind==='stopped');
  assert.equal(final.channels.query.dropped,101);
  assert.equal(final.channels.mutation.accepted,4);
  assert.equal(final.channels.mutationResult.accepted,4);
  assert.equal(final.channels.mutation.dropped,0);
  const writes=final.records.filter(x=>x.samplingChannel==='mutation');
  const results=final.records.filter(x=>x.samplingChannel==='mutationResult');
  assert.deepEqual(results.map(x=>x.callSequence),writes.map(x=>x.sequence));
  assert.ok(writes.every(x=>x.operationKind==='mutation'));
  const template = final.records.find(x=>x.samplingChannel==='templateResult');
  assert.equal(template.result.length,14);
  assert.equal(template.result.items.length,14);
  assert.equal(template.result.truncated,false);
  assert.equal(template.result.observation.deadlineReached,false);
  const regions=final.records.filter(x=>x.samplingChannel==='region');
  const regionResults=final.records.filter(x=>x.samplingChannel==='regionResult');
  assert.equal(regions.length,4);
  assert.equal(final.channels.region.dropped,1);
  assert.deepEqual(regionResults.map(x=>x.callSequence),regions.map(x=>x.sequence));
  assert.ok(regions.slice(0,2).every(x=>x.operationKind==='mutation'));
  assert.ok(regions.slice(2).every(x=>x.operationKind===undefined));
  assert.ok(regionResults.every(x=>x.result.items.length===14 && x.result.observation.profile==='region-lifecycle'));
  assert.equal(final.errors,0);
});
