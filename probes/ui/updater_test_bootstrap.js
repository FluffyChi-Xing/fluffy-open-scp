(function (run) {
    "use strict";

    // Updater.html owns initialization. Run after its original body load handler.
    window.addEventListener("load", function () {
        var attempts = 0;
        function mount() {
            var manager = window.gUIManager || (window.scrui && window.scrui.gUIManager);
            var localeManager = manager && manager.mLocaleManager;
            var code = String(localeManager && localeManager.mLocaleCode || "").toLowerCase();
            var locale = code === "zh-tw" ? "zh-TW" : code === "zh-cn" ? "zh-CN" : "en-US";
            var reply = JSON.parse(run(JSON.stringify({
                action: "apply",
                pageUri: String(window.location.href),
                generation: 1,
                locale: locale
            })));
            attempts += 1;
            if (reply.status !== "success" && attempts < 40) {
                window.setTimeout(mount, 250);
            }
        }
        window.setTimeout(mount, 0);
    }, false);
})
