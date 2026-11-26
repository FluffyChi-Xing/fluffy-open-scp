// Adapted from Simoder-MCP 591eeabb, host/session_router.js, inspect-ui-tree.
// MIT license, see LICENSE-Simoder-MCP.
(function (packetText) {
    "use strict";
    try {
        var packet = JSON.parse(packetText);
        var uiMgr = window.gUIManager || (window.scrui && window.scrui.gUIManager);
        if (!uiMgr) {
            throw new Error("scrui.gUIManager is not ready.");
        }
        var rootCtrl = null;
        if (packet.controlId !== undefined && packet.controlId !== null) {
            if (typeof uiMgr.FindControlByID !== "function") {
                throw new Error("gUIManager.FindControlByID is unavailable.");
            }
            rootCtrl = uiMgr.FindControlByID(Number(packet.controlId));
            if (!rootCtrl) {
                throw new Error("Control ID " + String(packet.controlId) + " was not found.");
            }
        } else if (typeof uiMgr.GetRootWindow === "function") {
            rootCtrl = uiMgr.GetRootWindow();
        } else if (typeof uiMgr.GetRootLayout === "function") {
            rootCtrl = uiMgr.GetRootLayout();
        }
        if (!rootCtrl) {
            throw new Error("No root UI control is available.");
        }
        var maxDepth = Math.max(0, Math.min(8, Number(packet.maxDepth !== undefined ? packet.maxDepth : 3)));
        var visibleOnly = packet.visibleOnly !== false;
        var maxNodes = Math.max(1, Math.min(300, Number(packet.maxNodes !== undefined ? packet.maxNodes : 120)));
        var uiNodes = [];
        var treeTruncated = false;
        var depthLimited = false;
        var boxRect = function (el) {
            if (!el || typeof el.getBoundingClientRect !== "function") { return null; }
            var b = el.getBoundingClientRect();
            return {
                left: Math.round(b.left),
                top: Math.round(b.top),
                width: Math.round(b.width),
                height: Math.round(b.height),
                right: Math.round(b.right),
                bottom: Math.round(b.bottom)
            };
        };
        var isVisible = function (ctrl) {
            return typeof ctrl.GetAbsoluteVisibility === "function" ? !!ctrl.GetAbsoluteVisibility() : (ctrl.mIsVisible !== false);
        };
        var walkCtrl = function (ctrl, depth) {
            if (!ctrl) { return; }
            var isVis = isVisible(ctrl);
            if (visibleOnly && !isVis) {
                return;
            }
            if (uiNodes.length >= maxNodes) {
                treeTruncated = true;
                return;
            }
            var absPos = null;
            if (typeof ctrl.GetAbsolutePosition === "function") {
                try { absPos = ctrl.GetAbsolutePosition(); } catch (_e) {}
            }
            var txt = "";
            if (ctrl.mTextElement && ctrl.mTextElement.textContent) {
                txt = String(ctrl.mTextElement.textContent).replace(/\s+/g, " ").replace(/^\s+|\s+$/g, "").slice(0, 120);
            } else if (ctrl.mRootElement && ctrl.mRootElement.textContent) {
                txt = String(ctrl.mRootElement.textContent).replace(/\s+/g, " ").replace(/^\s+|\s+$/g, "").slice(0, 120);
            }
            var styleCopy = null;
            if (ctrl.mStyleList && ctrl.mStyleList.length) {
                styleCopy = [];
                for (var si = 0; si < ctrl.mStyleList.length && si < 8; si++) {
                    styleCopy.push(ctrl.mStyleList[si]);
                }
            }
            uiNodes.push({
                depth: depth,
                controlId: ctrl.mControlID ? ctrl.mControlID >>> 0 : null,
                parentId: ctrl.mParentControl && ctrl.mParentControl.mControlID ? ctrl.mParentControl.mControlID >>> 0 : null,
                visible: isVis,
                left: ctrl.mLeft !== undefined ? ctrl.mLeft : null,
                top: ctrl.mTop !== undefined ? ctrl.mTop : null,
                width: ctrl.mWidth !== undefined ? ctrl.mWidth : null,
                height: ctrl.mHeight !== undefined ? ctrl.mHeight : null,
                absX: absPos && absPos.length >= 2 ? absPos[0] : null,
                absY: absPos && absPos.length >= 2 ? absPos[1] : null,
                hPin: ctrl.mHorizontalPinType !== undefined ? ctrl.mHorizontalPinType : null,
                vPin: ctrl.mVerticalPinType !== undefined ? ctrl.mVerticalPinType : null,
                drawableType: ctrl.mDrawableType !== undefined ? ctrl.mDrawableType : null,
                styleList: styleCopy,
                blurBackground: !!ctrl.mBlurBackground,
                className: ctrl.mRootElement ? String(ctrl.mRootElement.className || "") : "",
                overflow: (ctrl.mRootElement && ctrl.mRootElement.style) ? String(ctrl.mRootElement.style.overflow || "") : "",
                rect: boxRect(ctrl.mRootElement),
                text: txt,
                childCount: ctrl.mChildren ? ctrl.mChildren.length : 0
            });
            if (ctrl.mChildren && ctrl.mChildren.length) {
                for (var ci = 0; ci < ctrl.mChildren.length; ci++) {
                    var child = ctrl.mChildren[ci];
                    if (!child) { continue; }
                    if (depth === maxDepth) {
                        if (!visibleOnly || isVisible(child)) {
                            depthLimited = true;
                            break;
                        }
                        continue;
                    }
                    walkCtrl(child, depth + 1);
                    if (treeTruncated) { break; }
                }
            }
        };
        walkCtrl(rootCtrl, 0);
        var result = {
            viewport: {
                width: window.innerWidth || 0,
                height: window.innerHeight || 0
            },
            rootControlId: rootCtrl.mControlID ? rootCtrl.mControlID >>> 0 : null,
            truncated: treeTruncated,
            depthLimited: depthLimited,
            nodes: uiNodes
        };
        return JSON.stringify({ status: "success", result: result });
    } catch (error) {
        return JSON.stringify({ status: "failure", error: String(error.message || error) });
    }
})
