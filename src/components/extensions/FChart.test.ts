import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import FChart from "./FChart.vue";

describe("Lieflat line chart", () => {
  it("keeps exact series values and updates SVG geometry with data", async () => {
    const wrapper = mount(FChart, {
      props: {
        label: "Traffic",
        labels: ["Mon", "Tue"],
        series: [{ name: "Requests", values: [0, 100] }],
      },
    });
    expect(wrapper.find("svg").attributes("aria-label")).toBe("Traffic");
    expect(wrapper.find(".hairline").attributes("d")).toBe("M 38 250 L 562 42");
    expect(wrapper.findAll("circle title").map((t) => t.text())).toEqual([
      "Mon · Requests: 0",
      "Tue · Requests: 100",
    ]);
    await wrapper.setProps({
      series: [{ name: "Requests", values: [100, 0] }],
    });
    expect(wrapper.find(".hairline").attributes("d")).toBe("M 38 42 L 562 250");
    wrapper.unmount();
  });
  it("keeps empty and all-zero series finite", () => {
    for (const values of [[], [0, 0]]) {
      const wrapper = mount(FChart, {
        props: {
          label: "Traffic",
          labels: ["Mon", "Tue"],
          series: [{ name: "Requests", values }],
        },
      });
      expect(wrapper.html()).not.toMatch(/NaN|Infinity/);
      wrapper.unmount();
    }
  });
});
