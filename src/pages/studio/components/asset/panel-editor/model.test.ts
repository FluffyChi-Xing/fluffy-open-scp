import {describe,it,expect} from 'vitest';
import {availableComponents,clipSlot,insertComponent,panelComponents,itemName,nameHash,resolvePanelItems,type PanelSchema} from './model';
import {panelEditorZh,panelEditorEn} from '@/locales/panel-editor';
import schema from '@/lib/panel-schema.json';
import { unsupportedPanelItems } from './panelBuildDiagnostic';
describe('independent panel document',()=>{
 const doc:PanelSchema={$schema:'openscp.panel',schema_version:1,shell:'67771F5C:0B07405A:2BDCBFDD',hover:['park-feedback'],click:['maintenance','residents','park-visitors'],power:true,missingFeedback:'hide'};
 it('reorders content without changing the other view or original document',()=>{
  const moved=insertComponent(doc,'click','maintenance',3);expect(moved.click).toEqual(['residents','park-visitors','maintenance']);expect(moved.hover).toEqual(doc.hover);expect(doc.click[0]).toBe('maintenance');
 });
 it('offers only active behavior capabilities and keeps native IDs separate from schema identities',()=>{
  const available=availableComponents(['park','residence']);expect(available.some(c=>c.id==='residents')).toBe(true);expect(available.some(c=>c.id==='hotel-guests')).toBe(false);
  const feedback=panelComponents.find(c=>c.id==='residence-feedback')!;expect(nameHash(itemName(feedback,true))).not.toBe(nameHash(itemName(feedback,false)));
 });
 it('bounds selection by the visible content area rather than native authored width',()=>{expect(clipSlot({x:0,y:37,width:317,height:28},{x:7,y:40,width:265,height:206})).toEqual({x:7,y:40,width:265,height:25});});
 it('resolves saved service slots to the retail controls and emits feedback once', () => {
  const names = (items: string[]) => resolvePanelItems(items,panelComponents,true).map(row=>row.name);
  expect(names(['maintenance','fusion-generation','fusion-charge','fusion-controlnet','feedback'])).toEqual([
   'kFusionReactorFeedback','kMaintenance_Unconnected','kPowerProduction_Unconnected','kFusionReactorPowerStored',
   'kFusionReactorPowerUp','kPowerForLocalUseToggle','kAcademyConsumerCondensed',
  ]);
  expect(names(['maintenance','controlnet-workers','controlnet-production'])).toEqual([
   'kARFTextFeedback','OpenSCPPanel_maintenance','kAcademyWorkers','kAcademySkillGenerated','kAcademySkillForLocalUseToggle',
  ]);
  expect(names(['maintenance','feedback'])).toEqual(['OpenSCPPanel_maintenance','OpenSCPPanel_feedback_hide']);
 });
 it('has labels in both languages for every registered component',()=>{for(const c of panelComponents){expect(panelEditorZh.components).toHaveProperty(c.id);expect(panelEditorEn.components).toHaveProperty(c.id);}});
 it('accepts RCI controls in the schema and omits them from the managed container library', () => {
  const controls = ['land-value-residential', 'land-value-commercial', 'industrial-tech-level', 'density-happiness', 'density-profit',
   'residential-happiness-reasons', 'residential-sadness-reasons', 'commercial-happiness-reasons', 'commercial-sadness-reasons',
   'industrial-happiness-reasons', 'industrial-sadness-reasons'];
  expect(schema.$defs.items.items.enum).toEqual(panelComponents.map(component => component.id));
  expect(availableComponents([]).map(component => component.id)).toEqual(expect.arrayContaining(controls));
  expect(availableComponents([], true).filter(component => controls.includes(component.id))).toEqual([]);
 });
 it('offers only supported container controls and reads both new and historical errors', () => {
  expect(availableComponents(['tourism','commercial'], true).map(c=>c.id)).toEqual([
   'maintenance','feedback','profit','shoppers','tourists','commercial-happiness-reasons','commercial-sadness-reasons',
  ]);
  expect(availableComponents(['tourism','hotel'], true).map(c=>c.id)).toEqual(['maintenance','feedback','profit','hotel-occupancy']);
  expect(availableComponents(['office','commercial'], true, true).map(c=>c.id)).toEqual([
   'maintenance','feedback','profit','shoppers','commercial-happiness-reasons','commercial-sadness-reasons',
   'workers-by-wealth','mall-average-shoppers','tower-business-profit',
  ]);
  expect(unsupportedPanelItems('[panel] PANEL_CONTAINER_QUERY_UNSUPPORTED:hover:workers,click:workers,click:density-profit')).toEqual([
   {view:'hover',id:'workers'}, {view:'click',id:'workers'}, {view:'click',id:'density-profit'},
  ]);
  expect(unsupportedPanelItems('PANEL_CONTAINER_QUERY_UNSUPPORTED:profit')).toEqual([{id:'profit'}]);
 });
 it('offers financial profit and business profit indices in the common library',()=>{
  const ids=availableComponents([]).map(c=>c.id);
  expect(ids).toContain('profit');expect(ids).toContain('profit-index');
  expect(panelComponents.find(c=>c.id==='profit-index')?.metric).toEqual({current:'ProfitToken',previous:'ProfitTokenLast'});
  expect(insertComponent(doc,'click','profit',0).click[0]).toBe('profit');
 });
});
