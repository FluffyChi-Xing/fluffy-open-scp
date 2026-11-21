import { command } from '@/api/tauri';
import type { PanelLibrary, PanelProject, PanelSchema, PanelDependency } from './model';
export const readPanelProject=(project:string)=>command<PanelProject>('panel_project_read',{project});
export const loadPanelLibrary=(project:string)=>command<PanelLibrary>('panel_library',{project});
export const writePanelProject=(project:string,request:{schema:PanelSchema|null; dependency?:PanelDependency; manifestRevision:string; schemaRevision:string|null})=>command<PanelProject>('panel_project_save',{project,request});
