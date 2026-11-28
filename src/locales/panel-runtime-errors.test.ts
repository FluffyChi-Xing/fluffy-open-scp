import { afterEach, expect, it } from 'vitest';
import { i18n } from './index';
import { panelRuntimeErrorsZh } from './panel-runtime-errors';

const original = i18n.global.locale.value;
afterEach(() => { i18n.global.locale.value = original; });
it('switches all runtime composition diagnostics with the active language', () => {
  for (const code of Object.keys(panelRuntimeErrorsZh)) {
    const key = `panelEditor.runtimeErrors.${code}`;
    i18n.global.locale.value = 'zh-CN';
    const chinese = i18n.global.t(key);
    expect(chinese).not.toBe(key);
    i18n.global.locale.value = 'en-US';
    expect(i18n.global.t(key)).not.toBe(key);
    expect(i18n.global.t(key)).not.toBe(chinese);
  }
});
