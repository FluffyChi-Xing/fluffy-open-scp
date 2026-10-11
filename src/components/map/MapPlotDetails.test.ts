import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { describe, expect, it } from "vitest";
import MapPlotDetails from "./MapPlotDetails.vue";
import { i18n } from "@/locales";
import type { Region3DData, Region3DPlot } from "@/lib/region-map";

describe("selected city details", () => {
  it("shows only the selected city, preserves zero resources and switches language live", async () => {
    i18n.global.locale.value = "zh-CN";
    const plot: Region3DPlot = {
      uid: "1026",
      x: 16,
      y: 32,
      z: -850,
      name: "测试城市",
      nameEn: "Test City",
      kind: "city",
      info: {
        nameEntry: "ABC",
        descriptionEntry: "DEF",
        description: "原始城市介绍",
        descriptionEn: "Original description",
        buildable: 90,
        wind: 2,
        water: 3,
        coal: 0,
        oil: 2,
        ore: 1,
        rail: true,
        shipping: false,
      },
    };
    const data: Region3DData = {
      heightPngBase64: "",
      groundPngBase64: "",
      size: 2,
      metersPerPixel: 16,
      originWorld: [0, 0],
      waterZ: -870,
      heightDiv: 32,
      heightBias: -1024,
      desert: false,
      displayName: null,
      displayNameEn: null,
      brushes: [],
      plots: [plot, { ...plot, uid: "1027", name: "其他城市" }],
      staticLayers: {
        sources: [
          {
            site: "1026",
            bounds: [0, 0, 32, 32],
            origin: [16, 16],
            curves: [],
            maps: [
              { id: 0xd779c976, size: 2, values: [0, 0, 0, 0] },
              { id: 0x0f92dc56, size: 2, values: [0, 65535, 0, 65535] },
            ],
          },
        ],
      },
    };
    const wrapper = mount(MapPlotDetails, {
      props: { data, plot },
      global: { plugins: [i18n] },
    });
    expect(wrapper.text()).toContain("测试城市");
    expect(wrapper.text()).toContain("原始城市介绍");
    expect(wrapper.text()).not.toContain("其他城市");
    expect(wrapper.text()).toContain("0.0% 面积");
    expect(wrapper.text()).toContain("50.0% 面积");
    expect(wrapper.text()).toContain("城市地下水");
    expect(wrapper.text()).not.toContain("区域地下水");
    expect(wrapper.text()).toContain("无数据");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Test City");
    expect(wrapper.text()).toContain("Original description");
    expect(wrapper.text()).toContain("Buildable area");
    expect(wrapper.text()).toContain("City Groundwater");
    expect(wrapper.text()).not.toContain("原始城市介绍");
    await wrapper.setProps({ plot: null });
    expect(wrapper.text()).not.toContain("Test City");
    expect(wrapper.find(".resources").exists()).toBe(false);
    wrapper.unmount();
    i18n.global.locale.value = "zh-CN";
  });
});
