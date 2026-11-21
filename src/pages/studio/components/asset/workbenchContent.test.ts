import { expect, it } from "vitest";
import { workbenchContentKey } from "./workbenchContent";

it("keeps layer visibility, selection and JSON indentation out of asset dirty state", () => {
  const asset = { units: [{ kind: "prop", visible: true, transform: { matrix: [1, 2, 3] } }], editor: { groups: { props: true }, unitVisibility: {}, workbench: { menu: { title: "name" } } } };
  const baseline = workbenchContentKey(asset);
  const view = JSON.parse(JSON.stringify(asset, null, 2));
  view.units[0].visible = false;
  view.editor.groups.props = false;
  view.editor.unitVisibility = { "prop:0:0": false };
  view.editor.selectedId = "model";
  expect(workbenchContentKey(view)).toBe(baseline);
  view.units[0].transform.matrix[0] = 5;
  expect(workbenchContentKey(view)).not.toBe(baseline);
  view.units[0].transform.matrix[0] = 1;
  expect(workbenchContentKey(view)).toBe(baseline);
  view.editor.workbench.menu.title = "edited";
  expect(workbenchContentKey(view)).not.toBe(baseline);
});
