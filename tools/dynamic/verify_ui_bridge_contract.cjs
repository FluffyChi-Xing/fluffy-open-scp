// Offline contract checks against extracted retail scrui; never attaches to a game.
// Usage: node tools/dynamic/verify_ui_bridge_contract.cjs <extracted-scrui.js>
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const bytes = fs.readFileSync(process.argv[2]);
assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),
  'c36316891708c6bf89c9530714704446dd20104f9e40420c461c6972257e776a',
  'Unexpected scrui version: re-audit extraction boundaries before executing');
const source = bytes.toString('utf8');
const context = vm.createContext({});
function extract(start, end) {
  const a = source.indexOf(start);
  const b = source.indexOf(end, a + start.length);
  assert.ok(a >= 0 && b > a, `Missing boundary: ${start}`);
  vm.runInContext(source.slice(a, b), context, { timeout: 1000 });
}
vm.runInContext(`
var attached=true, calls=[], scrui={
  IsClientAttached:function(){return attached}, kFallbackRestPath:'rest/',
  kCurrentHostPath:'/', Assert:function(ok){if(!ok)throw Error('assert')},
  SendXHR:function(url,options){calls.push({url:url,options:options})},
  Client:{ProfZone:function(name,fn){return fn},
    PostGameCommand:function(a,b){calls.push([a,b]);return 'reply'},
    RequestGameData:function(a){calls.push(a);return a==='missing'?undefined:{query:a}}},
  cUIManager:function(){}
};`, context);
extract('scrui.cDataRequestManager=function', 'scrui.cDataRequestManager.prototype.Init=');
extract('scrui.BuildCommand=function', 'scrui.cDataRequestManager.prototype.RequestConfigurationData=');
extract('scrui.cDataRequestManager.prototype.RequestGameData=', 'scrui.cDataRequestManager.prototype.RequestData=');
extract('scrui.BuildRequestQuery=function', 'scrui.RunCheat=');
extract('scrui.cUIManager.ReceiveGameEventFromClient=function', 'scrui.cUIManager.Private_KeyDown=');
// Optional observer must preserve the same contract when wrapping the actual retail methods.
if (process.argv[3]) {
  vm.runInContext(`
    function setInterval(){return 1} function clearInterval(){}
    scrui.cUIManager.prototype.ReceiveGameEvent=function(){};
  `, context);
  vm.runInContext(fs.readFileSync(process.argv[3], 'utf8'), context, { timeout: 1000 });
  assert.equal(vm.runInContext('__OpenSCPObserver.status().stopped', context), false);
}
function evaluate(code) { return vm.runInContext(code, context, {timeout:1000}); }
function plain(code) { return JSON.parse(evaluate(`JSON.stringify(${code})`)); }
assert.deepEqual(plain('(new scrui.cDataRequestManager())'), {
  mAsynchronous:false,mRequests:[],mUseClientCallback:true,mUseGameEventQueue:false,mRestPath:'rest/'
});
assert.equal(evaluate('scrui.BuildCommand(["regiondata",42])'), 'regiondata/42');
assert.equal(evaluate('scrui.BuildRequestQuery(["camera","a b"])'), 'camera/a b');
assert.equal(evaluate('scrui.RequestStringFromKey({group:1,instance:2,type:3})'), '1,2,3');
assert.equal(evaluate('scrui.RequestStringFromUnitRef({slot:7,stamp:9})'), '7,9');
assert.equal(evaluate('scrui.RequestStringFromVector([1,2,3])'), '1,2,3');
evaluate('var manager=new scrui.cDataRequestManager(), reply; var returned=manager.PostGameCommand("regiondata/42",undefined,function(r){reply=r})');
assert.equal(evaluate('reply'), 'reply');
assert.equal(evaluate('returned'), undefined);
assert.deepEqual(plain('calls'), [['regiondata/42',null]]);
evaluate('calls=[];manager.RequestGameData(["RegionTemplates","missing"],function(r){reply=r})');
assert.deepEqual(plain('reply'), [{query:'RegionTemplates'},null]);
assert.deepEqual(plain('calls'), ['RegionTemplates','missing']);
evaluate('attached=false;manager=new scrui.cDataRequestManager();calls=[];manager.PostGameCommand("regiondata/42","body");manager.RequestGameData(["RegionTemplates","camera"],function(r){reply=r})');
assert.equal(evaluate('manager.mUseGameEventQueue'), true);
assert.equal(evaluate('calls[0].url'), '/gamecommand/regiondata/42');
assert.equal(evaluate('calls[0].options.data'), 'body');
assert.equal(evaluate('calls[1].url'), '/gamedata/batch/');
assert.equal(evaluate('calls[1].options.data'), 'RegionTemplates\ncamera\n');
evaluate('calls[1].options.success({results:[1,2]})');
assert.deepEqual(plain('reply'), [1,2]);
evaluate('calls=[];scrui.gUIManager={mRequestManager:{mUseGameEventQueue:false},ReceiveGameEvent:function(id,data){calls.push([id,data])}};scrui.cUIManager.ReceiveGameEventFromClient(246368015,{regionBoxID:42});scrui.gUIManager.mRequestManager.mUseGameEventQueue=true;scrui.cUIManager.ReceiveGameEventFromClient(123,null)');
assert.deepEqual(plain('calls'), [[246368015,{regionBoxID:42}]]);
console.log('PASS: extracted retail JS contract (mock native/XHR sinks; no live capture)');
