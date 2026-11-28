(function () {
    "use strict";
    var doc = window.document;
    var messages = {
        "en-us": ["Resources", "Launcher background", "Close"],
        "zh-cn": ["资源", "启动器背景", "关闭"],
        "zh-tw": ["資源", "啟動器背景", "關閉"]
    };
    function element(tag, parent, css) {
        var node = doc.createElement(tag);
        node.style.cssText = css;
        parent.appendChild(node);
        return node;
    }
    var trigger = element("button", doc.body, "position:fixed;left:12px;top:12px;z-index:1001;font:inherit;cursor:pointer;");
    var shade = element("div", doc.body, "display:none;position:fixed;top:0;left:0;bottom:0;right:0;background:rgba(14,24,36,.68);z-index:2147483647;");
    var panel = element("div", shade, "position:absolute;top:12%;left:12%;width:76%;height:76%;overflow:hidden;background:#e9edf1;border-radius:10px;color:#234f71;box-shadow:0 5px 24px #283543;font:inherit;");
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-modal", "true");
    var title = element("h2", panel, "margin:0;padding:10px 42px 10px 16px;font:inherit;font-weight:bold;");
    var close = element("button", panel, "position:absolute;right:8px;top:6px;width:30px;height:30px;cursor:pointer;border:0;background:transparent url('Graphics/Common/Buttons/btn_FE_Close_normal.png') center/contain no-repeat;");
    close.textContent = "×";
    var frame = element("div", panel, "position:absolute;top:46px;left:12px;right:12px;bottom:12px;background:#283543;");
    var initial = "Graphics/Launcher/OpenSCP-Launcher-Background.png";
    function showImage(image) {
        if (frame.firstChild) { frame.removeChild(frame.firstChild); }
        image.style.cssText = "position:absolute;max-width:100%;max-height:100%;top:0;left:0;right:0;bottom:0;margin:auto;";
        image.setAttribute("aria-label", title.textContent);
        frame.appendChild(image);
    }
    var latest = null;
    function paint() {
        if (latest) {
            var canvas = doc.createElement("canvas");
            canvas.width = latest.naturalWidth || latest.width;
            canvas.height = latest.naturalHeight || latest.height;
            canvas.getContext("2d").drawImage(latest, 0, 0);
            showImage(canvas);
        }
        else {
            var stock = doc.createElement("img");
            stock.src = initial;
            showImage(stock);
        }
    }
    function language() {
        var manager = window.gUIManager || (window.scrui && window.scrui.gUIManager);
        var locale = manager && manager.mLocaleManager;
        var labels = messages[String(locale && locale.mLocaleCode).toLowerCase()] || messages["en-us"];
        trigger.textContent = labels[0];
        title.textContent = labels[1];
        panel.setAttribute("aria-label", labels[1]);
        close.setAttribute("aria-label", labels[2]);
        if (frame.firstChild) { frame.firstChild.setAttribute("aria-label", labels[1]); }
    }
    function dismiss() {
        shade.style.display = "none";
        trigger.focus();
    }
    trigger.onclick = function () {
        language();
        paint();
        shade.style.display = "block";
        close.focus();
    };
    close.onclick = dismiss;
    shade.onclick = function (event) { if (event.target === shade) { dismiss(); } };
    doc.addEventListener("keydown", function (event) {
        if (shade.style.display === "none") { return; }
        event.stopPropagation();
        if (event.keyCode === 27) { dismiss(); }
        if (event.keyCode === 27 || event.keyCode === 9) { event.preventDefault(); }
    }, true);
    language();
    return {
        language: language,
        update: function (image) {
            latest = image;
            if (shade.style.display !== "none") { paint(); }
        }
    };
})
