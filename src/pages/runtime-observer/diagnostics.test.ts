import { describe, expect, it } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { i18n } from "@/locales";
import resource from "./fixtures/resource.json";
import ui from "./fixtures/ui.json";
import engine from "./fixtures/engine.json";
import globalEvent from "./fixtures/global-event.json";
import rules from "./fixtures/rules.json";
import identities from "./fixtures/rule-identities.json";
import provenance from "./fixtures/rule-provenance.json";
import definitions from "./fixtures/rule-definition.json";
import effects from "./fixtures/rule-effects.json";
import unitEffects from "./fixtures/unit-effects.json";
import ResourceEffects from "./ResourceEffects.vue";
import conditions from "./fixtures/rule-conditions.json";
import RuleEligibility from "./RuleEligibility.vue";
import { captureQuality } from "./captureQuality";
import {
  analyze,
  parseTrace,
  semanticCategory,
  selectEvidence,
} from "./diagnostics";
import Workbench from "./DiagnosticWorkbench.vue";
import QualityPanel from "./CaptureQuality.vue";
import LogStream from "./LogStream.vue";
import FSelect from "@/components/ui/FSelect.vue";

describe("diagnostic evidence", () => {
  it("classifies static evidence by semantic category and filters the result list", async () => {
    const consumption = analyze(unitEffects, "consumption").rows[0];
    const production = analyze(effects, "production").rows[0];
    expect(semanticCategory(consumption)).toBe("resource-production");
    expect(semanticCategory(production)).toBe("resource-production");
    const wrapper = mount(Workbench, { global: { plugins: [i18n] } });
    const input = wrapper.find("input[type=file]");
    Object.defineProperty(input.element, "files", {
      value: [
        {
          name: "categories.jsonl",
          size: 100,
          text: async () =>
            [...unitEffects, effects[0]]
              .map((r) => JSON.stringify(r))
              .join("\n"),
        },
      ],
    });
    await input.trigger("change");
    await flushPromises();
    expect(wrapper.findAll(".category-resource-production")).toHaveLength(2);
    await wrapper
      .findComponent(FSelect)
      .vm.$emit("update:modelValue", "resource-production");
    await flushPromises();
    expect(wrapper.findAll(".evidence-row")).toHaveLength(2);
    i18n.global.locale.value = "zh-CN";
    wrapper.unmount();
  });
  it("colors semantic live log categories and exposes structured hover details", async () => {
    const w = mount(LogStream, {
      props: {
        rows: [
          {
            sequence: 12,
            time: Date.now(),
            channel: "simulation.effect",
            phase: "unit-local-resource-update",
            ruleId: "0xd715d72f",
            unitSlot: 889,
            operations: [{ resourceId: "0xe71f36b7", input: 1, output: 0 }],
          },
          {
            sequence: 13,
            time: Date.now(),
            channel: "engine.warning",
            text: "probe warning",
          },
        ],
      },
      global: { plugins: [i18n] },
    });
    expect(w.findAll(".tone-resource")).toHaveLength(1);
    expect(w.findAll(".tone-engine")).toHaveLength(1);
    expect(w.text()).toContain("资源");
    expect(w.text()).toContain("通道");
    expect(w.text()).toContain("0xd715d72f");
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.text()).toContain("Resource");
    expect(w.text()).toContain("Semantic category");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("retains unit-local ownership without inventing a stable identity and renders the measured update", () => {
    const row = analyze(unitEffects, "real-unit-effects").rows[0];
    expect(row.source).toBe("simulation");
    expect(row.stage).toBe("unit-local-resource-update");
    expect(row.fields).toMatchObject({
      unitSlot: 189,
      unitPointer: "0x45a2b1a8",
      unitStamp: null,
      resourceOwner: "unit-local",
    });
    expect(row.fields).not.toHaveProperty("success");
    expect(selectEvidence([row], "simulation", "9e3c3dfa")).toHaveLength(1);
    const wrapper = mount(ResourceEffects, {
      props: { operations: unitEffects[0].operations, query: "9e3c3dfa" },
      global: { plugins: [i18n] },
    });
    expect(
      wrapper
        .findAll("tbody tr")[0]
        .findAll("td")
        .map((c) => c.text()),
    ).toEqual([
      "0x9e3c3dfa",
      "586",
      "864",
      "278",
      "864",
      "864",
      "已观察",
      "一致",
    ]);
    wrapper.unmount();
  });
  it("keeps eligibility separate from measured changes and exposes the failing resource to search", () => {
    const rows = analyze(conditions, "real-conditions").rows;
    expect(
      rows.every(
        (r) => r.source === "simulation" && r.stage === "global-eligibility",
      ),
    ).toBe(true);
    expect(
      selectEvidence(rows, "simulation", "capacity-shortfall"),
    ).toHaveLength(1);
    expect(selectEvidence(rows, "simulation", "80550550")).toHaveLength(1);
    for (const row of rows) {
      expect(row.fields).not.toHaveProperty("resourceEffects");
      expect(row.fields).not.toHaveProperty("success");
    }
  });
  it("explains real capacity rejection with localized evidence and does not claim allowed means executed", async () => {
    const rejected = conditions.find(
      (r) => r.diagnosis.reason === "capacity-shortfall",
    )!;
    const w = mount(RuleEligibility, {
      props: { evidence: rejected, query: "98ec1f24" },
      global: { plugins: [i18n] },
    });
    expect(w.text()).toContain("剩余容量不足");
    expect(w.text()).toContain("当前数量 24 / 容量 24");
    expect(w.find("mark").text()).toBe("98ec1f24");
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.text()).toContain("Insufficient remaining capacity");
    await w.setProps({
      evidence: conditions.find((r) => r.diagnosis.reason === "allowed"),
    });
    expect(w.text()).toContain("execution not yet proven");
    await w.setProps({ evidence: { diagnosis: { reason: "unresolved" } } });
    expect(w.text()).toContain("Reason unresolved");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("imports measured effects and searches resource IDs without promoting a kernel result to rule success", () => {
    const rows = analyze(effects, "real-effects").rows;
    expect(rows[0].stage).toBe("global-resource-update");
    expect(rows[0].source).toBe("simulation");
    expect(rows[0].fields.resourceEffects).toEqual(effects[0].operations);
    expect(rows[0].fields).not.toHaveProperty("success");
    expect(selectEvidence(rows, "simulation", "c9fb4c92")).toHaveLength(2);
  });
  it("renders actual amounts, unknown observations and resource highlighting in both languages", async () => {
    const w = mount(ResourceEffects, {
      props: {
        operations: [
          ...effects[0].operations,
          { resourceId: "0xmissing", status: "not-reached" },
        ],
        query: "c9fb4c92",
      },
      global: { plugins: [i18n] },
    });
    expect(w.find("mark").text()).toBe("c9fb4c92");
    expect(
      w
        .findAll("tbody tr")[0]
        .findAll("td")
        .map((c) => c.text()),
    ).toEqual(["0xc9fb4c92", "2", "3", "1", "24", "24", "已观察", "一致"]);
    expect(w.findAll("tbody tr")[1].text()).toContain("未知");
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.text()).toContain("Measured delta");
    expect(w.text()).toContain("Execution not observed");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("searches verified rule definitions without inventing measured bin changes", () => {
    const rows = analyze(definitions, "real-definition").rows;
    expect(rows[0].fields.ruleDefinition).toEqual(
      definitions[0].ruleDefinition,
    );
    expect(selectEvidence(rows, "simulation", "0xc9fb4c92")).toHaveLength(1);
    expect(rows[0].fields).not.toHaveProperty("beforeAmount");
    expect(rows[0].fields).not.toHaveProperty("afterAmount");
    expect(rows[0].fields).not.toHaveProperty("ruleName");
  });
  it("preserves real verified provenance with distinct abstract and disk keys", () => {
    // PID 7424 request 1, sequences 6/7/9; only installation root sanitized.
    const rows = analyze(provenance, "real-provenance").rows;
    expect(rows.map((r) => r.stage)).toEqual([
      "rule-resource-record",
      "rule-resource-create",
      "rule-resource-request",
    ]);
    expect(rows[2].fields).toMatchObject({
      abstractResourceTgi: "08068AE9:40800200:622B9CD7",
      recordTgi: "08068AEB:40800200:622B9CD7",
      resource: "0x2e722284",
      provenance: "created-from-record",
      path: "D:/fixture/SimCityUserData/EcoGame/SimCity-Scripts_287520926.package",
    });
  });
  it("keeps record opens, factory creation and final cache returns distinct", () => {
    const rows = analyze(
      [
        {
          channel: "resource.provenance",
          schema: 1,
          phase: "record",
          requestId: 7,
          database: { path: "candidate.package" },
          success: true,
          materialize: true,
        },
        {
          channel: "resource.provenance",
          schema: 1,
          phase: "request",
          requestId: 7,
          success: true,
          provenance: "cache-hit-origin-unknown",
          database: null,
        },
      ],
      "synthetic-provenance",
    ).rows;
    expect(rows[0].stage).toBe("rule-resource-record");
    expect(rows[1].fields.path).toBeUndefined();
    expect(rows[1].fields.provenance).toBe("cache-hit-origin-unknown");
    expect(
      captureQuality({ summary: { incompleteRequests: 1 } }, 2)
        .incompleteRequests,
    ).toBe(1);
  });
  it("links live rule IDs to abstract resource keys without claiming a package winner", () => {
    const rows = analyze(identities, "identity").rows;
    expect(rows[0].fields).toMatchObject({
      ruleId: "0x739d42ff",
      baseRuleWithinSet: 380,
      abstractResourceTgi: "08068AE9:40800200:622B9CD7",
    });
    expect(rows[1].fields.ruleId).toBe("0xf01b0eb5");
    expect(selectEvidence(rows, "simulation", "739d42ff")).toHaveLength(1);
    expect(
      selectEvidence(rows, "simulation", "08068AE9:40800200:622B9CD7"),
    ).toHaveLength(2);
    for (const row of rows) {
      expect(row.fields).not.toHaveProperty("path");
      expect(row.fields).not.toHaveProperty("ruleName");
    }
  });
  it("imports real rule entries without inventing success or unit identity", () => {
    const rows = analyze(rules, "rules").rows;
    expect(rows.map((r) => r.stage)).toEqual([
      "global-rule-log-entry",
      "unit-rule-log-entry",
    ]);
    expect(rows[0].fields.ruleIndex).toBe(380);
    expect(rows[1].fields).toMatchObject({
      unitSlot: 827,
      unitStamp: null,
      ruleIndex: 31661,
      applyCount: 3,
      maxApply: 9,
      readErrors: [],
    });
    for (const row of rows) expect(row.fields).not.toHaveProperty("success");
    const partial = analyze(
      [
        {
          ...rules[0],
          mapping: undefined,
          readErrors: ["mapping:vector_bounds"],
        },
      ],
      "partial",
    ).rows[0];
    expect(partial.fields.ruleIndex).toBeUndefined();
    expect(partial.fields.readErrors).toEqual(["mapping:vector_bounds"]);
  });
  it("keeps intentional rule filtering separate from lost records", () => {
    const quality = captureQuality(
      {
        records: 1,
        dropped: 0,
        channels: {
          unit: { hits: 22620, accepted: 1, filtered: 22619, dropped: 0 },
        },
      },
      1,
    );
    expect(quality.channels[0]).toMatchObject({ filtered: 22619, dropped: 0 });
    expect(quality.countMatches).toBe(true);
  });
  it("retains global UI events without inventing a unit or rule-success flag", () => {
    const row = analyze([globalEvent], "global").rows[0];
    expect(row.stage).toBe("global-ui-event");
    expect(row.fields.unitEventID).toBe(1935375069);
    expect(row.fields).not.toHaveProperty("unit");
    expect(row.fields).not.toHaveProperty("success");
  });
  it("shows quality counters and mismatches in both languages without treating unknown as zero", async () => {
    const w = mount(QualityPanel, {
      props: {
        status: {
          records: 2,
          dropped: 9,
          channels: { unit: { filtered: 22619 } },
        },
        records: 1,
        busy: false,
      },
      global: { plugins: [i18n] },
    });
    expect(w.text()).toContain("未知");
    expect(w.text()).toContain("定向过滤");
    expect(w.find("[role=alert]").text()).toContain("不一致");
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(w.text()).toContain("Unknown");
    expect(w.text()).toContain("Target-filtered");
    expect(w.find("[role=alert]").text()).toContain("differs");
    i18n.global.locale.value = "zh-CN";
    w.unmount();
  });
  it("distinguishes a probe from materialization and includes registration by path snapshot", () => {
    const session = analyze(resource, "resource");
    const rows = selectEvidence(
      session.rows,
      "resource",
      "00B1B104:00000000:4483E856",
    );
    expect(rows.some((r) => r.stage === "register")).toBe(true);
    expect(rows.filter((r) => r.stage === "probe")).toHaveLength(2);
    expect(
      rows.filter((r) => r.stage === "materialize" && r.fields.result === true),
    ).toHaveLength(1);
    expect(rows.find((r) => r.stage === "findRecord")?.fields.path).toContain(
      "000_openscp",
    );
  });
  it("pairs real requests by call sequence and preserves verified unit fields", () => {
    const session = analyze(ui, "ui");
    expect(session.rows[1].pair).toBe(0);
    expect(session.rows[3].pair).toBe(2);
    const unit = session.rows.find((r) => r.source === "simulation")!;
    expect(unit.fields.unit).toEqual({ slot: 76, stamp: 0 });
    expect(unit.fields.unitEventID).toBe(1769747626);
    expect(unit.fields).not.toHaveProperty("tick");
    expect(unit.fields).not.toHaveProperty("rule");
  });
  it("does not pair across bindings or separate imports; detects duplicate invocation ambiguity", () => {
    expect(
      analyze([ui[0], { ...ui[1], binding: "another" }], "x").rows[1].issue,
    ).toBe("unpaired");
    expect(analyze([ui[1]], "new").rows[0].issue).toBe("unpaired");
    expect(analyze([ui[0], ui[0], ui[1]], "x").rows[2].issue).toBe("ambiguous");
    expect(
      analyze([ui[0], ui[1], ui[1]], "x").rows.every(
        (r) => r.pair === undefined,
      ),
    ).toBe(true);
  });
  it("compares successful responses only for identical full arguments", () => {
    const call = ui[0],
      result = ui[1];
    const rows = analyze(
      [
        call,
        result,
        {
          ...call,
          sequence: 3,
          arguments: [{ type: "string", text: "different/1" }],
        },
        {
          ...result,
          sequence: 4,
          callSequence: 3,
          result: { type: "double", value: 1 },
        },
        { ...call, sequence: 5 },
        {
          ...result,
          sequence: 6,
          callSequence: 5,
          result: { type: "double", value: 2 },
        },
      ],
      "x",
    ).rows;
    expect(rows[3].changed).toBeUndefined();
    expect(rows[5].changed).toEqual(["$"]);
  });
  it("retains malformed line numbers and distinguishes engine output from host markers", async () => {
    const session = await parseTrace(
      "\uFEFF" +
        JSON.stringify(engine[0]) +
        "\nnull\nbad\n" +
        JSON.stringify({
          channel: "host.marker",
          message: "<script>bad</script>",
        }),
      "x",
    );
    expect(session.invalid).toEqual([2, 3]);
    expect(session.rows.map((r) => r.source)).toEqual(["engine", "marker"]);
    expect(session.rows[0].fields.message).toContain("Instancing");
  });
  it("imports a capture in the UI and switches all diagnostic messages live", async () => {
    const wrapper = mount(Workbench, { global: { plugins: [i18n] } });
    const input = wrapper.find("input[type=file]");
    Object.defineProperty(input.element, "files", {
      value: [
        {
          name: "test.jsonl",
          size: 100,
          text: async () =>
            [...ui, ...definitions, ...unitEffects]
              .map((r) => JSON.stringify(r))
              .join("\n"),
        },
      ],
    });
    await input.trigger("change");
    await flushPromises();
    expect(wrapper.text()).toContain("2 对调用/返回");
    expect(wrapper.text()).toContain("规则定义（非实时资源数量）");
    expect(wrapper.text()).toContain("未测量执行前后变化");
    expect(wrapper.text()).toContain("本单位资源槽");
    expect(wrapper.text()).toContain("stamp 未确定");
    await wrapper.find("input[maxlength]").setValue("<script>test</script>");
    await wrapper
      .findAll("button")
      .find((b) => b.text().includes("添加主机标记"))!
      .trigger("click");
    expect(wrapper.text()).toContain("<script>test</script>");
    expect(wrapper.find("script").exists()).toBe(false);
    i18n.global.locale.value = "en-US";
    await flushPromises();
    expect(wrapper.text()).toContain("Add host marker");
    expect(wrapper.text()).toContain("2 call/result pairs");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "All")!
      .trigger("mousedown", { button: 0, ctrlKey: false });
    await flushPromises();
    expect(wrapper.text()).toContain(
      "Rule definition (not live resource amounts)",
    );
    expect(wrapper.text()).toContain("Before/after changes were not measured.");
    expect(wrapper.text()).toContain("This unit's local bins");
    expect(wrapper.text()).toContain("Stamp is unresolved");
    i18n.global.locale.value = "zh-CN";
    wrapper.unmount();
  });
});
