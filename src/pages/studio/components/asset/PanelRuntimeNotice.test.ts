import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { createI18n } from 'vue-i18n';
import { describe, expect, it, vi } from 'vitest';
import { open } from '@tauri-apps/plugin-dialog';
import { command } from '@/api/tauri';
import { panelRuntimeZh, panelRuntimeEn } from '@/locales/panel-runtime';
import PanelRuntimeNotice from './PanelRuntimeNotice.vue';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@/api/tauri', () => ({ command: vi.fn() }));

describe('panel runtime build handoff', () => {
  const translations = () => createI18n({ legacy: false, locale: 'zh-CN', messages: {
    'zh-CN': { panelRuntime: panelRuntimeZh }, 'en-US': { panelRuntime: panelRuntimeEn },
  } });

  it('exports for the selected game and updates the notice on language switch', async () => {
    const i18n = translations();
    const gameData = 'D:/Offline/SimCityData';
    const output = 'D:/exports';
    const path = `${output}/openscp-panel-runtime-v1.package`;
    vi.mocked(open).mockResolvedValueOnce(gameData).mockResolvedValueOnce(output);
    vi.mocked(command).mockResolvedValueOnce(path);
    const wrapper = mount(PanelRuntimeNotice, { props: { required: true }, global: { plugins: [i18n] } });
    expect(wrapper.find('code').exists()).toBe(false);
    await wrapper.get('button').trigger('click');
    await vi.waitFor(() => expect(wrapper.get('code').text()).toBe(path));
    expect(command).toHaveBeenCalledWith('panel_export_shared_runtime', { gameData, output });
    i18n.global.locale.value = 'en-US';
    await nextTick();
    expect(wrapper.get('aside').attributes('aria-label')).toBe(panelRuntimeEn.required);
    expect(wrapper.text()).toContain('Each game installation loads one');
  });

  it.each([undefined, null, '', ' ', 123])('does not add a dependency for an inherited build (%s)', path => {
    const wrapper = mount(PanelRuntimeNotice, { props: { legacyPath: path }, global: { plugins: [translations()] } });
    expect(wrapper.find('aside').exists()).toBe(false);
  });

  it('shows the shared export action for an older build without presenting its duplicate as the dependency to install', () => {
    const wrapper = mount(PanelRuntimeNotice, { props: {
      legacyPath: 'D:/old/openscp-panel-runtime-v1.package',
    }, global: { plugins: [translations()] } });
    expect(wrapper.get('button').text()).toBe(panelRuntimeZh.export);
    expect(wrapper.find('code').exists()).toBe(false);
  });
});
