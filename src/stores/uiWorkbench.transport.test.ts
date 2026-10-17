/** @summary Changing display-resource transport must not change exported mods or turn installed images into authored media. */
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, expect, it } from "vitest";
import type { ReplicaData } from "@/lib/game-ui/replica/types";
import { useUiWorkbenchStore } from "./uiWorkbench";

const originalImage = "data:image/png;base64,iVBORw0KGgo=";
const authoredImage = "data:image/png;base64,iVBORw0KGgoAAA==";
/** @summary Minimal authoring model with a source image whose transport can vary independently of behavior. */
function fixture(preview: string): ReplicaData {
  return { layout: {}, assets: {}, locale: {}, model: { categories: [{ id: "AC12C904", label: "Roads", icon: null, iconHash: null }], row: { pitch: 46.85, size: 43, centerX: 875.5, offsetY: 0 } }, tools: { AC12C904: [{ instance: "7DD8A627", label: "Road", preview, marquee: preview, locked: false }] }, palette: { byCategory: {}, row: { pitch: 121.6, startX: 245, slotW: 121.6, slotH: 50, topInBar: 4 } } };
}
beforeEach(() => setActivePinia(createPinia()));

it("exports the same package for a label edit regardless of installed image transport", () => {
  const store = useUiWorkbenchStore();
  store.hydrate(fixture("/game-ui/replica/assets/original.png"));
  store.updateItem("AC12C904", "7DD8A627", { label: "Edited" });
  const baseline = store.buildGameExport().bytes;
  store.resetAll(); store.hydrate(fixture(originalImage));
  store.updateItem("AC12C904", "7DD8A627", { label: "Edited", preview: originalImage, marquee: originalImage });
  expect(store.buildGameExport().bytes).toEqual(baseline);
  expect(store.exportOverlay()).not.toContain(originalImage);
});

it("blocks copies of installed images but preserves genuinely authored crops", () => {
  const store = useUiWorkbenchStore(); store.hydrate(fixture(originalImage));
  store.addItem("AC12C904", "New", null);
  const id = store.added.AC12C904!.at(-1)!.id;
  store.updateItem("AC12C904", id, { preview: originalImage });
  expect(store.exportOverlay()).not.toContain(originalImage);
  store.updateItem("AC12C904", "7DD8A627", { label: "Edited" });
  const withoutCrop = store.buildGameExport().bytes;
  store.updateItem("AC12C904", "7DD8A627", { preview: authoredImage });
  expect(store.exportOverlay()).toContain(authoredImage);
  expect(store.buildGameExport().bytes.length).toBeGreaterThan(withoutCrop.length);
});

it("protects overlay export even if an editor assigns an unchanged source image directly", () => {
  const store = useUiWorkbenchStore(); store.hydrate(fixture(originalImage));
  store.edits["7DD8A627"] = { label: "Edited", preview: originalImage, marquee: originalImage };
  expect(store.exportOverlay()).not.toContain(originalImage);
});
