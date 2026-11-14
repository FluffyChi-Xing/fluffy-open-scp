import type { RouteRecordRaw, RouteRecordSingleView } from 'vue-router'

export type IconName =
  | 'dashboard'
  | 'project'
  | 'deployment'
  | 'setting'
  | 'components'
  | 'chart'
  | 'icons'
  | 'table'
  | 'form'
  | 'feedback'
  | 'result'
  | 'tokens'
  | 'external'
  | 'rocket'
  | 'shield'
  | 'globe'
  | 'filePen'
  | 'image'
  | 'map'
  | 'panelTop'
  | 'boxes'
  | 'sticky'
  | 'history'
  | 'activity'

export type RouteMeta = RouteRecordRaw['meta'] & {
  titleKey: string
  icon?: IconName
  order?: number
  groupKey?: string
  hideInMenu?: boolean
  activeMenu?: string
  noAffix?: boolean
  external?: boolean
  /** 沉浸式页面：内容区去内边距并撑满视口（页内自管滚动），隐藏页脚。 */
  fullbleed?: boolean
}

export interface TemplateRoute {
  name: string
  path: string
  component: RouteRecordSingleView['component']
  meta: RouteMeta
}

export type RegisteredRoute = RouteRecordRaw

export interface ExternalRouteEntry {
  key: string
  titleKey: string
  url: string
  openMode: 'new-tab' | 'iframe'
  icon?: IconName
  groupKey?: string
  hideInMenu?: boolean
  order?: number
}

export interface RouteModule {
  routes?: TemplateRoute[]
  externalRoutes?: ExternalRouteEntry[]
}

export interface NavigationItem {
  key: string
  titleKey: string
  icon?: IconName
  routeName?: string
  path?: string
  external?: ExternalRouteEntry
}

export interface NavigationGroup {
  key: string
  titleKey: string
  items: NavigationItem[]
}
