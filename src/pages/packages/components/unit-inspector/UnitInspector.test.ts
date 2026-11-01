import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import type { LotUnitDto } from "@/api/tauri";
import { i18n } from "@/locales";
import UnitInspector from "./UnitInspector.vue";
import { anchorKey, fnv1NameId } from "./spawnerAnchor";

vi.mock("@/api", () => ({
  isTauri: () => false,
  tauriApi: {
    renderTelemetry: {
      record: vi.fn().mockResolvedValue(0),
      summary: vi.fn().mockResolvedValue(null),
      clear: vi.fn().mockResolvedValue({ rows: 0 }),
    },
    packages: {
      readLotEditorSession: vi.fn().mockResolvedValue({ units: [] }),
    },
  },
}));

function spawnerUnit(index: number, instance: number): LotUnitDto {
  return {
    kind: "spawner",
    index,
    transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, index, 0, 0] },
    id: anchorKey(instance),
    count: null,
    countRandom: null,
    agent: null,
    fields: [],
  } as unknown as LotUnitDto;
}

function mountInspector(unit: LotUnitDto | null, units: LotUnitDto[] = unit ? [unit] : []) {
  return mount(UnitInspector, {
    props: { unit, units, editEnabled: true },
    global: { plugins: [i18n] },
  });
}

describe("UnitInspector 生成器面板", () => {
  it("选中生成器单元时，属性页签先给生成器面板再给原始属性行", () => {
    const wrapper = mountInspector(spawnerUnit(0, 0x141eed53));
    const panel = wrapper.findComponent({ name: "SpawnerUnitPanel" });
    expect(panel.exists()).toBe(true);
    expect(wrapper.text()).toContain("0x141EED53");
    // 属性页签仍保留原始属性表
    expect(
      wrapper.findComponent({ name: "PropertyEditorProperties" }).exists(),
    ).toBe(true);
  });

  it("未选中单元时不渲染任何生成器区域", () => {
    const wrapper = mountInspector(null);
    expect(wrapper.findComponent({ name: "SpawnerUnitPanel" }).exists()).toBe(false);
    // 页签本身仍是四个（不新增第五个页签）
    expect(wrapper.findAll('[role="tab"]')).toHaveLength(4);
  });

  it("非生成器单元不渲染生成器面板", () => {
    const light = {
      kind: "light",
      index: 0,
      transform: { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0] },
      lightType: "Point",
      color: [1, 1, 1],
      outerRadius: 8,
      innerRadius: null,
      diffuse: null,
      length: null,
      cullDistance: "Far",
      isVolumetric: false,
      debugName: null,
      fields: [],
    } as unknown as LotUnitDto;
    const wrapper = mountInspector(light);
    expect(wrapper.findComponent({ name: "SpawnerUnitPanel" }).exists()).toBe(false);
  });

  it("写入名字时按 FNV-1 换算并带上 type/group = 0 的 key", async () => {
    const unit = spawnerUnit(0, 0x141eed53);
    const wrapper = mountInspector(unit);
    await wrapper.find('[data-testid="spawner-name-input"]').setValue("NorthDoor");
    const commit = wrapper.find('[data-testid="spawner-commit-name"]');
    expect(commit.attributes("disabled")).toBeUndefined();
    await commit.trigger("click");
    const events = wrapper.emitted("update-fields");
    expect(events).toBeTruthy();
    const [id, patch] = events![0] as [string, { id: { typeId: number; group: number; instance: number } }];
    expect(id).toBe("spawner:0");
    expect(patch.id).toEqual({
      typeId: 0,
      group: 0,
      instance: fnv1NameId("NorthDoor"),
    });
  });
});
