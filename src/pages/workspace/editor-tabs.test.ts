import { describe, expect, it } from "vitest";
import {
  type TabsState,
  activeTab,
  closeTab,
  findTab,
  openTab,
  parseTabs,
  rebaseTabs,
  serializeTabs,
  setTabMode,
  tabFailed,
  tabLoaded,
  tabSaved,
  tabTitle,
  updateDraft,
} from "./editor-tabs";

const doc = (relativePath: string, content = "body") => ({
  relativePath,
  content,
  size: content.length,
  revision: `rev-${content.length}`,
});

function opened(paths: string[]): TabsState {
  let state: TabsState = { tabs: [], activePath: "" };
  for (const path of paths) state = openTab(state, path);
  return state;
}

describe("openTab", () => {
  it("appends a loading tab and activates it", () => {
    const state = openTab({ tabs: [], activePath: "" }, "mods/README.md");
    expect(state.tabs).toHaveLength(1);
    expect(state.tabs[0]).toMatchObject({
      path: "mods/README.md",
      loading: true,
      mode: "edit",
      dirty: false,
    });
    expect(state.activePath).toBe("mods/README.md");
  });

  it("activates without duplicating an open tab", () => {
    let state = opened(["a.md", "b.md"]);
    state = openTab(state, "a.md");
    expect(state.tabs.map((tab) => tab.path)).toEqual(["a.md", "b.md"]);
    expect(state.activePath).toBe("a.md");
  });
});

describe("tabLoaded / tabFailed / tabSaved", () => {
  it("fills content and clears loading", () => {
    const state = tabLoaded(opened(["a.md"]), doc("a.md", "# hi"));
    expect(state.tabs[0]).toMatchObject({
      draft: "# hi",
      revision: "rev-4",
      loading: false,
      dirty: false,
    });
  });

  it("keeps the tab and records the error on failure", () => {
    const state = tabFailed(opened(["a.md"]), "a.md", "boom");
    expect(state.tabs[0]).toMatchObject({ loading: false, error: "boom" });
  });

  it("ignores documents for closed tabs", () => {
    const state = tabLoaded(opened(["a.md"]), doc("gone.md"));
    expect(state).toEqual(opened(["a.md"]));
  });

  it("tabSaved syncs revision and clears dirty", () => {
    let state = tabLoaded(opened(["a.md"]), doc("a.md", "old"));
    state = updateDraft(state, "a.md", "new text");
    expect(state.tabs[0].dirty).toBe(true);
    const saved = tabSaved(state, doc("a.md", "new text"));
    expect(saved.tabs[0]).toMatchObject({ dirty: false, revision: "rev-8" });
  });
});

describe("closeTab", () => {
  it("activates the right neighbor, else the left one", () => {
    let state = { ...opened(["a.md", "b.md", "c.md"]), activePath: "a.md" };
    state = closeTab(state, "a.md")!;
    expect(state.activePath).toBe("b.md");
    state = { ...state, activePath: "c.md" };
    state = closeTab(state, "c.md")!;
    expect(state.activePath).toBe("b.md");
  });

  it("clears the active path when the last tab closes", () => {
    const state = closeTab(opened(["a.md"]), "a.md")!;
    expect(state.tabs).toEqual([]);
    expect(state.activePath).toBe("");
  });

  it("returns null for unknown paths", () => {
    expect(closeTab(opened(["a.md"]), "x.md")).toBeNull();
  });

  it("keeps active path when closing a background tab", () => {
    let state = opened(["a.md", "b.md"]);
    state = { ...state, activePath: "b.md" };
    expect(closeTab(state, "a.md")!.activePath).toBe("b.md");
  });
});

describe("updateDraft / setTabMode / activeTab / findTab", () => {
  it("marks dirty and skips identical drafts", () => {
    let state = tabLoaded(opened(["a.md"]), doc("a.md"));
    state = updateDraft(state, "a.md", "changed");
    expect(state.tabs[0]).toMatchObject({ draft: "changed", dirty: true });
    expect(updateDraft(state, "a.md", "changed")).toBe(state);
  });

  it("toggles preview mode", () => {
    const state = setTabMode(opened(["a.md"]), "a.md", "preview");
    expect(state.tabs[0].mode).toBe("preview");
  });

  it("resolves the active tab and misses unknown paths", () => {
    const state = opened(["a.md", "b.md"]);
    expect(activeTab(state)?.path).toBe("b.md");
    expect(findTab(state.tabs, "x.md")).toBeUndefined();
    expect(activeTab({ ...state, activePath: "x.md" })).toBeNull();
  });
});

describe("rebaseTabs", () => {
  it("renames an exact path and rebases folder children", () => {
    let state = opened(["mods/a.md", "mods/sub/b.md", "assets/c.md"]);
    state = { ...state, activePath: "mods/sub/b.md" };
    expect(rebaseTabs(state, "mods", "docs").tabs.map((t) => t.path)).toEqual([
      "docs/a.md",
      "docs/sub/b.md",
      "assets/c.md",
    ]);
  });

  it("rebases the active path", () => {
    const state = { ...opened(["a.md"]), activePath: "a.md" };
    expect(rebaseTabs(state, "a.md", "b.md").activePath).toBe("b.md");
  });

  it("returns the same state when nothing matches", () => {
    const state = opened(["a.md"]);
    expect(rebaseTabs(state, "zzz", "yyy")).toBe(state);
  });
});

describe("persistence", () => {
  it("round-trips paths and active tab", () => {
    const state = opened(["a.md", "b.md"]);
    const parsed = parseTabs(serializeTabs(state))!;
    expect(parsed).toEqual({ paths: ["a.md", "b.md"], activePath: "b.md" });
  });

  it("falls back to the first path for a stale active tab", () => {
    const parsed = parseTabs(
      JSON.stringify({ paths: ["a.md"], activePath: "gone.md" }),
    )!;
    expect(parsed.activePath).toBe("a.md");
  });

  it("rejects malformed payloads", () => {
    expect(parseTabs(null)).toBeNull();
    expect(parseTabs("")).toBeNull();
    expect(parseTabs("{")).toBeNull();
    expect(parseTabs(JSON.stringify({ paths: [42] }))!.paths).toEqual([]);
    expect(parseTabs(JSON.stringify({}))).toBeNull();
  });
});

describe("tabTitle", () => {
  it("returns the basename", () => {
    expect(tabTitle("mods/example/README.md")).toBe("README.md");
    expect(tabTitle("README.md")).toBe("README.md");
  });
});
