(function (packetText) {
    "use strict";
    try {
        var p = JSON.parse(packetText);
        if (String(window.location.href) !== p.pageUri) {
            throw new Error("The active page changed.");
        }
        if (typeof p.x !== "number" || typeof p.y !== "number" ||
                !isFinite(p.x) || !isFinite(p.y) || p.x < 0 || p.y < 0 ||
                p.x >= window.innerWidth || p.y >= window.innerHeight) {
            throw new Error("Point is outside the active viewport.");
        }
        var el = window.document.elementFromPoint(p.x, p.y);
        var path = [];
        while (el && el.nodeType === 1 && path.length < 32) {
            var box = el.getBoundingClientRect();
            var style = window.getComputedStyle(el, null);
            path.push({
                tag: String(el.tagName).toLowerCase(),
                id: String(el.getAttribute("id") || "").slice(0, 256),
                classes: String(el.getAttribute("class") || "").slice(0, 512),
                display: style.display, position: style.position,
                bounds: { left: box.left, top: box.top, right: box.right, bottom: box.bottom }
            });
            el = el.parentNode;
        }
        return JSON.stringify({status: "success", result: {
            pageUri: p.pageUri, path: path, truncated: !!el && el.nodeType === 1
        }});
    } catch (error) {
        return JSON.stringify({status: "failure", error: String(error.message || error)});
    }
})
