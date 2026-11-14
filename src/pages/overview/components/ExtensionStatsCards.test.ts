import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/locales";
import type { PackageStatistics } from "@/api/tauri";
import ExtensionStatsCards from "./ExtensionStatsCards.vue";

const stats = (): PackageStatistics => ({
  packages: [
    {
      path: "/a.package",
      name: "SimCity_DLC0.package",
      fileSize: 50,
      entryCount: 40,
      knownDecompressed: 80,
      unknownDecompressed: 20,
      extensions: [
        {
          ext: "property",
          typeId: 1,
          known: true,
          count: 30,
          storedSize: 10,
          decompressedSize: 20,
        },
        {
          ext: "rw4",
          typeId: 2,
          known: true,
          count: 10,
          storedSize: 20,
          decompressedSize: 80,
        },
      ],
    },
  ],
  failed: [],
  extensions: [
    {
      ext: "property",
      typeId: 1,
      known: true,
      count: 30,
      storedSize: 10,
      decompressedSize: 20,
    },
    {
      ext: "rw4",
      typeId: 2,
      known: true,
      count: 10,
      storedSize: 20,
      decompressedSize: 80,
    },
  ],
  totals: {
    fileSize: 50,
    storedSize: 30,
    decompressedSize: 100,
    knownDecompressed: 100,
    unknownDecompressed: 0,
    knownCount: 40,
    unknownCount: 0,
    entryCount: 40,
  },
});

afterEach(() => {
  i18n.global.locale.value = "zh-CN";
});

describe("extension analytics grid", () => {
  it("renders complementary Cloudflare-style metric cards", () => {
    const wrapper = mount(ExtensionStatsCards, {
      props: { stats: stats(), loading: false, error: "" },
      global: { plugins: [i18n] },
    });
    expect(wrapper.text()).toContain("资源类型构成");
    expect(wrapper.text()).toContain("可识别覆盖");
    expect(wrapper.text()).toContain("资源条目");
    expect(wrapper.text()).toContain("解压容量");
    expect(wrapper.text()).toContain("压缩倍率");
    expect(wrapper.text()).toContain("体积主导类型");
    // count-leading type vs size-leading type differ
    expect(wrapper.text()).toContain("property");
    expect(wrapper.text()).toContain("rw4");
    expect(wrapper.findAll(".metric-card")).toHaveLength(6);
    wrapper.unmount();
  });

  it("shows empty state when there are no stats yet", () => {
    const wrapper = mount(ExtensionStatsCards, {
      props: { stats: null, loading: false, error: "" },
      global: { plugins: [i18n] },
    });
    expect(wrapper.text()).toContain("打开过 package 后这里会展示统计");
    wrapper.unmount();
  });
});
