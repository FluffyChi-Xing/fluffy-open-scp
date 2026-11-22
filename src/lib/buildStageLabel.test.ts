import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { computed, nextTick } from 'vue';
import { createI18n } from 'vue-i18n';
import { describe, expect, it } from 'vitest';
import { buildStagesZh, buildStagesEn } from '@/locales/build-stages';
import { buildStageLabel } from './buildStageLabel';

describe('build stage display names', () => {
  const i18n = () => createI18n({ legacy: false, locale: 'zh-CN', messages: {
    'zh-CN': { buildStages: buildStagesZh }, 'en-US': { buildStages: buildStagesEn },
  } });

  it('covers every stage in the actual asset build chain in both languages', () => {
    const source = readFileSync(resolve('src-tauri/src/asset_build_m1/stages.rs'), 'utf8');
    const chain = source.match(/const CHAIN:[\s\S]*?=\s*\[([\s\S]*?)\];/)?.[1];
    expect(chain).toBeDefined();
    const stages = [...chain!.matchAll(/"([a-z-]+)"/g)].map(match => match[1]);
    expect(stages.length).toBeGreaterThan(10);
    const translator = i18n();
    for (const language of ['zh-CN', 'en-US'] as const) {
      translator.global.locale.value = language;
      for (const stage of [...stages, 'panel-bind', 'cache-reuse', 'vanilla-variant']) {
        expect(translator.global.te(`buildStages.${stage}`)).toBe(true);
        expect(buildStageLabel(stage, translator.global.t)).not.toBe(stage);
      }
    }
  });

  it('updates an active label when the language changes', async () => {
    const translator = i18n();
    const label = computed(() => buildStageLabel('load-inputs', translator.global.t));
    expect(label.value).toBe('读取构建输入');
    translator.global.locale.value = 'en-US';
    await nextTick();
    expect(label.value).toBe('Load build inputs');
    expect(buildStageLabel('new-stage', translator.global.t)).toBe('Build stage: new-stage');
  });
});
