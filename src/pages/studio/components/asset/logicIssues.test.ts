import { describe, expect, it } from "vitest";
import { logicIssues, normalizeLogicGraph } from "./logicGraph";

/** 回归（2026-11-07）：文档里 education 叶子的可选参数为 null 时
 * （boardInstance/presentationUnit 等），logicIssues 不得抛
 * "reading 'trim'"——null 一律按"未填"处理为校验问题而非崩溃。
 * 实机案例：大学项目 logic 图写入 null 值参数后画布交互全瘫。 */
describe("logicIssues null-safety", () => {
  it("education leaf with null optional params reports issues instead of throwing", () => {
    const graph = normalizeLogicGraph({
      nodes: [
        { id: "info-1", kind: "building-info", position: [0, 0] },
        { id: "core-1", kind: "behavior-core", position: [200, 0] },
        { id: "leaf-1", kind: "behavior-leaf", position: [0, 120] },
      ],
      edges: [
        { id: "e1", source: "info-1", target: "asset", targetHandle: "info" },
        { id: "e2", source: "core-1", target: "asset", targetHandle: "behavior" },
        { id: "e3", source: "leaf-1", target: "core-1", targetHandle: "leaf" },
      ],
      params: {
        "info-1": { w1: 20, w2: 10, w3: 4, minWorkers: 12, schedule: [6, 6, 7, 7, 6, 6, 7, 6] },
        "core-1": {
          power: { mode: "inherit", rate: null, amount: null },
          water: { mode: "inherit", rate: null, amount: null },
          garbage: { mode: "inherit", amount: null },
        },
        "leaf-1": {
          type: "education",
          capacity: null,
          boardInstance: null,
          presentationUnit: null,
          signatureBundle: "0x6C042366",
          signatureBundles: "0x487CDA43,0xBA823893",
          removeBundles: "0x6C042367,0x29D4B82E,0x95573B5D",
          titleOverride: "t",
          walkPath: false,
          servicePath: false,
          pedestrianPath: false,
        },
      },
    });
    expect(graph.nodes).toHaveLength(3);
    expect(() => logicIssues(graph)).not.toThrow();
    const codes = logicIssues(graph).map((issue) => issue.code);
    expect(codes).toContain("educationCapacity");
    expect(codes).toContain("leafHashInvalid");
  });
});
