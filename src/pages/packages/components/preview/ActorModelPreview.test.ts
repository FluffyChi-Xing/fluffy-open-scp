import { mount, flushPromises } from "@vue/test-utils";
import { describe, it, expect, vi } from "vitest";
import { i18n } from "@/locales";
import ActorModelPreview from "./ActorModelPreview.vue";
const api = vi.hoisted(() => ({
  command: vi.fn(),
  preview: vi.fn(),
  section: vi.fn(),
}));
vi.mock("@/api/tauri", () => ({ command: api.command }));
vi.mock("@/api", () => ({
  isTauri: () => true,
  tauriApi: {
    packages: { readRw4Preview: api.preview, readRw4Section: api.section },
  },
}));
function card() {
  return mount(ActorModelPreview, {
    props: {
      packageId: 1,
      references: [{ typeId: 0, groupId: 9, instanceId: 1 }],
    },
    global: {
      plugins: [i18n],
      stubs: {
        MeshPreview: {
          props: ["objBase64s"],
          template: '<div data-testid="mesh">{{objBase64s.length}}</div>',
        },
      },
    },
  });
}
describe("actor model preview", () => {
  it("uses the resolved model package and full TGI for mesh reads", async () => {
    api.command.mockResolvedValueOnce({
      models: [
        { packageId: 8, tgi: { typeId: 0x2f4e681b, group: 0, instance: 5 } },
      ],
      steps: [],
    });
    api.preview.mockResolvedValueOnce({
      sections: [{ typeCode: 0x20009, number: 2 }],
    });
    api.section.mockResolvedValueOnce({ mesh: { objBase64: "b2Jq" } });
    const wrapper = card();
    await wrapper.get("button").trigger("click");
    await flushPromises();
    expect(api.preview).toHaveBeenLastCalledWith(8, {
      typeId: 0x2f4e681b,
      group: 0,
      instance: 5,
    });
    expect(wrapper.get('[data-testid="mesh"]').text()).toBe("1");
  });
  it("explains missing references instead of presenting a blank preview", async () => {
    api.command.mockResolvedValueOnce({
      models: [],
      steps: [
        {
          tgi: { typeId: 0xb1b104, group: 9, instance: 1 },
          via: "reference",
          status: "missing",
          parts: null,
        },
      ],
    });
    const wrapper = card();
    await wrapper.get("button").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("源资源未找到");
    expect(wrapper.text()).toContain("0x00000009");
    expect(wrapper.find('[data-testid="mesh"]').exists()).toBe(false);
  });
});
