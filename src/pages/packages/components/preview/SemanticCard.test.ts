import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import SemanticCardView from "./SemanticCard.vue";
import { barGradientCss } from "./semanticCard";
import type { SemanticCard } from "@/api/tauri";

vi.mock("@/api", () => ({
  isTauri: () => false,
  tauriApi: {
    packages: {
      readData: vi.fn().mockRejectedValue(new Error("no tauri")),
      resolveNames: vi.fn().mockRejectedValue(new Error("no tauri")),
    },
  },
}));

function mountCard(card: SemanticCard) {
  return mount(SemanticCardView, {
    props: { card, packageId: 1 },
    global: { plugins: [i18n] },
  });
}

const parent = {
  typeId: 0x00b1b104,
  groupId: 0x0fc6cc94,
  instanceId: 0xc50b96eb,
};

describe("SemanticCard 语义预览卡", () => {
  it("警报卡：文本缺失时回退显示 #表:实例 引用，并渲染时长徽章", () => {
    const wrapper = mountCard({
      kind: "alert",
      parent,
      icon: { typeId: 0x2f7d0004, groupId: 0, instanceId: 0xb5d377c5 },
      texts: [{ tableId: 0x0d9175f2, instanceId: 0x1, text: null }],
      durationSeconds: 5,
    });
    expect(wrapper.find(".alert-text").text()).toBe("#D9175F2:1");
    expect(wrapper.find(".duration-badge").text()).toContain("5");
    expect(wrapper.find(".parent-row .ref-code").text()).toContain("0FC6CC94");
  });

  it("行动卡：已解析文案直出，失败标题独立分组", () => {
    const wrapper = mountCard({
      kind: "sim-action",
      parent: null,
      titles: [
        { tableId: 1, instanceId: 1, text: "尋找昂貴的商店。" },
        { tableId: 1, instanceId: 2, text: "想要買魚子醬。" },
      ],
      failedTitles: [{ tableId: 2, instanceId: 3, text: "未能找到富豪商店。" }],
    });
    const lists = wrapper.findAll(".text-list");
    expect(lists).toHaveLength(2);
    expect(lists[0].findAll("li")).toHaveLength(2);
    expect(lists[0].findAll("li")[0].text()).toBe("尋找昂貴的商店。");
    expect(lists[1].classes()).toContain("failed");
    expect(lists[1].text()).toContain("未能找到富豪商店。");
  });

  it("图层卡：色带按 RGBA 序列渲染为 CSS 渐变", () => {
    const wrapper = mountCard({
      kind: "map-layer",
      name: [{ tableId: 3, instanceId: 4, text: "kRegionDataLayer_Alloy" }],
      icon: null,
      barColors: [
        [0, 0.32, 0.61, 0],
        [1, 0.7, 0.23, 1],
      ],
      legend: { typeId: 0x67771f5c, groupId: 0x0b074e5a, instanceId: 0x5c5e4132 },
    });
    expect(wrapper.find(".bar-colors").exists()).toBe(true);
    // 渐变串本身是纯函数（jsdom 不序列化渐变背景，直接测函数）
    expect(
      barGradientCss([
        [0, 0.32, 0.61, 0],
        [1, 0.7, 0.23, 1],
      ]),
    ).toBe(
      "linear-gradient(to right, rgba(0,82,156,0) 0.0%, rgba(255,179,59,1) 100.0%)",
    );
    expect(wrapper.find(".alert-text").text()).toBe("kRegionDataLayer_Alloy");
  });
});
