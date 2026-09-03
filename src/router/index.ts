import { createRouter, createWebHistory } from 'vue-router'
import DefaultLayout from '@/layouts/DefaultLayout.vue'
import ExternalFramePage from '@/pages/ExternalFramePage.vue'
import LoginPage from '@/pages/LoginPage.vue'
import NotFoundPage from '@/pages/NotFoundPage.vue'
import { appRoutes, toRouteRecord } from '@/router/registry'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: DefaultLayout,
      children: [
        ...appRoutes.map(toRouteRecord),
        { path: 'workspace', redirect: { name: 'projects' } },
        { path: 'external/:key', name: 'external-frame', component: ExternalFramePage, meta: { titleKey: 'navigation.external', icon: 'external', activeMenu: 'example-frame' } }
      ]
    },
    { path: '/login', name: 'login', component: LoginPage, meta: { titleKey: 'navigation.login', hideInMenu: true, noAffix: true } },
    { path: '/:pathMatch(.*)*', name: 'not-found', component: NotFoundPage }
  ]
})

export default router
