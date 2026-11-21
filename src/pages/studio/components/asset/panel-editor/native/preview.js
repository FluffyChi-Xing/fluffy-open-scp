/* global scrui, simcity, gUIManager, createComboData, goog */
window.failures=[];
const hex=n=>(n>>>0).toString(16).toUpperCase().padStart(8,'0');
window.run=async function(kind='landmark',mode='extended',fixtureOverrides={}){
 try{
 const names=fixtureOverrides.__names||{};
 let p=fixtureOverrides.__panel;if(!p)throw Error('PANEL_PREVIEW_DOCUMENT_REQUIRED');window.sample=p;
 if(fixtureOverrides.__panel){p=fixtureOverrides.__panel;Object.assign(names,fixtureOverrides.__extraNames||{});window.sample=p;}
 document.querySelector('#caption').textContent=kind+' / '+mode+' — 本机包解析 / local package · 模拟统计 / simulated statistics';
 document.querySelector('#host').replaceChildren();
 scrui.gUIManager=new scrui.cUIManager;window.gUIManager=scrui.gUIManager;scrui.cProperties.InitializeData();
 if(fixtureOverrides.__nativeLocale)gUIManager.mLocaleManager.InitLocale(fixtureOverrides.__nativeLocale);
 scrui.Assert=(ok,msg)=>{if(!ok)console.warn('ASSERT',msg)};
 scrui.gUIManager.GetLayoutRepresentation=function(p,cb){let xhr=new XMLHttpRequest;xhr.open('GET',p,false);xhr.send();if(xhr.status!==200)throw Error('layout not found '+p);let data=JSON.parse(xhr.responseText.replace(/^\uFEFF/,''));if(cb)cb(data);return data;};
 let root=new scrui.cRootWindow(0,0,0,600,850,document.querySelector('#host'),0);gUIManager.mRootWindows.push(root);
 let url=`raw/${hex(p.layout.type_id)}_${hex(p.layout.group)}_${hex(p.layout.instance)}.json`,layout=gUIManager.LoadAndCacheLayout(url,root);
 window.panel=layout;const find=id=>layout.FindControlByID(id);
 // Panel bindings are installed by the shared versioned runtime.
 find(164191353)?.SetText(goog.string.htmlEscape(p.names[0]||kind));
 if(p.icon)find(168099868)?.SetWindowImage(`raw/${hex(p.icon.type_id)}_${hex(p.icon.group)}_${hex(p.icon.instance)}.png`);
 find(248008263)?.SetVisibility(mode==='extended');find(215183969)?.SetVisibility(false);find(215183970)?.SetVisibility(false);
 find(215184001)?.SetVisibility(false); // native alert container; fixture has no alerts
 const titleHeight=find(164191353).mHeight;
 find(212238278).SetPosition(find(212238278).mLeft,titleHeight);layout.SetPosition(0,0,layout.mWidth,titleHeight);
 let arr=p[mode]||[],container=find(212238278),result=[];window.nativeItems=[];
 simcity.gRolloverManager={mRolloverUI:{mCurrentUnitRef:{slot:1}}};simcity.audio.LastQueryMsg={mSlot:1};
 simcity.gGameTimeManager={GetElapsedGameTimeSeconds:()=>0};
 const queryLedger=new Map(),queryConflicts=[];const comboData=fixtureOverrides.__compose?createComboData(p,fixtureOverrides.__state||'normal',fixtureOverrides.__missingResources||[]):null;
 function dataFor(binding,method){let qs=binding[method]?.('1,0','1,0')||{},data={};for(let[k,q]of Object.entries(qs)){
  const type=q.includes('bins')?'bins':q.includes('flags')?'flags':q.includes('maps')?'maps':'scalar';let value=0;
  if(type==='bins')value={amount:0,capacity:0};
  if(type==='flags')value=[14,19,29,44].includes(q.at(-1));
  if(type==='maps')value={amount:50,capacity:100};
  if(/^(normal|activeBuilding|onFlag|openForBusiness|baseOpen|continuousWorkPlace|hasToggle|canEdit)$/.test(k))value=true;
  if(/^(wealth|density|happinessDisplay)$/.test(k))value=kind==='industrial'?2:3;
  if(/^(canAfford|shiftLength|workers|totalResidents|Student|SpotsForKids|token|tokenLast|defaulttoken|previouslyEducated)$/.test(k))value={amount:k==='token'?60:k==='tokenLast'?58:k==='workers'?20:100,capacity:200};
  if(k==='threshold')value=100;
  if(k==='maintCost')value={amount:kind==='park'?580:kind==='landmark'?450:400,capacity:1000};
  if(k==='TouristW1')value=type==='bins'?{amount:22,capacity:375}:22;if(k==='shoppers')value=22;if(k==='openShoppers')value=353;
  if(k==='hasToggle')value=false;
  if(k==='unitName'){let t=p.flat[0x09fb78cb]?.[0];value={tableID:t?.table_id||0,stringID:t?.instance_id||0,placeholder:p.names[0]||kind};}
  if(q.includes('nonSimProperties')||q.includes('properties')){let raw=p.flat[q.at(-1)];if(raw!==undefined)value=raw;}
  if(comboData)value=comboData.resolve(q,value);
  let resolved=Object.hasOwn(fixtureOverrides,k)?fixtureOverrides[k]:value;
  if(comboData){let key=JSON.stringify(q);if(queryLedger.has(key)&&JSON.stringify(queryLedger.get(key).value)!==JSON.stringify(resolved))queryConflicts.push({query:q,alias:k});if(!queryLedger.has(key))queryLedger.set(key,{query:q,value:resolved,aliases:[]});queryLedger.get(key).aliases.push(k);resolved=queryLedger.get(key).value;}
  data[k]=resolved;
 }return {qs,data};}
 for(let id of arr){let name=names[id],original=simcity.RolloverItems[name],binding=fixtureOverrides.__compose?Object.assign({},original):original,entry={id,name,layout:binding?.layoutPath};result.push(entry);
 try{if(!binding?.layoutPath)throw Error('no layoutPath');let item=gUIManager.LoadAndCacheLayout(binding.layoutPath,container),info={layout:item,dataBinding:binding};entry.size=[item.mWidth,item.mHeight];
  if(binding.stringData)item.FindControlByID(simcity.cRolloverUI.kTxtIDRolloverItemValue)?.SetLocaleText(new scrui.cLocaleString(binding.stringData.tableID,binding.stringData.stringID));
  if(binding.iconPath)item.FindControlByID(simcity.cRolloverUI.kWinIDRolloverItemIcon)?.SetWindowImage(binding.iconPath);
  const init=dataFor(binding,'initQueries');binding.init?.(info,init.data);const update=dataFor(binding,'updateQueries');entry.queries=update.qs;entry.fixture=update.data;binding.update?.(info,update.data);if(binding.minimize!==undefined)item.SetVisibility(!binding.minimize);
  if(fixtureOverrides.__hideMissingFeedback&&/TextFeedback/.test(name)){
   let active=update.data.normal?info.normalRollover:info.vacantRollover,locale=active?.FindControlByID(222285689)?.mLocaleString;
   if(locale&&!locale.mLoaded){
    const manager=gUIManager.mLocaleManager,tablePath=code=>'Locale/'+code+'/'+locale.mTableID+(/\.json$/i.test(locale.mTableID)?'':'.json');
    const attempted=[manager.mLocaleCode,...(manager.mLocaleCode==='en-us'?[]:['en-us'])];
    if(attempted.some(code=>!Object.hasOwn(manager.mCachedLocaleFiles,tablePath(code))))throw Error('PANEL_LOCALE_TABLE_UNAVAILABLE:'+locale.mTableID);
    item.SetVisibility(false);entry.hiddenReason={code:'missing-feedback-locale',table:locale.mTableID,string:locale.mStringID};
   }
  }
  entry.visible=item.mIsVisible;window.nativeItems.push({id,name,item,entry});
 }catch(e){entry.error=e.message;window.failures.push({kind,mode,name,error:e.message});}}
 let controller={mCurrentLayout:layout,mOriginalHeight:layout.mHeight,mSkyTowerGlobalInfoExists:false};simcity.cRolloverUI.prototype.UpdateItemPositions.call(controller);
 let menu=find(215191523);menu.SetVisibility(false);
 const popup=[];
 if(mode==='extended'){
  let selected=p.popup||[],defs=[];for(let id of selected){let name=Object.keys(simcity.PopupItems).find(n=>scrui.IDFromName(n)===id),b=simcity.PopupItems[name];if(!b)continue;let d=dataFor(b,'initQueries');try{let visible=!b.initTest||!!b.initTest(d.data);popup.push({id,name,visible});if(visible)defs.push(b);}catch(e){popup.push({id,name,error:e.message});}}
  if(defs.length){
   simcity.RequestDataBindingData=(q,b,c,d,cb)=>cb(b.map(binding=>dataFor(binding,'initQueries').data));
   let popupNative=new scrui.cPopupMenu;popupNative.SetPosition(0,0);popupNative.Init(menu,defs,{slot:1,sequence:0},null);
   menu.SetVisibility(true);let main=find(215183945);layout.SetPosition(0,0,layout.mWidth,layout.mHeight+menu.mHeight);main.SetPosition(main.mLeft,main.mTop,main.mWidth,main.mHeight-menu.mHeight);
  }
 }
 const inspectPower=()=>{let button=find(181820548);if(!button)return null;let layers=[];button.WalkChildren(c=>{if(c.mStyleList?.some(x=>/ring-(on|off)/.test(x)))layers.push({images:c.mStyleList,visible:c.mIsVisible});});return{selected:button.mIsSelected,enabled:button.mEnabled,state:button.mButtonState,layers};};
 const animationAudit=[{elapsedMs:0,power:inspectPower()}];
 gUIManager.Update(0);gUIManager.Update(34);gUIManager.Update(0);
 animationAudit.push({elapsedMs:34,power:inspectPower()});
 window.powerAudit=inspectPower;
 let geometryAudit=[];layout.WalkChildren(c=>{if(c.mLayoutPath?.includes('TextRightSmall')){let t=c.FindControlByID(164377449);geometryAudit.push({path:c.mLayoutPath,width:c.mWidth,textWidth:t?.mWidth,textStyle:t?.mTextStyle,computed:t?.mTextElement?getComputedStyle(t.mTextElement).cssText:null,dom:t?.mRootElement.outerHTML});}});
 let placements=[];for(let c of container.mChildren||[])if(c.mIsVisible){let source=window.nativeItems.find(n=>n.item===c);placements.push({id:source?.id,name:source?.name,path:c.mLayoutPath,x:c.mLeft,y:c.mTop,width:c.mWidth,height:c.mHeight});}
 const panelRect=layout.mRootElement.getBoundingClientRect();
 const contentRect=find(252147891)?.mRootElement.getBoundingClientRect();
 const contentBounds=contentRect?{x:contentRect.left-panelRect.left,y:contentRect.top-panelRect.top,width:contentRect.width,height:contentRect.height}:null;
 for(let n of window.nativeItems){const r=n.item.mRootElement.getBoundingClientRect();n.entry.rect={x:r.left-panelRect.left,y:r.top-panelRect.top,width:r.width,height:r.height};}
 const report={kind,mode,key:p.key,name:p.names,chain:p.chain,layout:p.layout,items:result,popup,animationAudit,geometryAudit,queryLedger:[...queryLedger.values()],queryConflicts,fixtureResources:comboData?.resources,placements,contentBounds,text:layout.mRootElement.innerText,dimensions:[layout.mWidth,layout.mHeight]};window.lastReport=report;
 document.querySelector('#notes').textContent=JSON.stringify(report,null,2);return report;
 }catch(e){window.failures.push({kind,mode,error:e.stack});document.querySelector('#notes').textContent=e.stack;return {error:e.stack};}
};
