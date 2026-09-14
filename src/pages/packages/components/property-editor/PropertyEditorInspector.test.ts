import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import PropertyEditorInspector from "./PropertyEditorInspector.vue";

vi.mock("@/api", () => ({
  isTauri: () => false,
  tauriApi: {
    renderTelemetry: {
      record: vi.fn().mockResolvedValue(0),
      summary: vi.fn().mockResolvedValue(null),
      clear: vi.fn().mockResolvedValue({ rows: 0 }),
    },
  },
}));

function mountInspector() {
  return mount(PropertyEditorInspector, {
    props: { unit: null },
    global: { plugins: [i18n] },
  });
}

describe("PropertyEditorInspector 页签", () => {
  it("共四个页签（属性 / 坐标 / 元数据 / 渲染遥测）", () => {
    const tabs = mountInspector().findAll('[role="tab"]');
    expect(tabs).toHaveLength(4);
    // 第四个页签的 i18n key 已被本地化（不再是原始 key）
    expect(tabs[3].attributes("title")).toBeTruthy();
    expect(tabs[3].attributes("title")).not.toContain("package.");
  });

  it("切到渲染遥测页签不会渲染元数据面板（守 v-else → v-else-if 迁移）", async () => {
    const wrapper = mountInspector();
    const tabs = wrapper.findAll('[role="tab"]');

    await tabs[3].trigger("click");
    expect(wrapper.findComponent({ name: "PropertyEditorRenderTelemetry" }).exists()).toBe(true);
    expect(wrapper.findComponent({ name: "PropertyEditorMetadataPanel" }).exists()).toBe(false);

    // 元数据页签仍然只渲染自己
    await tabs[2].trigger("click");
    expect(wrapper.findComponent({ name: "PropertyEditorMetadataPanel" }).exists()).toBe(true);
    expect(wrapper.findComponent({ name: "PropertyEditorRenderTelemetry" }).exists()).toBe(false);
  });

  it("默认停在属性页签", () => {
    const wrapper = mountInspector();
    expect(wrapper.findComponent({ name: "PropertyEditorProperties" }).exists()).toBe(true);
    expect(wrapper.find('[role="tab"][aria-selected="true"]').text()).toBeTruthy();
  });
});
