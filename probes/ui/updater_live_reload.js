(function (token) {
    "use strict";

    var base = "http://127.0.0.1:47837/" + token + "/";
    var page = String(new Date().getTime()) + Math.random().toString(36).slice(2, 12);
    var sequence = 0;
    var current = null;
    var receipt = null;
    var sceneId = 0x18ab51f2;

    function sceneElement() {
        var manager = window.gUIManager || (window.scrui && window.scrui.gUIManager);
        var scene = manager && typeof manager.FindControlByID === "function"
            ? manager.FindControlByID(sceneId) : null;
        return scene && scene.mRootElement;
    }

    function request() {
        var root = sceneElement();
        if (!root || !window.document.body.contains(root)) {
            window.setTimeout(request, 1000);
            return;
        }
        var ticket = page + "/" + (++sequence);
        var candidate = new Image();
        function next() {
            candidate.onload = candidate.onerror = null;
            window.clearTimeout(timeout);
            window.setTimeout(request, 1000);
        }
        var timeout = window.setTimeout(next, 5000);
        candidate.onerror = next;
        candidate.onload = function () {
            if (candidate.width > 1 && candidate.height > 1 && sceneElement() === root) {
                candidate.style.cssText = "position:absolute;left:0;top:0;width:100%;height:100%;pointer-events:none;";
                root.appendChild(candidate);
                if (current && current.parentNode) {
                    current.parentNode.removeChild(current);
                }
                current = candidate;
                receipt = new Image();
                receipt.src = base + "applied/" + ticket;
            }
            next();
        };
        candidate.src = base + "background/" + ticket;
    }

    window.addEventListener("load", function () {
        window.setTimeout(request, 0);
    }, false);
})
