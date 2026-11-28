const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

test("inspects the hit DOM ancestry and rejects stale pages and viewport margins", async () => {
    const { Window } = await import("happy-dom");
    const window = new Window({ url: "https://game.invalid/updater" });
    try {
        const doc = window.document;
        doc.body.innerHTML = '<button id="play"><span class="caption"></span></button>';
        let hit = doc.querySelector("span");
        doc.elementFromPoint = () => hit;
        const source = fs.readFileSync(path.join(__dirname, "../src/inspect_dom_point.js"), "utf8");
        const probe = vm.runInNewContext(source, { window });
        const read = (x, pageUri = window.location.href) => JSON.parse(probe(JSON.stringify({x, y: 1, pageUri})));
        const result = read(1).result;
        assert.deepEqual(result.path.map(n => n.tag), ["span", "button", "body", "html"]);
        assert.equal(result.path[0].classes, "caption");
        assert.equal(result.path[1].id, "play");
        assert.equal(result.truncated, false);
        assert.equal(read(window.innerWidth).status, "failure");
        assert.equal(read(1, "stale").status, "failure");
        for (let i = 0; i < 35; i++) { hit = hit.appendChild(doc.createElement("div")); }
        assert.equal(read(1).result.path.length, 32);
        assert.equal(read(1).result.truncated, true);
        hit = null;
        assert.deepEqual(read(1).result.path, []);
    } finally { window.close(); }
});
