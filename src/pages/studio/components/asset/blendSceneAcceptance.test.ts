import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { BlendSceneDocument } from "./blendSceneTypes";
import {
  schemaUnitJson,
  unitsFromSchema,
} from "@/pages/packages/components/property-editor/peSchemaDoc";
import {
  maskMatchesMeta,
  parseAuthoredLotMeta,
  schemaFromAuthored,
} from "@/lib/lot-ground/authored";
import {
  writeSceneLightControls,
  restoreSceneLightControls,
} from "./blendSceneProjection";
import { i18n } from "@/locales";

it("updates scene failure messages when switching languages", () => {
  const before = i18n.global.locale.value;
  try {
    i18n.global.locale.value = "zh-CN";
    expect(
      i18n.global.t("studio.assetWorkbench.blendSceneInitializationFailed"),
    ).toContain("模型已导入");
    i18n.global.locale.value = "en-US";
    expect(
      i18n.global.t("studio.assetWorkbench.blendSceneInitializationFailed"),
    ).toContain("Model imported");
  } finally {
    i18n.global.locale.value = before;
  }
});

describe.skipIf(!process.env.OPENSCP_TEST_SCENE)(
  "user blend scene acceptance",
  () => {
    it("initializes schema units and a valid editable authored lot from the native cache", () => {
      const scene = JSON.parse(
        readFileSync(process.env.OPENSCP_TEST_SCENE!, "utf8"),
      ) as BlendSceneDocument;
      expect(scene.errors).toEqual([]);
      const doc = {
        $schema: "openscp.lot-asset/1",
        units: scene.units.map(schemaUnitJson),
      };
      writeSceneLightControls(doc, scene.units);
      const restored = unitsFromSchema(JSON.parse(JSON.stringify(doc)));
      restoreSceneLightControls(doc, restored);
      expect(restored.filter((u) => u.kind === "light")).toHaveLength(31);
      expect(restored.filter((u) => u.kind === "prop")).toHaveLength(8);
      expect(restored.filter((u) => u.kind === "effect")).toHaveLength(1);
      const lot = scene.lot!;
      expect(parseAuthoredLotMeta(JSON.stringify(lot.meta))).not.toBeNull();
      expect(maskMatchesMeta(lot.meta, lot.mask)).toBe(true);
      const editable = schemaFromAuthored(lot);
      expect(editable.profile).toBe("lot-mask");
      expect(editable.image?.width).toBe(1024);
    });
  },
);
