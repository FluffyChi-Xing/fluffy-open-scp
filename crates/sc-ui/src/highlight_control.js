(function (packetText) {
    "use strict";

    var id = "openscp-scrui-highlight";
    var owner = "data-openscp-scrui-highlight";
    var generationKey = "__openscpScruiHighlightGeneration";
    var doc = window.document;
    try {
        var packet = JSON.parse(packetText);
        var sync = packet.action === "sync";
        var result = { rect: null };
        if (sync) {
            result.pageUri = String(window.location.href);
            result.generation = packet.generation;
            if (result.pageUri !== packet.pageUri) {
                throw new Error("The active page changed.");
            }
            if (typeof doc[generationKey] === "number" && packet.generation <= doc[generationKey]) {
                throw new Error("Highlight generation was already applied or is stale.");
            }
        }
        var previous = doc.getElementById(id);
        if (previous && previous.getAttribute(owner) !== "1") {
            throw new Error("Highlight identifier is already in use.");
        }
        if (packet.action === "clear" || (sync && packet.controlId === null)) {
            if (previous && previous.parentNode) {
                previous.parentNode.removeChild(previous);
            }
            if (sync) { doc[generationKey] = packet.generation; }
            return JSON.stringify({ status: "success", result: result });
        }
        if (!sync && packet.action !== "show") {
            throw new Error("Unknown highlight action.");
        }

        var manager = window.gUIManager || (window.scrui && window.scrui.gUIManager);
        var control = manager && typeof manager.FindControlByID === "function"
            ? manager.FindControlByID(packet.controlId)
            : null;
        var element = control && control.mRootElement;
        if (!element || !doc.body || element.ownerDocument !== doc || !doc.body.contains(element)) {
            throw new Error("Control is unavailable in the active document.");
        }
        var visible = typeof control.GetAbsoluteVisibility === "function"
            ? control.GetAbsoluteVisibility()
            : control.mIsVisible !== false;
        if (!visible) {
            throw new Error("Control is hidden.");
        }

        var bounds = element.getBoundingClientRect();
        var rect = {
            left: Math.max(0, Math.round(bounds.left)),
            top: Math.max(0, Math.round(bounds.top)),
            right: Math.min(window.innerWidth, Math.round(bounds.right)),
            bottom: Math.min(window.innerHeight, Math.round(bounds.bottom))
        };
        rect.width = rect.right - rect.left;
        rect.height = rect.bottom - rect.top;
        if (!isFinite(rect.width) || !isFinite(rect.height) || rect.width <= 0 || rect.height <= 0) {
            throw new Error("Control has no visible viewport bounds.");
        }

        var overlay = doc.createElement("div");
        overlay.id = id;
        overlay.setAttribute(owner, "1");
        overlay.setAttribute("aria-hidden", "true");
        var color = sync && packet.tone === "amber" ? "#ffb000" : "#00e5ff";
        overlay.style.cssText = "position:fixed;pointer-events:none;z-index:2147483647;"
            + "margin:0;padding:0;box-sizing:border-box;border:2px solid " + color + ";"
            + "background:transparent;left:" + rect.left + "px;top:" + rect.top + "px;"
            + "width:" + rect.width + "px;height:" + rect.height + "px;";
        if (overlay.style.getPropertyValue("pointer-events") !== "none") {
            throw new Error("The active view does not support input-transparent highlights.");
        }
        doc.body.appendChild(overlay);
        if (window.getComputedStyle(overlay, null).getPropertyValue("pointer-events") !== "none") {
            overlay.parentNode.removeChild(overlay);
            throw new Error("The active page overrode highlight input transparency.");
        }
        window.setTimeout(function () {
            if (overlay.parentNode) {
                overlay.parentNode.removeChild(overlay);
            }
        }, 1200);
        if (previous && previous.parentNode) {
            previous.parentNode.removeChild(previous);
        }
        if (sync) { doc[generationKey] = packet.generation; }
        result.rect = rect;
        return JSON.stringify({ status: "success", result: result });
    } catch (error) {
        if (overlay && overlay.parentNode) {
            overlay.parentNode.removeChild(overlay);
        }
        return JSON.stringify({ status: "failure", error: String(error.message || error) });
    }
})
