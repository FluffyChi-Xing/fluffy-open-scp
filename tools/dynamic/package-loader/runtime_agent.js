'use strict';
const cfg=CONFIG;
const mod=Process.getModuleByName('SimCity.exe');
let seq=0, dropped=0, errors=0, rows=[], total=0;
const hits={}, handles=[], seen=new Set(), dbs={};
function hex(x){return x.toString(16).padStart(8,'0').toUpperCase();}
function emit(phase,data){if(rows.length>=2048||total>=15000){dropped++;return;} rows.push(Object.assign({sequence:++seq,time:Date.now(),thread:Process.getCurrentThreadId(),phase},data));total++;}
function flush(){if(rows.length){send({kind:'batch',records:rows});rows=[];}}
function text(p){try{if(p.isNull())return null;let s='';for(let i=0;i<1024;i++){const c=p.add(i*2).readU16();if(c===0)return s;s+=String.fromCharCode(c);}return s;}catch(e){errors++;return null;}}
function db(p){try{if(p.isNull())return null;const id=p.toString();let path=null;const vt=p.readPointer();if(vt.equals(mod.base.add(0x929e20)))path=text(p.add(32).readPointer());const item={pointer:id,vtableRva:vt.sub(mod.base).toString(),path:path||dbs[id]?.path||null};dbs[id]=item;return item;}catch(e){errors++;return {pointer:p.toString(),unreadable:true};}}
function key(p){try{return {instance:hex(p.readU32()),type:hex(p.add(4).readU32()),group:hex(p.add(8).readU32())};}catch(e){errors++;return null;}}
function wanted(k,kind){if(!k)return false;const id=k.type+':'+k.group+':'+k.instance;if(cfg.target)return id===cfg.target;if(seen.size>=500)return false;const s=kind+id;if(seen.has(s))return false;seen.add(s);return true;}
function arg(ctx,n){return ctx.esp.add(4+n*4).readPointer();}
function hook(name,enter,leave){const h=cfg.hooks[name],at=mod.base.add(h.rva);const got=Array.from(new Uint8Array(at.readByteArray(h.bytes.length/2))).map(x=>x.toString(16).padStart(2,'0')).join('');if(got!==h.bytes)throw new Error('entry_mismatch:'+name);handles.push(Interceptor.attach(at,{onEnter(){hits[name]=(hits[name]||0)+1;try{enter.call(this);}catch(e){errors++;}},onLeave(rv){try{if(leave)leave.call(this,rv);}catch(e){errors++;}}}));}
hook('directory',function(){this.id=arg(this.context,0).toUInt32();},function(rv){if([0xa02149,0xa0214b,0xa02151,0xa0214f,0x45962cc,0x6cbed39,0x6cbed3a,0x8f03c82,0x8f03c83].includes(this.id)){const path=text(rv),s='dir'+this.id+path;if(!seen.has(s)){seen.add(s);emit('directory',{id:hex(this.id),path});}}});
hook('scan',function(){emit('scan',{path:text(arg(this.context,1)),priority:arg(this.context,4).toInt32()});});
hook('admission',function(){this.database=arg(this.context,0);this.prio=arg(this.context,1).toInt32();},function(rv){emit('admission',{database:db(this.database),priority:this.prio,result:!!(rv.toUInt32()&255)});});
hook('register',function(){this.manager=this.context.ecx.toString();this.register=!!(arg(this.context,0).toUInt32()&255);this.database=arg(this.context,1);this.prio=arg(this.context,2).toInt32();},function(rv){emit('register',{manager:this.manager,database:db(this.database),register:this.register,priority:this.prio,result:!!(rv.toUInt32()&255)});});
hook('open',function(){this.database=this.context.ecx;this.access=arg(this.context,0).toUInt32();},function(rv){emit('open',{database:db(this.database),access:this.access,result:!!(rv.toUInt32()&255)});});
hook('find',function(){this.k=key(arg(this.context,0));this.take=wanted(this.k,'find');},function(rv){if(this.take)emit('find',{key:this.k,database:db(rv)});});
hook('findRecord',function(){this.k=key(arg(this.context,0));this.take=wanted(this.k,'findRecord');this.out=arg(this.context,1);},function(rv){if(this.take)emit('findRecord',{key:this.k,recordKey:rv.isNull()||this.out.isNull()?null:key(this.out),database:db(rv)});});
hook('record',function(){this.k=key(arg(this.context,0));this.take=wanted(this.k,'record');this.database=this.context.ecx;this.output=arg(this.context,1);},function(rv){if(this.take)emit('record',{key:this.k,database:db(this.database),materialize:!this.output.isNull(),result:!!(rv.toUInt32()&255)});});
rpc.exports={stop(){handles.forEach(h=>h.detach());flush();return {hits,dropped,errors,total,databases:Object.values(dbs)};}};
setInterval(flush,100);
Interceptor.flush();
send({kind:'ready',base:mod.base.toString(),path:mod.path,hooks:Object.keys(cfg.hooks)});
