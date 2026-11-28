import { mount, flushPromises } from "@vue/test-utils";
import { describe, it, expect, vi } from "vitest";
import { i18n } from "@/locales";
import ActorAppearancePreview from "./ActorAppearancePreview.vue";
const api = vi.hoisted(() => ({ command: vi.fn() }));
vi.mock("@/api/tauri", () => ({ command: api.command }));
const actor = {
  id: 1,
  models: [],
  templates: [],
  appearances: [],
  speed: 5,
  scale: 1,
  animation: null,
  spawn: null,
  destroy: null,
  resourceBind: null,
};
describe("actor visual switch", () => {
  it("does no visual work by default and releases the stage when switched off", async () => {
    api.command.mockReset();
    api.command.mockResolvedValue({ body: {}, head: null });
    const wrapper = mount(ActorAppearancePreview, {
      props: {
        actor,
        resolution: {
          models: [
            {
              packageId: 7,
              tgi: { typeId: 0x2f4e681b, group: 0, instance: 1 },
            },
          ],
          steps: [],
        },
        packageId: 1,
        modelIndex: 0,
      },
      global: {
        plugins: [i18n],
        stubs: {
          ActorVisualViewport: {
            template: '<div data-testid="visual-stage"/>',
          },
        },
      },
    });
    expect(api.command).not.toHaveBeenCalled();
    await wrapper.get('[role="switch"]').trigger("click");
    await flushPromises();
    expect(api.command).toHaveBeenCalledTimes(1);
    expect(wrapper.find('[data-testid="visual-stage"]').exists()).toBe(true);
    await wrapper.get('[role="switch"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="visual-stage"]').exists()).toBe(false);
    expect(wrapper.emitted("active")?.at(-1)).toEqual([false]);
  });
});
