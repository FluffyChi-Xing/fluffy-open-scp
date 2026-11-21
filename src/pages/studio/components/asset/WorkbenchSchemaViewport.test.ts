import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import * as THREE from "three";
import { expect, it } from "vitest";
import type { DecalUnit } from "@/api/tauri";
import { schemaUnitJson } from "@/pages/packages/components/property-editor/peSchemaDoc";
import { createSunEnv } from "@/pages/packages/components/property-editor/refinedRender";
import { buildDecalBody } from "@/render/exec/three/decalBody";
import { createPlacedDecal, writeDecalSources } from "./workbenchDecals";
import WorkbenchSchemaViewport from "./WorkbenchSchemaViewport.vue";

const Viewport = defineComponent({
  name: "PropertyEditorViewport",
  props: {
    grouping: { type: Object, required: true }, decalTextures: Array,
    lotSize: Array, hiddenUnits: Set, groupVisibility: Object,
  },
  template: "<div />",
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
      global: { stubs: { PropertyEditorViewport: Viewport } },
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
    props: { schema: document() }, global: { stubs: { PropertyEditorViewport: Viewport } },
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
