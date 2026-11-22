import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { createI18n } from 'vue-i18n';
import { describe, expect, it } from 'vitest';
import { panelRuntimeZh, panelRuntimeEn } from '@/locales/panel-runtime';
import PanelRuntimeNotice from './PanelRuntimeNotice.vue';

describe('panel runtime build handoff', () => {
  const translations = () => createI18n({ legacy: false, locale: 'zh-CN', messages: {
    'zh-CN': { panelRuntime: panelRuntimeZh }, 'en-US': { panelRuntime: panelRuntimeEn },
  } });

  it('shows the required sidecar path and updates the active notice on language switch', async () => {
    const i18n = translations();
    const path = 'D:/mods/test/.build/hash/dependencies/openscp-panel-runtime-v1.package';
    const wrapper = mount(PanelRuntimeNotice, { props: { path }, global: { plugins: [i18n] } });
    expect(wrapper.get('code').text()).toBe(path);
    expect(wrapper.text()).toContain('仅部署建筑包');
    i18n.global.locale.value = 'en-US';
    await nextTick();
    expect(wrapper.get('aside').attributes('aria-label')).toBe(panelRuntimeEn.required);
    expect(wrapper.text()).toContain('Deploying only the building package');
  });

  it.each([undefined, null, '', ' ', 123])('does not add a dependency for an inherited build (%s)', path => {
    const wrapper = mount(PanelRuntimeNotice, { props: { path }, global: { plugins: [translations()] } });
    expect(wrapper.find('aside').exists()).toBe(false);
  });
});
