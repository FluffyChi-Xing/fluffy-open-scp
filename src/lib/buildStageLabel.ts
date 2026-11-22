import { buildStagesEn } from '@/locales/build-stages';

type Translate = (key: string, values?: Record<string, string>) => string;

/** Translate display names while preserving backend stage IDs in event data. */
export function buildStageLabel(stage: string, t: Translate): string {
  if (Object.prototype.hasOwnProperty.call(buildStagesEn, stage) && stage !== 'unknown') {
    return t(`buildStages.${stage}`);
  }
  if (stage === 'road::compile') return t('roadAsset.compileStage');
  if (stage.startsWith('map_resources::')) return t(`mapResources.${stage.split('::')[1]}`);
  return t('buildStages.unknown', { stage });
}
