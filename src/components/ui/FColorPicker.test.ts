import { defineComponent, nextTick } from "vue";
import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import FColorPicker from "@/components/ui/FColorPicker.vue";

const Host = defineComponent({
  components: { FColorPicker },
  data() {
    return { color: "#ff8800" };
  },
  template: `<FColorPicker :model-value="color" @update:model-value="color = $event" />`,
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("FColorPicker", () => {
  it("opens the popover panel on trigger click", async () => {
    const wrapper = mount(Host, { attachTo: document.body });
    expect(document.body.querySelector(".f-popover-panel")).toBeNull();

    await wrapper.find(".color-trigger").trigger("click");
    await nextTick();
    expect(document.body.querySelector(".f-popover-panel")).not.toBeNull();
    wrapper.unmount();
  });

  it("emits a normalized hex on color input change", async () => {
    const wrapper = mount(Host, { attachTo: document.body });
    await wrapper.find(".color-trigger").trigger("click");
    await nextTick();

    const input = document.body.querySelector(
      ".f-popover-panel input[type=color]",
    ) as HTMLInputElement;
    expect(input).not.toBeNull();
    input.value = "#00FFAA";
    input.dispatchEvent(new Event("input"));

    expect(wrapper.vm.color).toBe("#00ffaa");
    wrapper.unmount();
  });
});
