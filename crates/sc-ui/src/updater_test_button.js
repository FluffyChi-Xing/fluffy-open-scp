(function (packetText) {
    "use strict";

    var doc = window.document;
    var key = "__openscpUpdaterTestButton";
    var id = "openscp-updater-ui-test";
    var labels = {
        "en-US": ["UI test", "Clicked"],
        "zh-CN": ["UI 测试", "已点击"],
        "zh-TW": ["UI 測試", "已點擊"]
    };

    function caption(state) {
        var pressed = state.clicks % 2;
        state.button.textContent = state.labels[pressed];
        state.button.setAttribute("aria-pressed", pressed ? "true" : "false");
    }

    function mount(state) {
        if (doc.getElementById(id)) {
            throw new Error("Updater test button identifier is already in use.");
        }
        var button = doc.createElement("button");
        button.type = "button";
        button.id = id;
        // The reference 1350 x 864 launcher shows the button at x=441,
        // with 8 pixels between its bottom edge and the viewport bottom.
        button.style.cssText = "position:fixed;left:32.666667%;bottom:0.925926%;"
            + "z-index:1000;box-sizing:border-box;width:132px;height:28px;"
            + "font:inherit;color:#1e5582;background:#eaf3f9;border:1px solid #7f9daf;"
            + "border-radius:4px;cursor:pointer;";
        button.onclick = function (event) {
            event.stopPropagation();
            state.clicks = (state.clicks + 1) >>> 0;
            caption(state);
        };
        doc.body.appendChild(button);
        state.button = button;
    }

    function boundsOf(button) {
        if (!button || !button.parentNode) {
            return null;
        }
        var bounds = button.getBoundingClientRect();
        var left = Math.round(bounds.left);
        var top = Math.round(bounds.top);
        var right = Math.round(bounds.right);
        var bottom = Math.round(bounds.bottom);
        return {
            left: left, top: top, right: right, bottom: bottom,
            width: right - left, height: bottom - top
        };
    }

    try {
        var packet = JSON.parse(packetText);
        var uri = String(window.location.href);
        if (uri !== packet.pageUri || !window.simcity || !window.simcity.gUpdater || !doc.body) {
            throw new Error("The active document is not the requested updater page.");
        }
        var state = doc[key] || {
            generation: null, button: null, clicks: 0
        };
        if (packet.action !== "read") {
            if (state.generation !== null && packet.generation <= state.generation) {
                throw new Error("Updater test button generation was already applied or is stale.");
            }
            if (packet.action === "apply") {
                if (!Object.prototype.hasOwnProperty.call(labels, packet.locale)) {
                    throw new Error("Unsupported updater test button locale.");
                }
                if (!state.button || !state.button.parentNode) {
                    mount(state);
                }
                state.labels = labels[packet.locale];
                caption(state);
            } else if (packet.action === "clear") {
                if (state.button && state.button.parentNode) {
                    state.button.parentNode.removeChild(state.button);
                }
                state.button = null;
            } else {
                throw new Error("Unknown updater test button action.");
            }
            state.generation = packet.generation;
            doc[key] = state;
        }
        return JSON.stringify({ status: "success", result: {
            pageUri: uri,
            generation: state.generation,
            mounted: !!(state.button && state.button.parentNode),
            clicks: state.clicks,
            rect: boundsOf(state.button)
        } });
    } catch (error) {
        return JSON.stringify({ status: "failure", error: String(error.message || error) });
    }
})
