import type { RouteModule } from '@/router/types'

export default {
  externalRoutes: [
    { key: 'vue-docs', titleKey: 'navigation.vueDocs', icon: 'external', groupKey: 'navigation.resources', order: 10, url: 'https://vuejs.org/guide/introduction.html', openMode: 'new-tab' },
    { key: 'example-frame', titleKey: 'navigation.exampleFrame', icon: 'external', groupKey: 'navigation.resources', order: 20, url: 'https://example.com', openMode: 'iframe' }
  ]
} satisfies RouteModule
