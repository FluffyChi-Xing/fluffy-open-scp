const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const {readFileSync}=require('node:fs');
const {join}=require('node:path');
function harness(){
  const c={Process:{getModuleByName:()=>({})},rpc:{exports:{}}};vm.createContext(c);
  vm.runInContext(readFileSync(join(__dirname,'conditions_agent.js'),'utf8'),c);
  return (fn,value)=>{c.value=value;return JSON.parse(JSON.stringify(vm.runInContext(`${fn}(value)`,c)));};
}
const op=(input,output,amount,capacity)=>({ordinal:0,status:'observed',input,output,bin:{amount,capacity}});
const row=(operations,resourceCount=0)=>({readErrors:[],nativeAllowed:false,control:{matches:true},finalApplyCount:0,countCapture:'captured',minApply:1,operations,resourceCount});
test('distinguishes input, capacity and both range bounds with exact native count agreement',()=>{
  const run=harness();
  for(const [o,reason] of [[op(5,0,4,10),'input-shortfall'],[op(0,2,9,10),'capacity-shortfall'],[op(5,-10,4,20),'range-below'],[op(5,-10,10,20),'range-above']]){
    assert.equal(run('diagnose',row([o])).reason,reason);
    assert.equal(run('diagnose',row([o],1)).reason,'unresolved');
  }
});
test('control flags and minimum apply are distinct from resource exhaustion',()=>{
  const run=harness();const r=row([]);r.control.matches=false;r.countCapture='not-observed';
  assert.equal(run('diagnose',r).reason,'control-flags');
  const m=row([op(2,0,4,10)],2);m.minApply=3;
  assert.equal(run('diagnose',m).reason,'minimum-apply');
});
test('zero configured maximum is not interpreted as disabled and -1 count means unconstrained',()=>{
  const run=harness();const r=row([op(0,0,5,10)],-1);r.nativeAllowed=true;r.finalApplyCount=-1;r.configuredMaxApply=0;
  assert.equal(run('diagnose',r).reason,'allowed');assert.equal(run('predictCount',r.operations).count,-1);
});
test('unobserved dynamic paths and malformed arithmetic stay unresolved',()=>{
  const run=harness();const r=row([]);r.countCapture='not-observed';r.fromBinOpsOffset=20;
  assert.equal(run('diagnose',r).reason,'unresolved');
  assert.equal(run('diagnose',row([op(-2147483648,1,1,10)])).reason,'unresolved');
  assert.equal(run('diagnose',row([op(0,1,11,10)])).reason,'unresolved');
});
test('early exit leaves later operations unknown without preventing a first-failure diagnosis',()=>{
  const run=harness();assert.equal(run('diagnose',row([op(5,0,1,10),{status:'not-observed'}])).reason,'input-shortfall');
  assert.equal(run('diagnose',row([{status:'not-observed'}])).reason,'unresolved');
  assert.equal(run('diagnose',row([{ordinal:0,status:'bin-not-found'}])).reason,'bin-missing');
});
test('read errors override otherwise plausible reasons',()=>{
  const run=harness();const r=row([op(5,0,1,10)]);r.readErrors=['fault'];assert.equal(run('diagnose',r).reason,'read-error');
});
test('unrelated nested frames hide active parent attribution',()=>{
  const c={Process:{getModuleByName:()=>({})},rpc:{exports:{}}};vm.createContext(c);
  vm.runInContext(readFileSync(join(__dirname,'conditions_agent.js'),'utf8'),c);
  assert.equal(vm.runInContext('push(checks,1,{active:true});push(checks,1,{active:false});top(checks,1).active',c),false);
});
