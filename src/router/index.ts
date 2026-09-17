import { createRouter, createWebHistory } from "vue-router";
import DefaultLayout from "@/layouts/DefaultLayout.vue";
import ExternalFramePage from "@/pages/external/frame/index.vue";
import NotFoundPage from "@/pages/errors/not-found/index.vue";
import OnboardingPage from "@/pages/onboarding/index.vue";
import { appRoutes, toRouteRecord } from "@/router/registry";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      component: DefaultLayout,
      children: [
        ...appRoutes.map(toRouteRecord),
        {
          path: "external/:key",
          name: "external-frame",
          component: ExternalFramePage,
          meta: {
            titleKey: "navigation.external",
            icon: "external",
            activeMenu: "example-frame",
          },
        },
      ],
    },
    // 首次安装引导：全屏独立页，不套 DefaultLayout（同 not-found 先例）
    { path: "/onboarding", name: "onboarding", component: OnboardingPage },
    { path: "/:pathMatch(.*)*", name: "not-found", component: NotFoundPage },
  ],
});

export default router;
