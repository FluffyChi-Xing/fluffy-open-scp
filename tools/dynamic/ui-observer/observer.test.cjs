const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(`${__dirname}/observer.js`, 'utf8');

function setup(extra = '') {
  const context = vm.createContext({});
  vm.runInContext(`
    var intervals=[],timeouts=[],sent=[],throwValue={reason:'original'}, callbackThis={tag:'callback'};
    var __OpenSCPObserverConfig={build:'test-build',endpoint:'http://127.0.0.1:18765/capture/test'};
    function setInterval(fn){intervals.push(fn);return intervals.length;}
    function clearInterval(id){intervals[id-1]=null;}
    function setTimeout(fn){timeouts.push(fn);return timeouts.length;}
    function clearTimeout(id){timeouts[id-1]=null;}
    function XMLHttpRequest(){this.readyState=0;this.status=0;}
    XMLHttpRequest.prototype.open=function(method,url,async){this.async=async;};
    XMLHttpRequest.prototype.setRequestHeader=function(){};
    XMLHttpRequest.prototype.send=function(body){sent.push({body:body,request:this});};
    XMLHttpRequest.prototype.abort=function(){this.aborted=true;};
    var scrui={gClient:{},cDataRequestManager:function(){},cUIManager:function(){}};
    scrui.cDataRequestManager.prototype.PostGameCommand=function(name,body,callback){
      if(name==='throw'){throw throwValue;}
      if(callback){callback.call(callbackThis,body,'extra');}
      return this;
    };
    scrui.cDataRequestManager.prototype.RequestGameData=function(queries,callback,error){
      if(queries[0]==='error'){return error.call(callbackThis,'failure');}
      this.callback=callback;return 42;
    };
    scrui.cUIManager.prototype.ReceiveGameEvent=function(id,data){return data;};
    var originals=[scrui.cDataRequestManager.prototype.PostGameCommand,scrui.cDataRequestManager.prototype.RequestGameData,scrui.cUIManager.prototype.ReceiveGameEvent];
    var manager=new scrui.cDataRequestManager(),ui=new scrui.cUIManager();
    scrui.gUIManager={mRequestManager:manager};
    ${extra}
  `, context);
  vm.runInContext(source, context);
  return { context, run: code => vm.runInContext(code, context),
    records: () => JSON.parse(vm.runInContext('__OpenSCPObserver.exportJSON()', context)).records };
}

test('command/query behavior and callback exception identity', () => {
  const h = setup();
  assert.equal(h.run('manager.PostGameCommand("go",{value:1},function(a,b){this.received=[a,b]})===manager'), true);
  assert.equal(h.run('callbackThis.received[1]'), 'extra');
  assert.equal(h.run('manager.RequestGameData(["RegionTemplates"],function(a){this.received=a})'), 42);
  h.run('manager.callback.call(callbackThis,{cities:2})');
  assert.equal(h.run('callbackThis.received.cities'), 2);
  assert.equal(h.run('try{manager.PostGameCommand("throw")}catch(e){e===throwValue}'), true);
  assert.equal(h.run('manager.RequestGameData(["error"],function(){},function(){return this===callbackThis})'), true);
  assert.equal(h.run('try{manager.PostGameCommand("callbackThrows",null,function(){throw throwValue})}catch(e){e===throwValue}'), true);
  const records = h.records();
  const query = records.find(r => r.channel === 'query' && r.phase === 'call');
  assert.equal(records.find(r => r.channel === 'query' && r.phase === 'result').requestSequence, query.sequence);
  assert.equal(records.filter(r => r.channel === 'command' && r.phase === 'throw').length, 2);
});

test('snapshot ignores getters/toJSON, freezes data and bounds huge/cyclic objects', () => {
  const h = setup();
  h.run(`var value={x:1,toJSON:function(){throw Error('must not invoke')}};
    Object.defineProperty(value,'getter',{enumerable:true,get:function(){throw Error('must not invoke')}});
    value.self=value;ui.ReceiveGameEvent(246368015,value);value.x=9;`);
  const event = h.records().find(r => r.channel === 'event');
  assert.equal(event.payload.x, 1);
  assert.equal(event.payload.getter.$cut, 'accessor');
  assert.equal(event.payload.self.$cut, 'cycle');
  h.run('ui.ReceiveGameEvent(1,{huge:new Array(100000).join("a")})');
  assert.equal(h.records().at(-1).payload.huge.$cut, 'string');
  h.run('for(var i=0;i<2000;i++){ui.ReceiveGameEvent(i,{x:i})}');
  assert.equal(h.records().length, 512);
  assert.ok(h.run('__OpenSCPObserver.status().dropped') > 0);
});

test('duplicate install, rollback on missing method, stop and pending callbacks', () => {
  const h = setup();
  vm.runInContext(source, h.context);
  assert.equal(h.run('intervals.length'), 1);
  h.run('manager.RequestGameData(["later"],function(){return 7});__OpenSCPObserver.stop()');
  const count = h.records().length;
  assert.equal(h.run('manager.callback()'), 7);
  assert.equal(h.records().length, count);
  assert.equal(h.run('scrui.cDataRequestManager.prototype.PostGameCommand===originals[0]'), true);
  const broken = setup('delete scrui.cUIManager.prototype.ReceiveGameEvent;');
  assert.equal(broken.run('__OpenSCPObserver.status().stopped'), true);
  assert.equal(broken.run('scrui.cDataRequestManager.prototype.PostGameCommand===originals[0]'), true);
});

test('async transport retains failed batch, removes only acknowledged batch, stops in-flight', () => {
  const h = setup();
  h.run('__OpenSCPObserver.flush()');
  assert.equal(h.run('sent[0].request.async'), true);
  const batch = h.run('sent[0].body');
  h.run('sent[0].request.readyState=4;sent[0].request.status=500;sent[0].request.onreadystatechange()');
  assert.equal(h.run('JSON.stringify(JSON.parse(__OpenSCPObserver.exportJSON()).pending)'), batch);
  h.run('Date.prototype.getTime=function(){return 9999999999999};__OpenSCPObserver.flush()');
  assert.equal(h.run('sent[1].body'), batch);
  h.run('sent[1].request.readyState=4;sent[1].request.status=204;sent[1].request.onreadystatechange()');
  assert.equal(h.run('__OpenSCPObserver.status().acknowledged'), 1);
  assert.equal(JSON.parse(h.run('__OpenSCPObserver.exportJSON()')).pending, null);
  h.run('ui.ReceiveGameEvent(1,null);__OpenSCPObserver.flush();__OpenSCPObserver.stop()');
  assert.equal(h.run('sent[2].request.aborted'), true);
});
