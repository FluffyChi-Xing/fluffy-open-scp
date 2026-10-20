import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { describe, expect, it } from "vitest";
import { createHudRender } from "./render";
import type { ReplicaData } from "./types";

function fixture(): ReplicaData {
  const context = { window: {} as Record<string, unknown> };
  for (const file of ["layout", "assets", "model"]) {
    runInNewContext(readFileSync(`public/game-ui/replica/data/${file}.js`, "utf8"), context);
  }
  return {
    layout: context.window.HUD_LAYOUT as ReplicaData["layout"],
    assets: context.window.ASSET_MAP as ReplicaData["assets"],
    model: context.window.CATEGORY_MODEL as ReplicaData["model"],
    locale: {}, tools: {},
    palette: { byCategory: {}, row: { pitch: 1, startX: 0, slotW: 1, slotH: 1, topInBar: 0 } },
  };
}

function render(data: ReplicaData, mode: "city" | "region" | "development") {
  const stage = document.createElement("div");
  Object.defineProperties(stage, { clientWidth: { value: 1600 }, clientHeight: { value: 900 } });
  createHudRender(data, { mode }).render(stage);
  return stage;
}

describe("HUD selection", () => {
  it.each(["city", "region", "development"] as const)("selects only the %s view, including after hover", (mode) => {
    const data = fixture();
    const original = JSON.stringify(data.layout);
    const stage = render(data, mode);
    const buttons = [...stage.querySelectorAll<HTMLElement>(".hud-mode-button")];
    expect(buttons).toHaveLength(3);
    expect(buttons.filter((b) => b.classList.contains("layer-selected")).map((b) => b.dataset.mode)).toEqual([mode]);
    for (const button of buttons) {
      const selected = button.dataset.mode === mode;
      expect(button.style.backgroundImage).toContain(selected ? "Rounded_Selected.png" : "Rounded_Normal.png");
      button.dispatchEvent(new MouseEvent("mouseenter"));
      expect(button.style.backgroundImage).toContain(selected ? "Rounded_SelectedHigh.png" : "Rounded_Highlighted.png");
      button.dispatchEvent(new MouseEvent("mouseleave"));
      expect(button.getAttribute("aria-pressed")).toBe(String(selected));
    }
    expect(JSON.stringify(data.layout)).toBe(original);
  });

  it("retains the original tab shoulders as a selection layer and initializes cloned geometry", () => {
    const stage = render(fixture(), "city");
    const button = stage.querySelector(".cat-btn")!;
    const tab = button.querySelector<HTMLElement>('[data-comment="tab"]')!;
    expect(tab.classList.contains("layer-select")).toBe(true);
    expect(tab.classList.contains("layer-off")).toBe(false);
    expect(tab.style.backgroundImage).toContain("category_tab.png");
    const notificationRing = button.querySelector('[data-comment="ring on"]')!;
    expect(notificationRing.classList.contains("layer-off")).toBe(true);
    expect(notificationRing.classList.contains("layer-select")).toBe(false);
    // These stretched children must retain their authored dimensions after cloning.
    const glow = button.querySelector<HTMLElement>('[data-comment="tool unlocked"]')!;
    expect(parseFloat(glow.style.width)).toBeCloseTo(92.52, 2);
  });
});
