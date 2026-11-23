import catalog from '@/lib/panel-components.json';
export type PanelMode = 'hover' | 'click';
export interface PanelSchema {
  $schema: 'openscp.panel'; schema_version: 1; shell: string;
  hover: string[]; click: string[]; power: boolean; missingFeedback: 'hide' | 'native';
}
export interface PanelDependency { schema: string; sources: Array<{path: string; sha256: string}>; runtime: string }
export interface PanelProject { schema: PanelSchema | null; schemaPath: string; dependency?: PanelDependency; manifestRevision: string; schemaRevision: string | null }
export interface PanelComponent { id: string; native: string; group: string; feedback?: boolean; counter?: boolean; singleUnit?: boolean; reasonList?: boolean; metric?: {current: string; previous: string} }
export interface PanelResource { type: number; group: number; instance: number; locale: string; path: string }
export interface PanelLibrary { schema: PanelSchema; dependency: PanelDependency; resources: PanelResource[]; gameScript: string; scruiScript: string; css: string; components: PanelComponent[]; contexts: Record<string, Record<string, unknown>>; nativeLocales?: Record<string,string> }
export interface Rect { x: number; y: number; width: number; height: number }
export interface PreviewReport { dimensions: [number, number]; contentBounds: Rect; items: Array<{id: number; name: string; visible: boolean; rect: Rect; error?: string}>; text: string; error?: string; powerRect?: Rect }
export const panelComponents: readonly PanelComponent[] = catalog;
export function availableComponents(leaves: readonly string[], managed = false): PanelComponent[] {
  if (managed) {
    const canReadTourists = leaves.includes('tourism') && !leaves.includes('hotel');
    return panelComponents.filter(c => c.id === 'maintenance' || (canReadTourists && c.id === 'tourists'));
  }
  return panelComponents.filter(c => c.group === 'common' || leaves.includes(c.group));
}
export function itemName(c: PanelComponent, hide: boolean): string {
  return `OpenSCPPanel_${c.id}${c.feedback ? hide ? '_hide' : '_native' : ''}`;
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
