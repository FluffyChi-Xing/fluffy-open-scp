/* global simcity */
window.createComboData=function(panel,state,missingNames=[],sampleRci=false,sampleTower=false){
 const missing=new Map(missingNames.map(n=>[simcity['kResourceID'+n],n]));
 const resources={},set=(name,amount,capacity=100)=>{let id=simcity['kResourceID'+name];if(id!==undefined)resources[id]={name,amount,capacity,provenance:'explicit synthetic composition fixture'};};
 set('TotalResidents',40,60);set('TotalWorkers',25,34);set('ExpensesFromMaintenance',450,1000);
 set('TotalShoppers',14,175);set('OpenShoppers',161,175);
 for(let[w,amount]of [[1,8],[2,4],[3,2]])set('TouristW'+w,amount,175);
 set('GuestW2',12,68);set('OpenGuestsW2',56,68);for(let w of [1,3]){set('GuestW'+w,0,0);set('OpenGuestsW'+w,0,0);}
 set('HappinessToken',60,100);set('HappinessTokenLast',58,100);set('ProfitToken',60,100);set('ProfitTokenLast',58,100);
 set('CasinoSpecialtyCurrentMonthIncome',2400,100000);set('CasinoSpecialtyCurrentMonthExpenses',450,100000);
 set('CasinoSpecialtyLastMonthIncome',2100,100000);set('CasinoSpecialtyLastMonthExpenses',450,100000);
 resources[0x4E1450B0]={name:'IncomeFromCasinos',amount:168,capacity:100000,provenance:'explicit synthetic composition fixture'};
 set('ParkVisitorsW1ThisPeriod',7,100);set('ParkVisitorsW1PerPeriod',13,100);
 set('ParkGlobals_LocalsDisplay',9,100);set('ParkGlobals_KidsDisplay',3,100);set('HomelessDisplay',1,100);
 set('CanAfford',1,1);set('ShiftLength',8,8);set('Student',50,250);set('SpotsForKids',200,250);
 if(sampleRci){
  set('Wealth',2);set('Density',3);set('HappinessDisplay',3);
  set('HappinessFromPark',20);set('SadnessFromNoPower',12);
 }
 if(sampleTower){
  for(let [tier,workers,jobs] of [[1,80,100],[2,32,50],[3,0,0]]){
   set('WorkerW'+tier,workers);set('JobsW'+tier,jobs);
  }
  for(let [id,amount,capacity] of [[0x13014ACE,82,2400],[0x0735A595,4600,100000],
   [0x0D0B5542,1200,100000],[0xCD51E152,2700,100000],[0xCCF9FA95,3000,100000]]){
   resources[id]={name:'resource:'+id,amount,capacity,provenance:'tower operations preview fixture'};
  }
 }
 const props={};for(let name of ['kPropHappiness_HappinessTokensThresholdForDensityUpgrade','kPropProfit_ProfitTokensThresholdForDensityUpgrade'])props[simcity[name]]=100;
 props[0x0AFB9881]=13334;props[0x0C09DAA0]=400000;
 props[simcity.kPropPowerProducer_Rate]=2;
 if(state.indexOf('fusion-')===0){
  set('Power',state==='fusion-charging'?220000:state==='fusion-running'?400000:0,400000);
  set('FusionReactorCharging',state==='fusion-charging'?1:0,1);
  set('FusionPower',state==='fusion-running'?1:0,1);
  set('PowerToTransmit',state==='fusion-running'?13334:0,13334);
  set('AcademySkilled',1,1);set('AcademySkillNeeded',1250,1250);
 }
 for(let [id,amount,capacity] of [[0x3EB59739,13334,13334],[0x8E8B4E10,320000,400000],[0x5737C2E4,150,1250]]){
  if(resources[id])continue;
  resources[id]={name:'resource:'+id,amount,capacity,provenance:'fusion power preview fixture'};
 }
 props[0x0E2660C6]=68;
 props[0x0D084715]=2;
 props[0x0A7917C5]=25;props[0x0A7917C6]=50;props[0x0A7917C7]=100;props[0xAA5FE358]=5250;
 props[0x0FD16BE4]=6;props[0x0FF564C9]=1;props[0x0FF564CB]=2;props[0x0FF564CD]=4;
 if(!sampleTower){set('JobsW1',25);set('JobsW2',50);set('JobsW3',100);}
 for(let [id,amount] of [[0x97235AA7,25],[0x97235AA4,39],[0x97235AA5,75],[0xFCAB1E3F,403]]){
  resources[id]={name:'resource:'+id,amount,capacity:100000,provenance:'ControlNet preview fixture'};
 }
 props[simcity.kPropWork_MinimumWorkersForProduction]=5;props[simcity.kPropWork_TotalJobs]=34;
 return {resources,resolve(q,fallback){
  const kind=q.includes('bins')?'bins':q.includes('binAmount')?'amount':q.includes('flags')?'flags':null,id=q.at(-1);
  if(kind==='bins'||kind==='amount'){
   if(missing.has(id))throw Error('PANEL_RESOURCE_MISSING:'+missing.get(id));
   let bin=resources[id];if(!bin){bin={name:'resource:'+id,amount:0,capacity:100,provenance:'explicit zero-valued synthetic supporting resource'};resources[id]=bin;}
   return kind==='bins'?{amount:bin.amount,capacity:bin.capacity}:bin.amount;
  }
  if(kind==='flags'){
   if(id===simcity.kUnitFlagForcePowerShutDown)return state==='fusion-ready';
   if(id===simcity.kUnitFlagOpenForBusiness)return state!=='closed';
   return [simcity.kUnitFlagNormal,simcity.kUnitFlagActiveBuilding,44].includes(id);
  }
  if(q.includes('properties')&&props[id]!==undefined)return props[id];
  return fallback;
 }};
};
