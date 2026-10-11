import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/locales";
import type { PackageStatistics } from "@/api/tauri";
import ExtensionStatsCards from "./ExtensionStatsCards.vue";

const stats = (counts: number[]): PackageStatistics => ({
  packages: [],
  failed: [],
  extensions: counts.map((count, i) => ({
    ext: `type-${i}`,
    typeId: i,
    known: true,
    count,
    storedSize: count,
    decompressedSize: count,
  })),
  totals: {
    fileSize: 100,
    storedSize: 100,
    decompressedSize: 100,
    knownDecompressed: 100,
    unknownDecompressed: 0,
    knownCount: 100,
    unknownCount: 0,
    entryCount: 100,
  },
});
const card = (counts: number[]) =>
  mount(ExtensionStatsCards, {
    props: { stats: stats(counts), loading: false, error: "" },
    global: { plugins: [i18n] },
  });
afterEach(() => {
  i18n.global.locale.value = "zh-CN";
});

describe("extension chart presentation", () => {
  it("retains every exact category while grouping the dial to six segments", () => {
    const wrapper = card([70, 10, 5, 5, 5, 3, 2]);
    expect(wrapper.findAll(".legend li")).toHaveLength(7);
    const segments = wrapper.findAll(".donut-seg");
    expect(segments).toHaveLength(6);
    expect(
      segments
        .map((s) => s.attributes("d"))
        .join(" ")
        .match(/M /g),
    ).toHaveLength(100);
    expect(wrapper.text()).toContain("type-6");
    wrapper.unmount();
  });
  it("renders zero as zero and a sole category as a complete dial", () => {
    const wrapper = card([100, 0]);
    expect(wrapper.findAll(".size-fill")[1]!.attributes("style")).toContain(
      "width: 0%",
    );
    expect(
      (wrapper.findAll(".donut-seg")[0]!.attributes("d") ?? "").match(/M /g),
    ).toHaveLength(100);
    expect(wrapper.findAll(".donut-seg")[1]!.attributes("d")).toBe("");
    wrapper.unmount();
  });
  it("updates newly added chart notes when the active locale changes", async () => {
    const wrapper = card([100]);
    expect(wrapper.text()).toContain("每格约 1%");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("Each tick ≈ 1%");
    expect(wrapper.text()).toContain("Source: previously opened packages");
    wrapper.unmount();
  });
});
