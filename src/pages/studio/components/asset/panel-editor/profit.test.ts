import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { createI18n } from 'vue-i18n';
import { describe, expect, it } from 'vitest';
import PanelLibrary from './PanelLibrary.vue';
import { panelEditorZh, panelEditorEn } from '@/locales/panel-editor';
import labels from '@/lib/panel-messages.json';
import catalog from '@/lib/panel-components.json';

describe('common profit templates', () => {
  it('adds the standalone template and updates its data-source hint on language switch', async () => {
    const i18n = createI18n({ legacy: false, locale: 'zh-CN', messages: {
      'zh-CN': { panelEditor: panelEditorZh, panelMetrics: labels['zh-CN'] },
      'en-US': { panelEditor: panelEditorEn, panelMetrics: labels['en-US'] },
    } });
    const wrapper = mount(PanelLibrary, { props: { leaves: [], items: [], power: false }, global: { plugins: [i18n] } });
    const button = wrapper.findAll('button').find(b => b.text().includes('每日利润（财务账本）'))!;
    expect(button.attributes('title')).toContain('账本依赖');
    await button.trigger('click');
    expect(wrapper.emitted('add')?.[0]).toEqual(['profit']);
    i18n.global.locale.value = 'en-US'; await nextTick();
    expect(button.text()).toContain('Daily profit');
    expect(button.attributes('title')).toBe(labels['en-US'].dailyProfitHint);
  });

  it('queries only the four accounting inputs actually used by native daily profit', () => {
    const f = bindings();
    const query = f.daily.updateQueries('1,0');
    expect(Object.keys(query)).toEqual(['incomeToday','expensesToday','incomeYesterday','expensesYesterday']);
    expect(query).not.toHaveProperty('maintCost');
    expect(query).not.toHaveProperty('income');
  });

  it('shows actual business indices with localized current/previous labels, without currency', () => {
    const f = bindings();
    const info = { ...f.controls, panelNative: {} };
    f.index.update(info, { current: { amount: 60 }, previous: { amount: 58 } });
    expect(f.controls.textControlRight1.text).toBe('60');
    expect(f.controls.textControlRight2.text).toBe('58');
    expect(f.controls.textControlTitle.text).toBe('利润指标');
    f.locale.mLocaleCode = 'en-us';
    f.index.update(info, { current: { amount: 0 }, previous: { amount: 58 } });
    expect(f.controls.textControlLeft1.text).toBe('Current');
    expect(f.controls.textControlRight1.text).toBe('0');
    f.index.update(info, { current: null, previous: { amount: 58 } });
    expect(f.index.minimize).toBe(true);
  });
});

function bindings() {
  const text = () => ({ text: '', SetText(value: string) { this.text = value; } });
  const controls = { textControlTitle: text(), textControlLeft1: text(), textControlLeft2: text(), textControlRight1: text(), textControlRight2: text() };
  const simcity = {
    RolloverItems: { kBlockLandmarkProfit: { updateQueries: () => ({ incomeToday: [], expensesToday: [], incomeYesterday: [], expensesYesterday: [], maintCost: [], income: [], expenses: [] }) }, kEmptySpace10pt: {} } as Record<string, unknown>,
    cRolloverUI: { prototype: { UpdateItemPositions() {} } },
  };
  const locale = { mLocaleCode: 'zh-tw' };
  runInNewContext(readFileSync(resolve('src/pages/studio/components/asset/panel-editor/native/runtime.js'), 'utf8'), {
    window: { OpenSCPPanelComponents: catalog.filter(c => ['profit','profit-index'].includes(c.id)), OpenSCPPanelMessages: labels },
    simcity, scrui: { gUIManager: { mLocaleManager: locale } },
  });
  // Runtime bindings live across the VM boundary and include mutable native state.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const registered = simcity.RolloverItems as Record<string, any>;
  return { daily: registered.OpenSCPPanel_profit, index: registered['OpenSCPPanel_profit-index'], controls, locale };
}
