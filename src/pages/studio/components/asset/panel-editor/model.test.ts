import {describe,it,expect} from 'vitest';
import {availableComponents,clipSlot,insertComponent,panelComponents,itemName,nameHash,type PanelSchema} from './model';
import {panelEditorZh,panelEditorEn} from '@/locales/panel-editor';
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
 it('has labels in both languages for every registered component',()=>{for(const c of panelComponents){expect(panelEditorZh.components).toHaveProperty(c.id);expect(panelEditorEn.components).toHaveProperty(c.id);}});
 it('offers financial profit and business profit indices in the common library',()=>{
  const ids=availableComponents([]).map(c=>c.id);
  expect(ids).toContain('profit');expect(ids).toContain('profit-index');
  expect(panelComponents.find(c=>c.id==='profit-index')?.metric).toEqual({current:'ProfitToken',previous:'ProfitTokenLast'});
  expect(insertComponent(doc,'click','profit',0).click[0]).toBe('profit');
 });
});
