import { flushPromises, mount } from "@vue/test-utils";
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

  it("载具卡：列表模式展示车灯与模型引用，切 3D 无 tauri 时显示错误态", async () => {
    const wrapper = mountCard({
      kind: "vehicle",
      parent: null,
      name: [{ tableId: 4, instanceId: 5, text: "Empty Coal Truck" }],
      models: [{ typeId: 0x2f4e681b, groupId: 0, instanceId: 0x79a56b0d }],
      lightCount: 2,
      lightNames: ["headlightCar", "brakelightCar"],
      details: { effects: [], sinks: [], graphics: [] },
    });
    expect(wrapper.text()).toContain("Empty Coal Truck");
    expect(wrapper.text()).toContain("headlightCar");
    expect(wrapper.text()).toContain("2");
    const buttons = wrapper.findAll(".view-toggle button");
    await buttons[1].trigger("click");
    await flushPromises();
    // 无 tauri 运行时：网格不可用 → 错误态文案
    expect(wrapper.text()).toContain(
      i18n.global.t("package.card.meshUnavailable"),
    );
  });

  it("道路卡：路宽/压平徽章与外观引用", () => {
    const wrapper = mountCard({
      kind: "road",
      pathTitle: [{ tableId: 6, instanceId: 7, text: "Dirt Road (CB)" }],
      appearance: {
        typeId: 0x00b1b104,
        groupId: 0x09558b7e,
        instanceId: 0x565bcf6c,
      },
      ghostAppearance: null,
      flattenTerrain: true,
      pathWidth: 48,
    });
    expect(wrapper.text()).toContain("Dirt Road (CB)");
    expect(wrapper.text()).toContain("48");
    expect(wrapper.text()).toContain("0x00B1B104:0x09558B7E:0x565BCF6C");
  });

  it("菜单卡：标题/描述直出 + 排序徽章", () => {
    const wrapper = mountCard({
      kind: "menu",
      title: [{ tableId: 8, instanceId: 9, text: "Public Library" }],
      description: [{ tableId: 10, instanceId: 11, text: "A city institution." }],
      icon: { typeId: 0x2f7d0004, groupId: 0x40e02400, instanceId: 0x9f61a524 },
      parentMenu: null,
      order: 100,
    });
    expect(wrapper.text()).toContain("Public Library");
    expect(wrapper.text()).toContain("A city institution.");
    expect(wrapper.text()).toContain("100");
  });

  it("资源定义卡：Resource Name 直出", () => {
    const wrapper = mountCard({
      kind: "resource-def",
      parent: null,
      resourceName: [{ tableId: 12, instanceId: 13, text: "kResourceIDOreRaw" }],
    });
    expect(wrapper.text()).toContain("kResourceIDOreRaw");
  });

  it("无模型运输条目显示效果与触发槽位说明", () => {
    const wrapper = mountCard({ kind: 'vehicle', parent: null, name: [], models: [], lightCount: null, lightNames: [], details: { effects: [{ typeId: 0, groupId: 0, instanceId: 0xcd400ac9 }], sinks: [{ typeId: 0, groupId: 0, instanceId: 0x0186ee3a }], graphics: [] } });
    expect(wrapper.text()).toContain('运输效果配置');
    expect(wrapper.text()).toContain('0xCD400AC9');
    expect(wrapper.text()).toContain('0x0186EE3A');
    expect(wrapper.text()).not.toContain('暂无可用载具模型');
  });

  it("演员表保留负数并在切换语言时更新文案", async () => {
    const wrapper = mountCard({ kind: 'resource-entry', parent: null, slots: [], floatA: null, floatB: null, enabled: null,
      scene: { recognized: true, selfDestruct: false, issues: [], actors: [{ id: 0x8017fce5, models: [], templates: [], speed: 5, scale: 1.35, animation: null, spawn: null, destroy: null, resourceBind: null, appearances: [{ body: null, skin: -1, skinMax: -1, outfit: -1, outfitMax: -1 }] }] }
    });
    expect(wrapper.text()).toContain('肤色下限');
    expect(wrapper.text()).toContain('-1');
    expect(wrapper.text()).toContain('0x8017FCE5');
    const previous = i18n.global.locale.value;
    try {
      i18n.global.locale.value = 'en-US';
      await flushPromises();
      expect(wrapper.text()).toContain('Skin minimum');
      expect(wrapper.text()).not.toContain('肤色下限');
    } finally { i18n.global.locale.value = previous; }
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
