import type { RouteModule } from "@/router/types";

export default {
  routes: [
    {
      name: "studio",
      path: "studio",
      component: () => import("@/pages/studio/index.vue"),
      meta: {
        titleKey: "navigation.studio",
        icon: "rocket",
        groupKey: "navigation.modding",
        order: 0,
      },
    },
    {
      name: "studio-project-detail",
      path: "studio/projects/:id",
      component: () => import("@/pages/studio/project/detail.vue"),
      meta: {
        titleKey: "navigation.studioProject",
        icon: "rocket",
        groupKey: "navigation.modding",
        order: 1,
        hideInMenu: true,
      },
    },
    {
      name: "studio-diagnostics",
      path: "studio/diagnostics",
      component: () => import("@/pages/studio/panels/diagnostics.vue"),
      meta: {
        titleKey: "navigation.studioDiagnostics",
        icon: "shield",
        groupKey: "navigation.modding",
        order: 10,
      },
    },
    {
      name: "studio-map",
      path: "studio/map",
      component: () => import("@/pages/studio/panels/map.vue"),
      meta: {
        titleKey: "navigation.studioMap",
        icon: "map",
        groupKey: "navigation.modding",
        order: 15,
      },
    },
    {
      name: "studio-raster",
      path: "studio/raster",
      component: () => import("@/pages/studio/panels/raster.vue"),
      meta: {
        titleKey: "navigation.studioRaster",
        icon: "image",
        groupKey: "navigation.modding",
        order: 40,
      },
    },
    {
      name: "studio-ui",
      path: "studio/ui",
      component: () => import("@/pages/studio/panels/ui.vue"),
      meta: {
        titleKey: "navigation.studioUI",
        icon: "panelTop",
        groupKey: "navigation.modding",
        order: 45,
      },
    },
    {
      name: "studio-camera",
      path: "studio/camera",
      component: () => import("@/pages/studio/panels/camera.vue"),
      meta: {
        titleKey: "navigation.studioCamera",
        icon: "image",
        groupKey: "navigation.modding",
        order: 52,
      },
    },
  ],
} satisfies RouteModule;
