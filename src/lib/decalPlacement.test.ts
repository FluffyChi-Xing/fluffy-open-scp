import { expect, it } from "vitest";
import { decalPlacementKind } from "./decalPlacement";

it("identifies the verified game material chains without guessing unknown families", () => {
  expect(decalPlacementKind(0x73684efc)).toBe("floating");
  expect(decalPlacementKind(0xe5390a98)).toBe("projected");
  // The hole's float-quad VS does not bypass receiver-depth clipping.
  expect(decalPlacementKind(0x4491de3a)).toBe("projected");
  for (const material of [null, undefined, 0, 0x44a3f5ba, 0x12345678]) {
    expect(decalPlacementKind(material)).toBe("unknown");
  }
});
