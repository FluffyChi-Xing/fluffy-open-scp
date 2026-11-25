import { afterEach, expect, it } from "vitest";
import { i18n } from "./index";

const initialLocale = i18n.global.locale.value;
afterEach(() => { i18n.global.locale.value = initialLocale; });

it("updates the native service contract error when the active language changes", () => {
  const key = "studio.assetWorkbench.behaviorSheet.buildError.BEHAVIOR_COPAN_SERVICES";
  i18n.global.locale.value = "zh-CN";
  const chinese = i18n.global.t(key);
  expect(chinese).toContain("水电预算");
  i18n.global.locale.value = "en-US";
  expect(i18n.global.t(key)).toContain("utility budget");
  expect(i18n.global.t(key)).not.toBe(chinese);
  i18n.global.locale.value = "zh-CN";
  expect(i18n.global.t(key)).toBe(chinese);
  const residence = "studio.assetWorkbench.behaviorSheet.buildError.BEHAVIOR_COPAN_RESIDENCE";
  expect(i18n.global.t(residence)).toContain("返家");
  i18n.global.locale.value = "en-US";
  expect(i18n.global.t(residence)).toContain("household recipe");
});
