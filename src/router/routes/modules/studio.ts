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
      name: "studio-overview",
      path: "studio/overview",
      component: () => import("@/pages/studio/overview.vue"),
      meta: {
        titleKey: "navigation.studioOverview",
        icon: "dashboard",
        groupKey: "navigation.modding",
        order: 5,
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
      name: "studio-i18n",
      path: "studio/i18n",
      component: () => import("@/pages/studio/panels/i18n.vue"),
      meta: {
        titleKey: "navigation.studioI18n",
        icon: "globe",
        groupKey: "navigation.modding",
        order: 20,
      },
    },
    {
      name: "studio-property",
      path: "studio/property",
      component: () => import("@/pages/studio/panels/property.vue"),
      meta: {
        titleKey: "navigation.studioProperty",
        icon: "filePen",
        groupKey: "navigation.modding",
        order: 30,
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
      name: "studio-asset",
      path: "studio/asset",
      component: () => import("@/pages/studio/panels/asset.vue"),
      meta: {
        titleKey: "navigation.studioAsset",
        icon: "boxes",
        groupKey: "navigation.modding",
        order: 50,
      },
    },
    {
      name: "studio-versions",
      path: "studio/versions",
      component: () => import("@/pages/studio/panels/versions.vue"),
      meta: {
        titleKey: "navigation.studioVersions",
        icon: "history",
        groupKey: "navigation.modding",
        order: 55,
      },
    },
  ],
} satisfies RouteModule;
