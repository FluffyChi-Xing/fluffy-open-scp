import type { ExternalRouteEntry, NavigationGroup, NavigationItem, RouteModule, TemplateRoute } from '@/router/types'
import type { RouteRecordSingleView } from 'vue-router'

const modules = import.meta.glob<{ default: RouteModule }>('./routes/modules/*.ts', { eager: true })
const moduleList = Object.values(modules).map((module) => module.default)

export const appRoutes = moduleList.flatMap((module) => module.routes ?? [])
export const externalRoutes = moduleList.flatMap((module) => module.externalRoutes ?? [])
export const externalRouteByKey = new Map(externalRoutes.map((entry) => [entry.key, entry]))

const groupOrder = ['navigation.workspace', 'navigation.resources', 'navigation.modding', 'navigation.export', 'navigation.manage', 'navigation.knowledge']

function navigationItem(route: TemplateRoute): NavigationItem {
  return { key: route.name, titleKey: route.meta.titleKey, icon: route.meta.icon, routeName: route.name, path: `/${route.path}`.replace(/^\/$/, '/') }
}

export const topLevelNavigationItems: NavigationItem[] = appRoutes
  .filter((route) => !route.meta.groupKey && !route.meta.hideInMenu)
  .sort((left, right) => (left.meta.order ?? 0) - (right.meta.order ?? 0))
  .map(navigationItem)

export const navigationGroups: NavigationGroup[] = groupOrder.map((groupKey) => {
  const internal = appRoutes
    .filter((route) => route.meta.groupKey === groupKey && !route.meta.hideInMenu)
    .sort((left, right) => (left.meta.order ?? 0) - (right.meta.order ?? 0))
    .map(navigationItem)
  const external = externalRoutes
    .filter((entry) => entry.groupKey === groupKey && !entry.hideInMenu)
    .sort((left, right) => (left.order ?? 0) - (right.order ?? 0))
    .map((entry) => ({ key: entry.key, titleKey: entry.titleKey, icon: entry.icon, external: entry }))

  return { key: groupKey, titleKey: groupKey, items: [...internal, ...external] }
}).filter((group) => group.items.length)

export const leafNavigationItems = [...topLevelNavigationItems, ...navigationGroups.flatMap((group) => group.items)]

export function findExternalRoute(key: string): ExternalRouteEntry | undefined {
  return externalRouteByKey.get(key)
}

export function toRouteRecord(route: TemplateRoute): RouteRecordSingleView {
  return { path: route.path, name: route.name, component: route.component, meta: route.meta }
}
