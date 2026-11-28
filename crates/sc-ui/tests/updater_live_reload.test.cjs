const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

const script = fs.readFileSync(path.join(__dirname, "../../../probes/ui/updater_live_reload.js"), "utf8");

test("restores the acknowledged image on a rebuilt scene and ignores detached deliveries", () => {
    const images = [];
    const timers = new Map();
    let id = 0;
    let load;
    const makeRoot = () => ({
        appendChild(image) { image.parentNode = this; },
        removeChild(image) { image.parentNode = null; },
    });
    let root = makeRoot();
    let attached = true;
    const window = {
        gUIManager: { FindControlByID: () => ({ mRootElement: root }) },
        document: { body: { contains: element => attached && element === root } },
        addEventListener: (_, callback) => { load = callback; },
        setTimeout: (callback, delay) => { timers.set(++id, { callback, delay }); return id; },
        clearTimeout: key => timers.delete(key),
    };
    function tick(delay) {
        const [key, timer] = [...timers].find(([, timer]) => timer.delay === delay);
        timers.delete(key);
        timer.callback();
    }
    function Image() {
        images.push(this);
        this.style = {};
        this.width = this.height = 32;
    }
    vm.runInNewContext(script + '("preview");', { window, Image });
    load();
    tick(0);
    images[0].onload();
    assert.match(images[1].src, /\/applied\//);
    root = makeRoot();
    tick(1000);
    assert.equal(images[0].parentNode, root);
    attached = false;
    images[2].onload();
    assert.equal(images.length, 3);
    assert.equal(images[2].parentNode, undefined);
    attached = true;
    tick(1000);
    images[3].onload();
    assert.equal(images[0].parentNode, null);
    assert.equal(images[3].parentNode, root);
    assert.match(images[4].src, /\/applied\//);
});
