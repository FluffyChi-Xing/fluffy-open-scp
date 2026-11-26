import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import * as THREE from "three";
import { expect, it } from "vitest";
import type { DecalUnit } from "@/api/tauri";
import FStartupLoading from "@/components/ui/FStartupLoading.vue";
import { i18n } from "@/locales";
import { schemaUnitJson } from "@/pages/packages/components/property-editor/peSchemaDoc";
import { createSunEnv } from "@/pages/packages/components/property-editor/refinedRender";
import { buildDecalBody } from "@/render/exec/three/decalBody";
import { createPlacedDecal, writeDecalSources } from "./workbenchDecals";
import WorkbenchSchemaViewport from "./WorkbenchSchemaViewport.vue";

const Viewport = defineComponent({
  name: "PropertyEditorViewport",
  props: {
    grouping: { type: Object, required: true }, decalTextures: Array,
    lotSize: Array, lotColors: Array, hiddenUnits: Set, groupVisibility: Object, modelState: String,
  },
  template: "<div />",
});
const global = { plugins: [i18n], stubs: { PropertyEditorViewport: Viewport } };

it("uses the startup logo during model loading and keeps the viewport mounted", async () => {
  const before = i18n.global.locale.value;
  const wrapper = mount(WorkbenchSchemaViewport, {
    props: { schema: document(), modelState: "loading" },
    global,
  });
  try {
    const viewport = wrapper.getComponent(Viewport);
    expect(viewport.props("modelState")).toBe("pending");
    expect(viewport.attributes("inert")).toBeDefined();
    const loading = wrapper.getComponent(FStartupLoading);
    expect(loading.get("img").attributes("src")).toBe("/scp-logo.png");
    for (const locale of ["zh-CN", "en-US"] as const) {
      i18n.global.locale.value = locale;
      await wrapper.vm.$nextTick();
      expect(loading.text()).toContain(i18n.global.t("common.loading"));
    }
    await wrapper.setProps({ modelState: "error" });
    expect(wrapper.findComponent(FStartupLoading).exists()).toBe(false);
    expect(wrapper.getComponent(Viewport).vm).toBe(viewport.vm);
    expect(viewport.props("modelState")).toBe("error");
    expect(viewport.attributes("inert")).toBeUndefined();
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = before;
  }
});

it("preserves scene inputs across selection, tool and time changes", async () => {
  const wrapper = mount(WorkbenchSchemaViewport, {
    props: { schema: document() },
    global,
  });
  try {
    const viewport = wrapper.getComponent(Viewport);
    const before = { ...viewport.props() };
    await wrapper.setProps({ selectedId: "decal:8", tool: "translate", timeOfDay: 18 } as never);
    for (const key of ["grouping", "lotColors", "lotSize", "hiddenUnits", "groupVisibility"] as const)
      expect(viewport.props(key)).toBe(before[key]);
    expect(viewport.attributes("selectedid")).toBe("decal:8");
    const next = document();
    next.lot.colors = [{ rgba: [1, 2, 3, 255] }];
    await wrapper.setProps({ schema: next });
    expect(viewport.props("lotColors")).toEqual([[1, 2, 3, 255]]);
    expect(viewport.props("grouping")).not.toBe(before.grouping);
  } finally { wrapper.unmount(); }
});

function document() {
  const unit = createPlacedDecal({ typeId: 0, group: 0, instance: 42 }, {
    packagePath: "Game.package", atlas: { typeId: 1, group: 2, instance: 3 },
  }, {
    category: 2, basis: [1, 0, 0, 0, -1, 0, 0, 0, -1],
    scale: 2, depth: 1, materialData: [0, 0, 0],
  }, 8, [3, 4, 0]);
  const schema = {
    $schema: "openscp.lot-asset/1", units: [schemaUnitJson(unit)],
    lot: { size: [32, 48], colors: [] }, editor: { groups: { decals: true } },
  };
  writeDecalSources(schema, [unit]);
  return JSON.parse(JSON.stringify(schema));
}

it.each([0x12345678, 0xe5390a98, 0x73684efc, 0x4491de3a])(
  "keeps dropped family %s renderable through the actual schema viewport boundary", async (materialInstance) => {
    const schema = document();
    const texture = {
      png: "png", category: 2, index: 8, materialInstance, aspectRatio: 1,
      idInstance: 42, atlasInstance: 3, width: 16, height: 16,
      error: null, shaderDefInstance: null, variant: null, quantized: false,
    };
    const wrapper = mount(WorkbenchSchemaViewport, {
      props: { schema }, attrs: { "decal-textures": [texture] },
      global,
    });
    try {
      const viewport = wrapper.getComponent(Viewport);
      const unit = viewport.props("grouping").decals[0] as DecalUnit;
      expect(unit.decalId?.instance).toBe(42);
      expect(unit.decalSource?.packagePath).toBe("Game.package");
      expect(viewport.props("decalTextures")).toEqual([texture]);
      const state = { projected: 0, fallback: 0, holeLightCount: 0, neonAnimated: false };
      const object = await buildDecalBody({
        THREE, unit, texture, decoded: new THREE.Texture(), proxies: [],
        env: createSunEnv(THREE),
        engineMaterials: true, decalLight: null, powered: true,
        isStale: () => false, state,
      });
      expect(state.fallback).toBe(1);
      expect(object?.children).toHaveLength(1);
      const mesh = object!.children[0] as THREE.Mesh<THREE.BufferGeometry, THREE.ShaderMaterial>;
      object!.updateMatrixWorld(true);
      expect(mesh.getWorldPosition(new THREE.Vector3()).z).toBeCloseTo(0.02);
      if (materialInstance === 0x4491de3a) {
        // The fallback's coordinate transform must use decal-local positions.
        const front = new THREE.Vector3(0, 0, 0)
          .applyMatrix4(mesh.material.uniforms.uHoleToBox.value);
        expect(front.toArray()).toEqual([0, 0, -1]);
        expect(mesh.material.fragmentShader).not.toContain("textureFloatPosition.z < 0.0");
      }
      mesh.geometry.dispose();
      mesh.material.dispose();
    } finally { wrapper.unmount(); }
  },
);

it("updates dropped metadata when the scene schema is replaced", async () => {
  const wrapper = mount(WorkbenchSchemaViewport, {
    props: { schema: document() }, global,
  });
  try {
    const next = document();
    next.units[0].decalSource.packagePath = "Other.package";
    next.units[0].transform.matrix[9] = 15;
    await wrapper.setProps({ schema: next });
    const unit = wrapper.getComponent(Viewport).props("grouping").decals[0];
    expect(unit.decalSource.packagePath).toBe("Other.package");
    expect(unit.transform.matrix[9]).toBe(15);
    expect(wrapper.getComponent(Viewport).props("lotSize")).toEqual([32, 48]);
  } finally { wrapper.unmount(); }
});
