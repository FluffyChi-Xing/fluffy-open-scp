import KnowledgePage from '@/pages/knowledge/index.vue'
import type { RouteModule } from '@/router/types'

/** 知识库：游戏基础知识的双语介绍文档（assets/knowledge + FMarkdown）。 */
export default {
  routes: [
    {
      name: 'knowledge-file-types',
      path: 'knowledge/file-types',
      component: KnowledgePage,
      meta: {
        titleKey: 'navigation.knowledgeFileTypes',
        icon: 'filePen',
        groupKey: 'navigation.knowledge',
        order: 1,
      },
    },
    {
      name: 'knowledge-rendering',
      path: 'knowledge/rendering',
      component: KnowledgePage,
      meta: {
        titleKey: 'navigation.knowledgeRendering',
        icon: 'image',
        groupKey: 'navigation.knowledge',
        order: 2,
      },
    },
    {
      name: 'knowledge-engine',
      path: 'knowledge/engine',
      component: KnowledgePage,
      meta: {
        titleKey: 'navigation.knowledgeEngine',
        icon: 'boxes',
        groupKey: 'navigation.knowledge',
        order: 3,
      },
    },
  ],
} satisfies RouteModule
