/** @summary Preserve the established authoring cascade and gradient behavior while changing resource transport. */
import { describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { compatibilityHudGradients } from "../local-assets";
import { COMPATIBILITY_BASE_STYLE_IDS, COMPATIBILITY_HUD_STYLE_IDS } from "./styles";

describe("original HUD stylesheet contract", () => {
  it("preserves base and generated HUD cascade order", () => {
    expect(COMPATIBILITY_BASE_STYLE_IDS).toEqual(["92319616", "40969046", "8145AF53", "D743498E", "0292AFE2", "14D97BD3", "06DEC864", "CD6DD755"]);
    expect(COMPATIBILITY_HUD_STYLE_IDS).toEqual(["92319616", "40969046", "24E582BA", "7F44F9EE", "8145AF53", "06DEC864", "CD6DD755", "0292AFE2", "D743498E", "14D97BD3"]);
  });
  it("converts balanced gradients without changing historical direction semantics", () => {
    expect(compatibilityHudGradients("a{-moz-linear-gradient(50% -0% -90deg,\n rgba(1, 2, 3, 1) 0%,\n rgba(4, 5, 6, 1) 100%)}"))
      .toBe("a{linear-gradient(-90deg, rgba(1, 2, 3, 1) 0%, rgba(4, 5, 6, 1) 100%)}");
    expect(compatibilityHudGradients("a{-moz-linear-gradient(right, red, blue);-moz-linear-gradient(45deg, black, white)}"))
      .toBe("a{linear-gradient(180deg, red, blue);linear-gradient(45deg, black, white)}");
    expect(() => compatibilityHudGradients("-moz-linear-gradient(top, rgba(0,0,0,1)")).toThrow();
  });
});
