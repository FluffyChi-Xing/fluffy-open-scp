import { describe, expect, it } from "vitest";
import {
  deriveSemanticsMenu,
  emptyMenuBinding,
  menuBindingIssues,
  menuTargetLabelKey,
  menuTargetsFor,
  newMenuEntry,
  newMenuEntryId,
  normalizeMenuBinding,
} from "./menuBinding";

describe("normalizeMenuBinding", () => {
  it("defends against non-object and empty documents", () => {
    expect(normalizeMenuBinding(undefined)).toEqual(emptyMenuBinding());
    expect(normalizeMenuBinding("junk")).toEqual(emptyMenuBinding());
    expect(normalizeMenuBinding(null)).toEqual(emptyMenuBinding());
  });

  it("migrates the legacy {enabled,label,category} shape into one city entry", () => {
    const binding = normalizeMenuBinding({ enabled: true, label: "科洛大厦", category: "住宅" });
    expect(binding.enabled).toBe(true);
    expect(binding.label).toBe("科洛大厦");
    expect(binding.description).toBe("");
    expect(binding.uiPosition).toBeNull();
    expect(binding.entries).toEqual([
      { id: "menu-1", dimension: "city", categoryInstance: "" },
    ]);
  });

  it("does not spawn an entry from an untouched legacy block", () => {
    expect(normalizeMenuBinding({ enabled: false, label: "", category: "" }).entries).toEqual([]);
  });

  it("keeps v2 entries, uppercases instances and drops duplicates/malformed rows", () => {
    const binding = normalizeMenuBinding({
      enabled: true,
      label: "科洛大厦",
      description: "测试",
      uiPosition: 12000,
      entries: [
        { id: "menu-1", dimension: "development", categoryInstance: "94e412ed" },
        { id: "menu-1", dimension: "city", categoryInstance: "899BEA70" },
        { junk: true },
        { id: "menu-3", dimension: "other", categoryInstance: 42 },
      ],
    });
    expect(binding.entries).toEqual([
      { id: "menu-1", dimension: "development", categoryInstance: "94E412ED" },
      { id: "menu-3", dimension: "city", categoryInstance: "" },
    ]);
    expect(binding.uiPosition).toBe(12000);
  });
});

describe("newMenuEntryId", () => {
  it("continues the numeric suffix sequence", () => {
    expect(newMenuEntryId([])).toBe("menu-1");
    expect(
      newMenuEntryId([
        { id: "menu-2", dimension: "city", categoryInstance: "" },
        { id: "menu-5", dimension: "city", categoryInstance: "" },
      ]),
    ).toBe("menu-6");
  });

  it("ignores foreign ids", () => {
    expect(newMenuEntryId([{ id: "units-a", dimension: "city", categoryInstance: "" }])).toBe("menu-1");
  });

  it("newMenuEntry defaults to the city dimension with no category", () => {
    expect(newMenuEntry([])).toEqual({ id: "menu-1", dimension: "city", categoryInstance: "" });
  });
});

describe("menuTargetsFor / menuTargetLabelKey", () => {
  it("exposes 14 city and 9 development targets", () => {
    expect(menuTargetsFor("city")).toHaveLength(14);
    expect(menuTargetsFor("development")).toHaveLength(9);
  });

  it("resolves label keys per dimension and rejects cross-dimension ids", () => {
    expect(menuTargetLabelKey("city", "899bea70")).toBe("studio.assetWorkbench.menuTargets.park");
    expect(menuTargetLabelKey("development", "94E412ED")).toBe(
      "studio.workbench.developmentMenus.culture",
    );
    expect(menuTargetLabelKey("city", "94E412ED")).toBeNull();
    expect(menuTargetLabelKey("city", "")).toBeNull();
  });
});

describe("deriveSemanticsMenu", () => {
  it("returns nothing (without issues) when the binding is disabled", () => {
    const binding = { ...emptyMenuBinding(), enabled: false };
    expect(deriveSemanticsMenu(binding)).toEqual({ values: null, reasons: [] });
    expect(menuBindingIssues(binding)).toEqual([]);
  });

  it("fails closed on missing title or unchosen categories", () => {
    const binding: ReturnType<typeof emptyMenuBinding> = {
      ...emptyMenuBinding(),
      enabled: true,
      entries: [
        { id: "menu-1", dimension: "city", categoryInstance: "" },
        { id: "menu-2", dimension: "city", categoryInstance: "899BEA70" },
      ],
    };
    const derived = deriveSemanticsMenu(binding);
    expect(derived.values).toBeNull();
    expect(derived.reasons).toEqual(["menuLabelRequired", "menuCategoryRequired:menu-1"]);
  });

  it("projects title, description, uiPosition and hex categoryInstances", () => {
    const binding: ReturnType<typeof emptyMenuBinding> = {
      enabled: true,
      label: "  科洛大厦  ",
      description: " 测试建筑 ",
      uiPosition: 12000,
      iconPng: "",
      marqueePng: "",
      constructionCost: null,
      maintenanceCost: null,
      entries: [
        { id: "menu-1", dimension: "city", categoryInstance: "899BEA70" },
        { id: "menu-2", dimension: "development", categoryInstance: "94E412ED" },
      ],
    };
    expect(deriveSemanticsMenu(binding)).toEqual({
      values: {
        title: "科洛大厦",
        description: "测试建筑",
        uiPosition: 12000,
        categoryInstances: ["0x899BEA70", "0x94E412ED"],
      },
      reasons: [],
    });
  });

  it("omits empty description and null position", () => {
    const binding: ReturnType<typeof emptyMenuBinding> = {
      enabled: true,
      label: "科洛大厦",
      description: "",
      uiPosition: null,
      iconPng: "",
      marqueePng: "",
      constructionCost: null,
      maintenanceCost: null,
      entries: [{ id: "menu-1", dimension: "city", categoryInstance: "899BEA70" }],
    };
    const { values } = deriveSemanticsMenu(binding);
    expect(values).toEqual({
      title: "科洛大厦",
      categoryInstances: ["0x899BEA70"],
    });
  });
});
