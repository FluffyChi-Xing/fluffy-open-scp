import { afterEach, expect, it } from "vitest";
import { i18n } from "./index";

const initialLocale = i18n.global.locale.value;

afterEach(() => {
  i18n.global.locale.value = initialLocale;
});

it("updates the effect rendering control text with the active language", () => {
  i18n.global.locale.value = "zh-CN";
  expect(i18n.global.t("package.effectRendering")).toBe("渲染 Effect");
  expect(i18n.global.t("package.effectRenderingHint")).toContain("精细模式");
  expect(i18n.global.t("package.effectRenderingLoading")).toContain("加载");
  expect(i18n.global.t("package.effectRenderingError")).toContain("失败");
  expect(i18n.global.t("package.effectRenderingLimited")).toContain("标记锥");

  i18n.global.locale.value = "en-US";
  expect(i18n.global.t("package.effectRendering")).toBe("Render effects");
  expect(i18n.global.t("package.effectRenderingHint")).toContain("refined mode");
  expect(i18n.global.t("package.effectRenderingLoading")).toContain("Loading");
  expect(i18n.global.t("package.effectRenderingError")).toContain("Could not");
  expect(i18n.global.t("package.effectRenderingLimited")).toContain("markers");
});
