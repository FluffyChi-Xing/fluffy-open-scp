import catalog from '@/lib/panel-components.json';
import nativeServiceRecipes from '@/lib/panel-native-service-items.json';
export type PanelMode = 'hover' | 'click';
export interface PanelSchema {
  $schema: 'openscp.panel'; schema_version: 1; shell: string;
  hover: string[]; click: string[]; power: boolean; missingFeedback: 'hide' | 'native';
}
export interface PanelDependency { schema: string; sources: Array<{path: string; sha256: string}>; runtime: string }
export interface PanelProject { schema: PanelSchema | null; schemaPath: string; dependency?: PanelDependency; manifestRevision: string; schemaRevision: string | null }
export interface PanelComponent { id: string; native: string; group: string; feedback?: boolean; counter?: boolean; singleUnit?: boolean; reasonList?: boolean; towerOperations?: boolean; metric?: {current: string; previous: string} }
export interface PanelResource { type: number; group: number; instance: number; locale: string; path: string }
export interface PanelLibrary { schema: PanelSchema; dependency: PanelDependency; resources: PanelResource[]; gameScript: string; scruiScript: string; serviceScript?: string | null; serviceConstants?: string | null; css: string; components: PanelComponent[]; contexts: Record<string, Record<string, unknown>>; nativeLocales?: Record<string,string> }
export interface Rect { x: number; y: number; width: number; height: number }
export interface PreviewReport { dimensions: [number, number]; contentBounds: Rect; items: Array<{id: number; name: string; visible: boolean; rect: Rect; error?: string}>; text: string; error?: string; powerRect?: Rect }
export const panelComponents: readonly PanelComponent[] = catalog;
export function panelLibraryGroup(component: PanelComponent, leaves: readonly string[]): string {
  if (component.id === 'tower-business-profit' || component.id === 'workers-by-wealth') {
    return 'building-stats';
  }
  if (component.id === 'profit-index' && leaves.includes('commercial')) return 'commercial';
  return component.group;
}
export function availableComponents(leaves: readonly string[], managed = false, mall = false): PanelComponent[] {
  if (managed) {
    const canReadTourists = leaves.includes('tourism') && !leaves.includes('hotel');
    const canReadShoppers = leaves.includes('commercial') && !leaves.includes('hotel');
    const office = leaves.includes('office');
    return panelComponents.filter(c => ['maintenance', 'feedback', 'profit'].includes(c.id) || (canReadTourists && c.id === 'tourists')
      || (leaves.includes('park') && ['park-feedback', 'park-hours', 'park-visitors-title', 'park-visitors'].includes(c.id))
      || (leaves.includes('residence') && ['residence-feedback', 'residents', 'land-value-residential', 'residential-happiness-reasons', 'residential-sadness-reasons'].includes(c.id))
      || (leaves.includes('hotel') && c.id === 'hotel-occupancy')
      || (leaves.includes('controlnet') && c.group === 'controlnet')
      || (leaves.includes('fusion-power') && c.group === 'fusion-power')
      || (leaves.includes('casino') && c.group === 'casino')
      || (canReadShoppers && c.id === 'shoppers') || (mall && c.id === 'mall-average-shoppers')
      || (canReadShoppers && ['commercial-happiness-reasons', 'commercial-sadness-reasons'].includes(c.id))
      || (office && c.id === 'workers-by-wealth')
      || (office && mall && !leaves.includes('hotel') && !leaves.includes('tourism') && c.id === 'tower-business-profit'));
  }
  const sharedBusiness = mall && leaves.includes('office') && leaves.includes('commercial')
    && !leaves.includes('hotel') && !leaves.includes('tourism');
  return panelComponents.filter(c => (!c.towerOperations || sharedBusiness)
    && (c.group === 'common' || leaves.includes(c.group)));
}
export function itemName(c: PanelComponent, hide: boolean): string {
  return `OpenSCPPanel_${c.id}${c.feedback ? hide ? '_hide' : '_native' : ''}`;
}
export function resolvePanelItems(items: readonly string[], all: readonly PanelComponent[], hide: boolean) {
  const fusion = items.find(id => id.startsWith('fusion-'));
  const station = items.find(id => id.startsWith('controlnet-'));
  const owner = items.includes('feedback') ? 'feedback' : fusion ?? station!;
  const status = fusion ? 'kFusionReactorFeedback' : station ? 'kARFTextFeedback' : null;
  const rows = items.flatMap(id => {
    const c = all.find(component => component.id === id);
    if (!c) throw Error('PANEL_COMPONENT_UNKNOWN:' + id);
    if (status && id === 'feedback') return [];
    const native = (nativeServiceRecipes as Record<string, string[]>)[id];
    const names = fusion && id === 'maintenance' ? ['kMaintenance_Unconnected'] : native ?? [itemName(c, hide)];
    return names.map(name => ({ id, name }));
  });
  return status ? [{ id: owner, name: status }, ...rows] : rows;
}
export function nameHash(name: string): number { let h=2166136261; for(const c of name.toLowerCase())h=(Math.imul(h,16777619)^c.charCodeAt(0))>>>0; return h; }
export function insertComponent(schema: PanelSchema, mode: PanelMode, id: string, index: number): PanelSchema {
  const old=schema[mode].indexOf(id),items=schema[mode].filter(v=>v!==id);
  const at=Math.max(0,Math.min(items.length,index-(old>=0&&old<index?1:0)));items.splice(at,0,id);
  return {...schema,[mode]:items};
}
export function clipSlot(rect: Rect, bounds: Rect): Rect {
  const x=Math.max(rect.x,bounds.x),y=Math.max(rect.y,bounds.y);
  return {x,y,width:Math.max(0,Math.min(rect.x+rect.width,bounds.x+bounds.width)-x),height:Math.max(0,Math.min(rect.y+rect.height,bounds.y+bounds.height)-y)};
}
