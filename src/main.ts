import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { CalendarDays } from 'lucide-vue-next'
import { registerIcons } from '@/lib/icons'
import { appConfig } from './config/app'
import { appEnv } from './config/env'
import { i18n } from './locales'
import App from './App.vue'
import router from './router'
import { useAppStore } from './stores/app'
import { createPermissionPlugin } from './directives'

import './styles/main.css'

registerIcons({ CalendarDays })

const pinia = createPinia()


router.afterEach((to) => {
  const app = useAppStore(pinia)
  if (!app.documentTitle) return
  const titleKey = to.meta.titleKey as string | undefined
  const pageTitle = titleKey ? i18n.global.t(titleKey) : ''
  const suffix = appEnv.title || appConfig.name
  document.title = pageTitle ? `${pageTitle} · ${suffix}` : suffix
})

createApp(App)
  .use(pinia)
  .use(router)
  .use(i18n)
  .use(createPermissionPlugin(appConfig.permission))
  .mount('#app')
