import SettingsPage from '@/pages/SettingsPage.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'settings', path: 'settings', component: SettingsPage, meta: { titleKey: 'navigation.settings', icon: 'setting', groupKey: 'navigation.manage', order: 10 } }
  ]
} satisfies RouteModule
