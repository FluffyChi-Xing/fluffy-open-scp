import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/locales";
import BehaviorLeafSheet from "./BehaviorLeafSheet.vue";

describe("BehaviorLeafSheet", () => {
  const previousLocale = i18n.global.locale.value;
  const node = {
    id: "leaf-tourism-1",
    kind: "behavior-leaf",
    position: [0, 0],
  } as const;

  afterEach(() => {
    i18n.global.locale.value = previousLocale;
  });

  it("edits pure park capacities in both languages and rejects fractional seats", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body, global: { plugins: [i18n] },
      props: { node: { ...node, id: "park", position: [0, 0] },
        params: { type: "park", landValue: 500, attraction: 500 },
        labelKey: "studio.assetWorkbench.logicNode.leafPark", trigger: null },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const fields = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]');
      expect([...fields].map(input => input.value)).toEqual(["90", "80", "960"]);
      expect(fields[1].closest("label")?.textContent).toContain("儿童接待容量");
      i18n.global.locale.value = "en-US"; await nextTick();
      expect(fields[1].closest("label")?.textContent).toContain("Child capacity");
      fields[1].value = "12.5";
      fields[1].dispatchEvent(new Event("change", { bubbles: true })); await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toBeUndefined();
      fields[1].value = "120";
      fields[1].dispatchEvent(new Event("change", { bubbles: true })); await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")?.[0]?.[0]).toMatchObject({ childrenCapacity: 120 });
    } finally { wrapper.unmount(); }
  });

  it("translates the retail capacity controls and saves a custom value", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-commercial-1", position: [0, 0] },
        params: { type: "commercial" },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const shoppers = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]')[1];
      expect(shoppers.value).toBe("60");
      expect(shoppers.closest("label")?.textContent?.trim()).toBe("中收入顾客 ($$)");
      const goods = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]')[3];
      expect(goods.closest("label")?.textContent?.trim()).toBe("低价值商品数量 ($)");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(shoppers.closest("label")?.textContent?.trim()).toBe("Medium-wealth shoppers ($$)");
      expect(goods.closest("label")?.textContent?.trim()).toBe("Low-value goods ($)");
      goods.value = "123";
      goods.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      shoppers.value = "15";
      shoppers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[{
        type: "commercial", commercialProfile: "ground-mall-mixed-v1",
        shoppers: undefined, shoppersW1: 60, shoppersW2: 15, shoppersW3: 60, goodsW1: 123, goodsW2: 750, goodsW3: 375,
      }]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("leaves staffing in Building Properties while editing retail", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "commercial", shoppers: 20, goods: 20 },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const fields = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]');
      expect(fields).toHaveLength(6);
      expect(dialog.textContent).toContain("建筑属性统一设置");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(dialog.textContent).toContain("Set jobs and shifts in Building Properties");
      expect(dialog.querySelector('[role="alert"]')).toBeNull();

    } finally {
      wrapper.unmount();
    }
  });

  it("adds another customer tier while keeping the saved tier and goods", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-retail-wealth", position: [0, 0] },
        params: { type: "commercial", commercialProfile: "ground-commercial-w2-v1", shoppers: 6 },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const fields = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]');
      expect([...fields].map((field) => field.value)).toEqual(["0", "6", "0", "36", "18", "9"]);
      fields[0].value = "25";
      fields[0].dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.textContent).toContain("共享接待容量 31");
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "commercial", commercialProfile: "ground-mall-mixed-v1", shoppers: undefined,
          shoppersW1: 25, shoppersW2: 6, shoppersW3: 0, goodsW1: 36, goodsW2: 18, goodsW3: 9 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("uses template placeholders, translates the open sheet, and applies only configured values", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "tourism", tourismProfile: "native-tourism-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafTourism",
        trigger: null,
      },
    });
    try {
      await nextTick();
      expect(
        document.querySelector('[role="dialog"] h2')?.textContent?.trim(),
      ).toBe("编辑观光");
      const attraction = document.querySelector<HTMLInputElement>(
        'input[placeholder="400"]',
      )!;
      expect(attraction.value).toBe("");
      expect(wrapper.emitted("apply")).toBeUndefined();

      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(
        document.querySelector('[role="dialog"] h2')?.textContent?.trim(),
      ).toBe("Edit Tourism");
      expect(document.querySelector('[role="switch"]')).toBeNull();

      attraction.value = "275";
      attraction.dispatchEvent(new Event("input", { bubbles: true }));
      await nextTick();
      document
        .querySelector("form.behavior-sheet-form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
      expect(wrapper.emitted("apply")).toEqual([
        [
          {
            type: "tourism",
            tourismProfile: "native-tourism-v1",
            attraction: 275,
          },
        ],
      ]);
    } finally {
      wrapper.unmount();
    }
  });

  it("edits hotel rooms separately from tourism and checks the native bin limit", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "hotel" },
        labelKey: "studio.assetWorkbench.logicNode.leafHotel",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const input = dialog.querySelector<HTMLInputElement>('input[placeholder="68"]')!;
      expect(input.closest("label")?.textContent?.trim()).toBe("客房数量");
      i18n.global.locale.value = "en-US";
      input.value = "1001";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(input.closest("label")?.textContent?.trim()).toBe("Guest rooms");
      expect(dialog.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true);
      input.value = "84";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[{ type: "hotel", rooms: 84 }]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("edits the Copan resident capacity and translates the active control", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-residence-1", position: [0, 0] },
        params: { type: "residence", residenceProfile: "copan-residence-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafResidence",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const input = document.querySelector<HTMLInputElement>('input[placeholder="1100"]')!;
      expect(input.value).toBe("");
      expect(input.closest("label")?.textContent?.trim()).toBe("住户容量（人数）");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(input.closest("label")?.textContent?.trim()).toBe("Resident capacity");
      const recipe = document.querySelector<HTMLButtonElement>('[role="dialog"] .f-select')!;
      expect(recipe.textContent?.trim()).toBe("Copan Building");
      recipe.click();
      await nextTick();
      [...document.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')]
        .find((option) => option.textContent?.trim() === "MegaTower § apartment")!.click();
      await nextTick();
      input.value = "550";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      input.closest("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "residence", residenceProfile: "megatower-residence-v1", population: 550 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("offers three residence tiers and edits shopper and happiness capacities", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-ground-residence", position: [0, 0] },
        params: { type: "residence", residenceProfile: "ground-residence-w1-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafResidence",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      dialog.querySelector<HTMLButtonElement>(".f-select")!.click();
      await nextTick();
      const options = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')];
      expect(options.slice(0, 4).map((option) => option.textContent?.trim()))
        .toEqual(["低收入住宅 ($)", "中收入住宅 ($$)", "高收入住宅 ($$$)", "低收入高密住宅 ($)"]);
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(options[3].textContent?.trim()).toBe("High-density low-wealth residence ($)");
      options[3].click();
      await nextTick();
      expect(dialog.querySelector('input[placeholder="6600"]')).not.toBeNull();
      dialog.querySelector<HTMLButtonElement>(".f-select")!.click();
      await nextTick();
      [...document.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')]
        .find((option) => option.textContent?.trim() === "High-wealth residence ($$$)")!.click();
      await nextTick();
      const workers = dialog.querySelector<HTMLInputElement>('input[placeholder="7"]')!;
      expect(workers.closest("label")?.textContent?.trim()).toBe("Resident worker capacity");
      const shoppers = dialog.querySelector<HTMLInputElement>('input[placeholder="3"]')!;
      expect(shoppers.closest("label")?.textContent?.trim()).toBe("Resident shopper capacity");
      expect(shoppers.readOnly).toBe(false);
      workers.value = "11";
      workers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true);
      workers.value = "6";
      workers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(shoppers.placeholder).toBe("4");
      shoppers.value = "2";
      shoppers.dispatchEvent(new Event("change", { bubbles: true }));
      const happiness = dialog.querySelector<HTMLInputElement>('input[placeholder="110"]')!;
      happiness.value = "120";
      happiness.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(workers.placeholder).toBe("8");
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "residence", residenceProfile: "ground-residence-w3-v1", residentWorkers: null,
          residentShoppers: 2, happinessCapacity: 120 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("translates office fields in the open sheet and rejects an invalid workforce", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-office-1", position: [0, 0] },
        params: { type: "office" },
        labelKey: "studio.assetWorkbench.logicNode.leafOffice",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const inputs = Array.from(dialog.querySelectorAll<HTMLInputElement>('input[type="number"]'));
      expect(inputs).toHaveLength(0);
      expect(dialog.textContent).toContain("在建筑属性页设置岗位");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(dialog.textContent).toContain("Set jobs, minimum staffing, and shifts in Building Properties");
    } finally {
      wrapper.unmount();
    }
  });
});
