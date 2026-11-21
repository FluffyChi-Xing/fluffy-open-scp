/* global scrui, simcity */
// Own runtime glue. Original game handlers/layouts are resolved locally, never vendored.
// ES5 deliberately: this also runs in the retail game's WebKit.
(function () {
  if (window.OpenSCPPanelRuntimeVersion) return;
  window.OpenSCPPanelRuntimeVersion = 'openscp.panel-runtime/1';
  function copy(source) { var out = {}, k; for (k in source) if (Object.prototype.hasOwnProperty.call(source, k)) out[k] = source[k]; return out; }
  function counterInit(info, native) {
    var p = info.layout, label = null;
    while (p && !label) { label = p.FindControlByID(226403367); p = p.mParentControl; }
    if (!label || !label.mLocaleString) throw Error('PANEL_COUNTER_LOCALE');
    var locale = label.mLocaleString;
    info.panelCounter = info.layout.FindControlByID(164377449);
    info.panelCounter.SetLocaleText(new scrui.cLocaleString(locale.mTableID, locale.mStringID));
    native.init.call(native, info.panelCounter);
  }
  function missing(info, data) {
    var active = data.normal ? info.normalRollover : info.vacantRollover;
    var locale = active && active.FindControlByID(222285689).mLocaleString;
    if (!locale || locale.mLoaded) return false;
    var manager = scrui.gUIManager.mLocaleManager, code = manager.mLocaleCode;
    var path = function (lang) { return 'Locale/' + lang + '/' + locale.mTableID + (/\.json$/i.test(locale.mTableID) ? '' : '.json'); };
    if (!manager.mCachedLocaleFiles[path(code)] || (code !== 'en-us' && !manager.mCachedLocaleFiles[path('en-us')])) throw Error('PANEL_LOCALE_TABLE_UNAVAILABLE:' + locale.mTableID);
    return true;
  }
  function register(component, hide) {
    var original = simcity.RolloverItems[component.native];
    if (!original) throw Error('PANEL_NATIVE_BINDING:' + component.native);
    var binding = copy(original);
    if (component.counter) binding.layoutPath = simcity.RolloverItems.kHoursPark.layoutPath;
    binding.init = function (info, data) {
      info.panelNative = copy(original);
      if (component.counter) counterInit(info, info.panelNative);
      else if (original.init) original.init.call(info.panelNative, info, data);
    };
    binding.update = function (info, data) {
      if (!info.panelNative) info.panelNative = copy(original);
      if (original.update) original.update.call(info.panelNative, info, data);
      this.minimize = !!info.panelNative.minimize || !!(hide && missing(info, data));
    };
    // Population updates must not receive the content info object.
    if (component.counter) binding.update = function (info, data) {
      original.update.call(info.panelNative, info.layout, data); info.panelCounter.SetVisibility(true); this.minimize = false;
    };
    simcity.RolloverItems['OpenSCPPanel_' + component.id + (component.feedback ? (hide ? '_hide' : '_native') : '')] = binding;
  }
  var all = window.OpenSCPPanelComponents, i;
  for (i = 0; i < all.length; i++) { register(all[i], false); if (all[i].feedback) register(all[i], true); }
  simcity.RolloverItems.OpenSCPPanel_empty = {
    layoutPath: simcity.RolloverItems.kEmptySpace10pt.layoutPath,
    init: function (info) { info.layout.SetPosition(0, 0, 0, 0); },
    update: function () { this.minimize = true; }
  };
}());
