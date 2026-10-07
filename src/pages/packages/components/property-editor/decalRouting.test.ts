import { describe, expect, it } from "vitest";
import { decalRoute } from "./decalRouting";

describe("native decal material routing", () => {
  it("renders the sign tube pass on its own plane", () => {
    expect(decalRoute(0x73684efc)).toEqual({ family: "sdf", surface: "plane" });
  });
  it("keeps graffiti and interior maps projected even when no wall is hit", () => {
    expect(decalRoute(0xe5390a98)).toEqual({ family: "sign", surface: "projected" });
    expect(decalRoute(0x4491de3a)).toEqual({ family: "hole", surface: "projected" });
    expect(decalRoute(null).surface).toBe("projected");
  });
});
