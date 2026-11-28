const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

test("keeps a fixed viewport position, observes clicks, reloads and clears", () => {
    const buttons = [];
    const document = {
        getElementById: id => buttons.find(button => button.id === id),
        createElement: tag => {
            assert.equal(tag, "button");
            return {
                style: {},
                setAttribute(key, value) { this[key] = value; },
                getBoundingClientRect() {
                    const length = name => Number(this.style.cssText.match(new RegExp(`(?:^|;)${name}:([\\d.]+)`))[1]);
                    const left = window.innerWidth * length("left") / 100;
                    const bottom = window.innerHeight * (1 - length("bottom") / 100);
                    return { left, top: bottom - length("height"), right: left + length("width"), bottom };
                }
            };
        },
        body: {
            appendChild(button) { buttons.push(button); button.parentNode = this; },
            removeChild(button) { buttons.splice(buttons.indexOf(button), 1); button.parentNode = null; }
        }
    };
    const pageUri = "game://localhost/resource/0x40464200!0xcb74143c.0xdd6233d6";
    const window = {
        document, location: { href: pageUri }, innerWidth: 900, innerHeight: 576,
        simcity: { gUpdater: {} }
    };
    const source = fs.readFileSync(path.join(__dirname, "../src/updater_test_button.js"), "utf8");
    const probe = vm.runInNewContext(source, { window });
    const run = (action, params = {}) => JSON.parse(probe(JSON.stringify({ action, pageUri, ...params })));
    const apply = (generation, locale) => run("apply", { generation, locale });

    const mounted = apply(1, "zh-TW").result;
    assert.deepEqual(mounted.rect, { left: 294, top: 543, right: 426, bottom: 571, width: 132, height: 28 });
    assert.match(buttons[0].style.cssText, /^position:fixed;/);
    assert.equal(buttons[0].textContent, "UI 測試");
    let stopped = false;
    buttons[0].onclick({ stopPropagation() { stopped = true; } });
    assert.equal(stopped, true);
    assert.equal(run("read").result.clicks, 1);
    assert.equal(buttons[0].textContent, "已點擊");
    window.simcity.gUpdater.mControlIds = { VIEW_GAME_UPDATES_BUTTON: 242330041 };
    window.gUIManager = { FindControlByID() { assert.fail("Fixed button must not look up controls"); } };

    assert.equal(apply(2, "en-US").result.generation, 2);
    assert.equal(buttons.length, 1);
    assert.equal(buttons[0].textContent, "Clicked");
    assert.equal(apply(1, "zh-CN").status, "failure");
    assert.equal(run("clear", { generation: 3, pageUri: "game://other" }).status, "failure");
    assert.equal(apply(3, "invalid").status, "failure");
    assert.equal(run("read").result.generation, 2);

    assert.deepEqual(run("read").result.rect, mounted.rect);
    window.innerWidth = 1200;
    assert.equal(run("read").result.rect.left, 392);

    const cleared = run("clear", { generation: 3 }).result;
    assert.equal(cleared.mounted, false);
    assert.equal(cleared.rect, null);
    assert.equal(buttons.length, 0);
});
