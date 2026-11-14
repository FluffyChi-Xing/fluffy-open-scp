import { describe, expect, it } from "vitest";
import { resolvePressTarget } from "./press";

function elementFrom(html: string): HTMLElement {
  const host = document.createElement("div");
  host.innerHTML = html;
  document.body.appendChild(host);
  return host;
}

describe("resolvePressTarget", () => {
  it("accepts buttons, role=button and opted-in elements", () => {
    const host = elementFrom(
      `<button id="plain"><span id="inner">x</span></button>
       <div id="role" role="button">y</div>
       <div id="optin" data-press>z</div>`,
    );
    expect(resolvePressTarget(host.querySelector("#inner"))?.id).toBe("plain");
    expect(resolvePressTarget(host.querySelector("#role"))?.id).toBe("role");
    expect(resolvePressTarget(host.querySelector("#optin"))?.id).toBe("optin");
    host.remove();
  });

  it("rejects disabled, aria-disabled, opted-out and non-interactive targets", () => {
    const host = elementFrom(
      `<button id="disabled" disabled>x</button>
       <button id="aria" aria-disabled="true">x</button>
       <button id="off" data-press="off">x</button>
       <div id="plain">x</div>`,
    );
    for (const id of ["disabled", "aria", "off", "plain"]) {
      expect(resolvePressTarget(host.querySelector(`#${id}`))).toBeNull();
    }
    expect(resolvePressTarget(null)).toBeNull();
    host.remove();
  });
});
