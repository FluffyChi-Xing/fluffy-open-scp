/** @summary Benign rollover formatting retains DOM appearance while hostile markup cannot become active. */
import { describe, expect, it } from "vitest";
import { formatPreviewRichText } from "./rich-text";

/** @summary Attach a fixture for computed-style comparisons.
 * @param html Fixture markup.
 * @returns Attached fixture root; callers remove it after assertions.
 */
function fixture(html: string): HTMLDivElement {
  const root = document.createElement("div");
  root.innerHTML = html;
  document.body.appendChild(root);
  return root;
}

describe("passive preview rich text", () => {
  it("retains empty nullable statistics and numeric text semantics", () => {
    expect(formatPreviewRichText(null)).toBe("");
    expect(formatPreviewRichText(undefined)).toBe("");
    expect(formatPreviewRichText(123)).toBe("123");
  });
  it("preserves legacy bold, italic, line breaks, span colors and whitespace", () => {
    const original = '<b>Road</b><br><i>Fast</i> <span class="amount" title="Budget" style="color:#12ab34;font-weight:700;font-style:italic;text-decoration:underline;white-space:pre-wrap">  § 5\n</span>';
    const baseline = fixture(original), safe = fixture(formatPreviewRichText(original));
    expect(safe.textContent).toBe(baseline.textContent);
    expect([...safe.children].map(n => n.tagName)).toEqual([...baseline.children].map(n => n.tagName));
    for (const selector of ["b", "i", "span"]) {
      const left = getComputedStyle(baseline.querySelector(selector)!);
      const right = getComputedStyle(safe.querySelector(selector)!);
      for (const key of ["color", "fontWeight", "fontStyle", "textDecoration", "whiteSpace"] as const) expect(right[key]).toBe(left[key]);
    }
    expect(safe.querySelector("span")?.className).toBe("amount");
    expect(safe.querySelector("span")?.title).toBe("Budget");
    baseline.remove(); safe.remove();
  });

  it("retains passive lists, paragraphs, headings, table cells and legacy font color", () => {
    const safe = fixture(formatPreviewRichText('<h3>Info</h3><p><strong>A</strong><em>B</em><u>C</u><s>D</s></p><ul><li>Item</li></ul><table><tbody><tr><td>Cell</td></tr></tbody></table><font color="red">Cost</font>'));
    expect(safe.querySelectorAll("h3,p,strong,em,u,s,ul,li,table,tbody,tr,td,font")).toHaveLength(13);
    expect(getComputedStyle(safe.querySelector("font")!).color).toBe("red");
    expect(safe.textContent).toBe("InfoABCDItemCellCost");
    safe.remove();
  });

  it("flattens unknown and link wrappers while preserving inert text and inner formatting", () => {
    const safe = fixture(formatPreviewRichText('<custom-label><a href="javascript:alert(1)">Keep <b>this</b></a></custom-label>'));
    expect(safe.innerHTML).toBe("Keep <b>this</b>");
    safe.remove();
  });

  it("drops active containers, resource elements and their content", () => {
    const safe = fixture(formatPreviewRichText('Safe<script>window.bad=1</script><style>body{display:none}</style><svg onload="alert(1)"><text>bad</text></svg><math><mi>bad</mi></math><iframe src="https://evil.invalid">bad</iframe><object data="https://evil.invalid">bad</object><form><button>bad</button></form><img src=x onerror="alert(1)"><template>bad</template> End'));
    expect(safe.textContent).toBe("Safe End");
    expect(safe.querySelectorAll("*")).toHaveLength(0);
    safe.remove();
  });

  it("never carries events, URLs, arbitrary attributes or resource-bearing CSS", () => {
    const safe = fixture(formatPreviewRichText('<span id="target" onclick="alert(1)" data-x="bad" style="background-image:url(https://evil.invalid/x);position:fixed;color:rgb(1,2,3);font-style:italic" title="&quot; onmouseover=bad" class="safe-token">Text</span>'));
    const span = safe.querySelector("span")!;
    expect(span.getAttributeNames().sort()).toEqual(["class", "style", "title"]);
    expect(span.style.backgroundImage).toBe("");
    expect(span.style.position).toBe("");
    expect(span.style.color).toBe("rgb(1, 2, 3)");
    expect(span.style.fontStyle).toBe("italic");
    expect(span.getAttribute("onmouseover")).toBeNull();
    expect(safe.innerHTML).not.toMatch(/https:|onclick|data-x|id=/);
    safe.remove();
  });

  it("rejects expression, variable and custom-function styling without losing plain text", () => {
    const safe = fixture(formatPreviewRichText('<span style="color:var(--unsafe);font-weight:calc(100 + 600);white-space:inherit;text-decoration:url(x)">Budget &amp; roads</span>'));
    expect(safe.textContent).toBe("Budget & roads");
    expect(safe.querySelector("span")?.getAttribute("style")).toBeNull();
    safe.remove();
  });
});
