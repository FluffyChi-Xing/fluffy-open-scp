import { i18n } from "@/locales";
import { shallowMount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import RasterFlowTool from "./RasterFlowTool.vue";
import RasterEditor from "@/components/raster/RasterEditor.vue";
import { useRasterSession } from "@/components/raster/useRasterSession";
import type { FlowSchema } from "./contracts";

function fixture() {
  const session = useRasterSession();
  session.changeProfile("slot2");
  session.create(2, 2);
  const schema: FlowSchema = {
    schema_version: 1,
    id: "texture-input-3",
    kind: "texture-input",
    config: {
      slot: 2,
      asset: "assets/textures/original.png",
      document: JSON.stringify(session.snapshot()),
    },
  };
  const wrapper = shallowMount(RasterFlowTool, {
    props: { schema, project: "example", busy: false },
    global: {
      plugins: [i18n],
      stubs: { FSheet: { template: "<div><slot /></div>" } },
    },
  });
  return { wrapper, schema, session };
}
describe("Raster flow host", () => {
  it("reopens persisted pixel data and emits a schema without changing the source asset", async () => {
    const { wrapper, session } = fixture();
    await wrapper.find("button").trigger("click");
    const editor = wrapper.findComponent(RasterEditor);
    expect(editor.props("initialSchema")?.profile).toBe("slot2");
    expect(editor.props("mode")).toBe("flow");
    session.name.value = "edited normal";
    editor.vm.$emit("schema", session.snapshot());
    const saved = wrapper.emitted("save")![0][0] as FlowSchema;
    expect(saved.config.asset).toBe("assets/textures/original.png");
    expect(JSON.parse(String(saved.config.document)).name).toBe(
      "edited normal",
    );
    // A request is not a persistence acknowledgement; the editor stays open on failure.
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(true);
    await wrapper.setProps({ schema: saved });
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(false);
    wrapper.unmount();
  });
  it("rejects a persisted document for a different material slot", async () => {
    const { wrapper, schema } = fixture();
    await wrapper.setProps({
      schema: { ...schema, config: { ...schema.config, slot: 4 } },
    });
    await wrapper.find("button").trigger("click");
    expect(wrapper.findComponent(RasterEditor).exists()).toBe(false);
    expect(wrapper.get("[role=alert]").text()).toContain("槽位不一致");
    wrapper.unmount();
  });
});
