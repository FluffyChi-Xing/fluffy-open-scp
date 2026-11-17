import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import RoadProjectPanel from "./RoadProjectPanel.vue";

vi.mock("@/components/ui/MonacoEditor.vue", () => ({
  default: { name: "MonacoEditor", props: ["value", "language", "editable"], template: "<div />" },
}));

vi.mock("../asset/useMenuImages", () => ({
  useMenuImages: () => ({
    previews: { icon: "", marquee: "" },
    importingImage: false,
    uploadMenuImage: vi.fn(),
  }),
}));

it("uses categorized cards for the live road, editable menu and saved dependencies", async () => {
  const locale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const panel = mount(RoadProjectPanel, {
    props: {
      project: "test",
      schema: {
        title: "Road A",
        description: "",
        identity: 42,
        format: "openscp.road-asset/1",
        recipe: "mediumStreet",
        gamePackage: "game/SimCity_Game.package",
        scriptsPackage: "game/Scripts.package",
        section: [{ id: "divider", kind: "median", width: 4.5 }],
      },
      metadata: { author: "test", version: "1.0.0", description: "" },
      dirty: false,
      saving: false,
      manifest: {
        build: { schema: "schemas/road/schema.json" },
        dependencies: {
          road: {
            resolved: {
              packageSha256: "test-sha",
              packages: ["Graphics.package"],
              resources: [
                {
                  tgi: "00B1B104:00000000:00000001",
                  mode: "external",
                  package: "Graphics.package",
                },
              ],
              unresolved: [],
            },
          },
        },
      },
    },
    global: { plugins: [i18n] },
  });
  try {
    const navigate = async (index: number) =>
      panel.findAll("nav button")[index].trigger("click");
    await navigate(1);
    await panel.get("input").setValue("New road");
    expect(panel.emitted("menu")?.[0]).toEqual([{ title: "New road" }]);
    await panel.get('input[type="number"]').setValue("123");
    expect(panel.emitted("menu")?.[1]).toEqual([{ menu: { position: 123 } }]);
    const cost = () => panel.get('input[aria-describedby="road-unit-cost-hint"]');
    expect(cost().element.parentElement?.textContent).toContain("每米建造费用");
    await cost().setValue("0");
    await cost().setValue("");
    expect(panel.emitted("menu")?.slice(2)).toEqual([
      [{ unitCost: 0 }],
      [{ unitCost: null }],
    ]);
    await navigate(2);
    expect(panel.text()).toContain("总宽 4.5 米");
    expect(panel.text()).toContain("1 个分隔带");
    await navigate(3);
    expect(panel.text()).toContain("game/SimCity_Game.package");
    expect(panel.text()).toContain("Graphics.package");
    await panel.setProps({ dirty: true });
    expect(panel.text()).toContain("构建已保存的道路");
    expect(panel.text()).not.toContain("Graphics.package");
    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(panel.text()).toContain("Dependencies");
    await navigate(1);
    expect(cost().element.parentElement?.textContent).toContain("Construction cost per metre");
    await navigate(4);
    const editor = panel.getComponent({ name: "MonacoEditor" });
    expect(editor.props("language")).toBe("json");
    expect(editor.props("editable")).not.toBe(true);
    expect(editor.props("value")).toContain("test-sha");
  } finally {
    panel.unmount();
    i18n.global.locale.value = locale;
  }
});
