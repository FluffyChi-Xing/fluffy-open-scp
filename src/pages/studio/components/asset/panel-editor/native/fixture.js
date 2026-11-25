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
 props[simcity.kPropWork_MinimumWorkersForProduction]=5;props[simcity.kPropWork_TotalJobs]=34;
 return {resources,resolve(q,fallback){
  const kind=q.includes('bins')?'bins':q.includes('binAmount')?'amount':q.includes('flags')?'flags':null,id=q.at(-1);
  if(kind==='bins'||kind==='amount'){
   if(missing.has(id))throw Error('PANEL_RESOURCE_MISSING:'+missing.get(id));
   let bin=resources[id];if(!bin){bin={name:'resource:'+id,amount:0,capacity:100,provenance:'explicit zero-valued synthetic supporting resource'};resources[id]=bin;}
   return kind==='bins'?{amount:bin.amount,capacity:bin.capacity}:bin.amount;
  }
  if(kind==='flags'){if(id===simcity.kUnitFlagOpenForBusiness)return state!=='closed';return [simcity.kUnitFlagNormal,simcity.kUnitFlagActiveBuilding,44].includes(id);}
  if(q.includes('properties')&&props[id]!==undefined)return props[id];
  return fallback;
 }};
};
