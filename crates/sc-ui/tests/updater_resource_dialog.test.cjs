const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

test("dialog previews the packaged and live images, updates locale and restores focus", async () => {
    const { Window } = await import("happy-dom");
    const window = new Window();
    try {
        const doc = window.document;
        const locale = { mLocaleCode: "en-US" };
        window.gUIManager = { mLocaleManager: locale };
        const draws = [];
        window.HTMLCanvasElement.prototype.getContext = () => ({
            drawImage: image => draws.push(image)
        });
        const script = fs.readFileSync(path.join(__dirname, "../../../probes/ui/updater_resource_dialog.js"), "utf8");
        const dialog = vm.runInNewContext(script + "()", { window });
        const trigger = doc.querySelector("button");
        trigger.click();
        const panel = doc.querySelector('[role="dialog"]');
        assert.equal(panel.parentNode.style.display, "block");
        assert.match(panel.querySelector("img").src, /OpenSCP-Launcher-Background.png$/);
        const first = { width: 40, height: 32 };
        dialog.update(first);
        assert.equal(draws[0], first);
        assert.equal(panel.querySelector("canvas").width, 40);
        locale.mLocaleCode = "zh-CN";
        dialog.language();
        assert.equal(panel.getAttribute("aria-label"), "启动器背景");
        assert.equal(trigger.textContent, "资源");
        doc.dispatchEvent(new window.KeyboardEvent("keydown", { keyCode: 27 }));
        assert.equal(doc.activeElement, trigger);
        assert.equal(panel.parentNode.style.display, "none");
        const second = { width: 60, height: 40 };
        dialog.update(second);
        assert.equal(draws.length, 1);
        trigger.click();
        assert.equal(draws[1], second);
        panel.querySelector("button").click();
        assert.equal(panel.parentNode.style.display, "none");
    } finally {
        await window.happyDOM.abort();
        window.close();
    }
});
