import { createRouter, createWebHistory } from 'vue-router'
import DefaultLayout from '@/layouts/DefaultLayout.vue'
import ExternalFramePage from '@/pages/external/frame/index.vue'
import NotFoundPage from '@/pages/errors/not-found/index.vue'
import { appRoutes, toRouteRecord } from '@/router/registry'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: DefaultLayout,
      children: [
        ...appRoutes.map(toRouteRecord),
        { path: 'external/:key', name: 'external-frame', component: ExternalFramePage, meta: { titleKey: 'navigation.external', icon: 'external', activeMenu: 'example-frame' } }
      ]
    },
    { path: '/:pathMatch(.*)*', name: 'not-found', component: NotFoundPage }
  ]
})

export default router
