import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import RenderStatsCard from "./RenderStatsCard.vue";
import type { RenderTelemetrySummary } from "@/api/tauri";

function summary(overrides: Partial<RenderTelemetrySummary> = {}): RenderTelemetrySummary {
  return {
    windowDays: 7,
    since: 0,
    totalCount: 3,
    stages: [
      {
        stage: "model_load",
        count: 2,
        avgMs: 40,
        minMs: 20,
        maxMs: 60,
        p50Ms: 40,
        p95Ms: 58,
        lastMs: 60,
      },
      {
        stage: "decal_render",
        count: 1,
        avgMs: 10,
        minMs: 10,
        maxMs: 10,
        p50Ms: 10,
        p95Ms: 10,
        lastMs: 10,
      },
    ],
    ...overrides,
  };
}

function mountCard(props: {
  summary: RenderTelemetrySummary | null;
  loading?: boolean;
  error?: string;
}) {
  return mount(RenderStatsCard, {
    props: { loading: false, error: "", ...props },
    global: { plugins: [i18n] },
  });
}

describe("RenderStatsCard", () => {
  it("每个 stage 一行哑铃：空心点=平均、实心点=p95", () => {
    const wrapper = mountCard({ summary: summary() });
    expect(wrapper.findAll(".stage-row")).toHaveLength(2);
    expect(wrapper.findAll(".dot-hollow")).toHaveLength(2);
    expect(wrapper.findAll(".dot-ink")).toHaveLength(2);
    expect(wrapper.findAll(".connector")).toHaveLength(2);
    // 数值与位置严格成正比：p95 点的百分比位置在平均点右侧
    const percent = (el: { attributes: (n: string) => string | undefined }) =>
      Number.parseFloat((el.attributes("style") ?? "").match(/left:\s*([\d.]+)%/)?.[1] ?? "NaN");
    const [avgDot] = wrapper.findAll(".dot-hollow");
    const [p95Dot] = wrapper.findAll(".dot-ink");
    expect(percent(p95Dot)).toBeGreaterThan(percent(avgDot));
  });

  it("按平均耗时降序排列（最慢的在最上），读数与样本数在右侧", () => {
    const wrapper = mountCard({ summary: summary() });
    // model_load 平均 40ms > decal_render 10ms
    expect(wrapper.findAll(".reading-p95")[0].text()).toContain("58");
    expect(wrapper.findAll(".reading-p95")[1].text()).toContain("10");
    expect(wrapper.findAll(".reading-avg")[0].text()).toContain("40");
    expect(wrapper.findAll(".stage-count")[0].text()).toContain("2");
  });

  it("行高固定，不随卡片宽度缩放（防止图形被等比撑大）", () => {
    const wrapper = mountCard({ summary: summary() });
    // 图表是 HTML 行而非 SVG，因此不存在 viewBox 等比放大
    expect(wrapper.find("svg").exists()).toBe(false);
    expect(wrapper.findAll(".axis-tick")).toHaveLength(3);
  });

  it("副标题给出图例与时间窗口", () => {
    const wrapper = mountCard({ summary: summary({ totalCount: 42, windowDays: 7 }) });
    const sub = wrapper.find(".stats-sub").text();
    expect(sub).toContain("7");
    expect(sub).toContain("42");
  });

  it("无数据时显示空状态", () => {
    const wrapper = mountCard({ summary: null });
    expect(wrapper.find(".chart").exists()).toBe(false);
    expect(wrapper.find(".state").exists()).toBe(true);
  });

  it("错误优先于空状态显示", () => {
    const wrapper = mountCard({ summary: null, error: "store_error" });
    expect(wrapper.find(".state.error").text()).toBe("store_error");
  });

  it("点击清空需二次确认后才 emit", async () => {
    const wrapper = mountCard({ summary: summary() });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await wrapper.find(".clear-button").trigger("click");
    expect(wrapper.emitted("clear")).toBeUndefined();

    confirm.mockReturnValue(true);
    await wrapper.find(".clear-button").trigger("click");
    expect(wrapper.emitted("clear")).toHaveLength(1);
    confirm.mockRestore();
  });
});
