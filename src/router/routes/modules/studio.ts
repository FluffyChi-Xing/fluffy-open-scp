import StudioPage from '@/pages/studio/index.vue'
import AssetPanelPage from '@/pages/studio/panels/asset.vue'
import DiagnosticsPanelPage from '@/pages/studio/panels/diagnostics.vue'
import I18nPanelPage from '@/pages/studio/panels/i18n.vue'
import PropertyPanelPage from '@/pages/studio/panels/property.vue'
import RasterPanelPage from '@/pages/studio/panels/raster.vue'
import UiPanelPage from '@/pages/studio/panels/ui.vue'
import type { RouteModule } from '@/router/types'

export default {
  routes: [
    { name: 'studio', path: 'studio', component: StudioPage, meta: { titleKey: 'navigation.studio', icon: 'rocket', groupKey: 'navigation.modding', order: 0 } },
    { name: 'studio-diagnostics', path: 'studio/diagnostics', component: DiagnosticsPanelPage, meta: { titleKey: 'navigation.studioDiagnostics', icon: 'shield', groupKey: 'navigation.modding', order: 10 } },
    { name: 'studio-i18n', path: 'studio/i18n', component: I18nPanelPage, meta: { titleKey: 'navigation.studioI18n', icon: 'globe', groupKey: 'navigation.modding', order: 20 } },
    { name: 'studio-property', path: 'studio/property', component: PropertyPanelPage, meta: { titleKey: 'navigation.studioProperty', icon: 'filePen', groupKey: 'navigation.modding', order: 30 } },
    { name: 'studio-raster', path: 'studio/raster', component: RasterPanelPage, meta: { titleKey: 'navigation.studioRaster', icon: 'image', groupKey: 'navigation.modding', order: 40 } },
    { name: 'studio-ui', path: 'studio/ui', component: UiPanelPage, meta: { titleKey: 'navigation.studioUI', icon: 'panelTop', groupKey: 'navigation.modding', order: 45 } },
    { name: 'studio-asset', path: 'studio/asset', component: AssetPanelPage, meta: { titleKey: 'navigation.studioAsset', icon: 'boxes', groupKey: 'navigation.modding', order: 50 } }
  ]
} satisfies RouteModule
