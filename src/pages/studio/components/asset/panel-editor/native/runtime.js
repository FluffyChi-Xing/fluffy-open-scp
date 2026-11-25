/* global scrui, simcity */
// Own runtime glue. Original game handlers/layouts are resolved locally, never vendored.
// ES5 deliberately: this also runs in the retail game's WebKit.
(function () {
  if (window.OpenSCPPanelRuntimeVersion) return;
  window.OpenSCPPanelRuntimeVersion = 'openscp.panel-runtime/1';
  function copy(source) { var out = {}, k; for (k in source) if (Object.prototype.hasOwnProperty.call(source, k)) out[k] = source[k]; return out; }
  function missingData(data) {
    var keys = [], key;
    for (key in data) if (Object.prototype.hasOwnProperty.call(data, key) && data[key] === null) keys.push(key);
    return keys;
  }
  var containerQuery = function (unit) {
    return [simcity.kDataUnits, unit, simcity.kDataNonSimProperties, 0x0FB2DB9A];
  };
  var towerLedger = { incomeToday: 0x0735A595, expensesToday: 0x0D0B5542,
    incomeYesterday: 0xCD51E152, expensesYesterday: 0xCCF9FA95 };
  function withContainerCounters(unit, queries, ledger) {
    queries.panelContainer = containerQuery(unit);
    Object.keys(ledger).forEach(function (key) {
      queries['component_' + key] = [simcity.kDataConnectedUnits, unit, simcity.kDataBinAmount, ledger[key]];
    });
    return queries;
  }
  function readContainerCounters(data, combine) {
    data = copy(data);
    Object.keys(data).forEach(function (key) {
      if (key.indexOf('component_') !== 0) return;
      var field = key.slice(10), amount = data[key];
      if ((data.panelContainer === true || data.panelContainer === 1) && typeof amount === 'number' && amount >= 0) {
        if (!combine) data[field] = amount;
        else if (typeof data[field] === 'number') data[field] += amount;
      }
      delete data[key];
    });
    delete data.panelContainer;
    return data;
  }
  function reasonData(binding, data) {
    // Unallocated optional counters contribute no reason. Keep the native
    // state, feedback table and display inputs mandatory.
    var result = copy(data), queries = binding.panelReasonQueries || {}, key, query;
    for (key in queries) {
      if (!Object.prototype.hasOwnProperty.call(queries, key) || result[key] !== null ||
          /^(normal|textFeedbackFile|happinessDisplay|wealth|openForBusiness)$/.test(key)) continue;
      query = queries[key];
      if (query.indexOf(simcity.kDataBins) !== -1) result[key] = { amount: 0, capacity: 0 };
      else if (query.indexOf(simcity.kDataBinAmount) !== -1) result[key] = 0;
      else if (query.indexOf(simcity.kDataFlags) !== -1) result[key] = false;
    }
    return result;
  }
  function updateReasons(binding, info, data) {
    var native = info.panelNative, additionalUpdate = native.additionalUpdate, reasons = [];
    var requiredCount = native.getRequiredCount;
    if (data.normal && (data.openForBusiness === undefined || data.openForBusiness)) {
      var manager = scrui.gUIManager && scrui.gUIManager.mLocaleManager;
      reasons = additionalUpdate.call(native, info, data).filter(function (reason) {
        if (!(reason[0] > 0)) return false;
        if (!manager || manager.mAsynchronous) return true;
        // Same severity index and optional table override as kBlockSadnessGeneric.
        var strings = reason[1], index = Math.max(0, strings.length - 2 - data.happinessDisplay);
        if (!strings[index]) return false;
        var locale = new scrui.cLocaleString(reason[3] || data.textFeedbackFile, strings[index]);
        locale.LoadString();
        return locale.mLoaded;
      });
    }
    native.titleText = binding.titleText;
    var count = Math.min(reasons.length, 3);
    native.getRequiredCount = function () { return count; };
    native.additionalUpdate = function () { return reasons; };
    for (var i = 1; i <= 3; i++) {
      info['textControlLeft' + i].SetVisibility(false);
      info['textControlRight' + i].SetVisibility(false);
    }
    try { native.update(info, data); }
    finally { native.additionalUpdate = additionalUpdate; native.getRequiredCount = requiredCount; }
  }
  function fitSlot(info, component) {
    var layout = info.layout, parent = layout.mParentControl;
    if (!parent) return;
    // Modular blocks occupy one full row, rather than the native quarter grid.
    layout.SetPosition(0, 0, parent.mWidth, layout.mHeight);
    if (component.id === 'shoppers') {
      var icon = layout.FindControlByID(simcity.cRolloverUI.kWinIDRolloverItemIcon);
      // WebKit scales about the center; reserve the unscaled control box so
      // the scaled icon cannot extend below its row or overlap the value.
      var text = info.textControl, width = icon.mWidth;
      var height = Math.max(layout.mHeight, icon.mHeight, text.mHeight);
      icon.SetPosition(0, (height - icon.mHeight) / 2);
      text.SetPosition(width + 4, (height - text.mHeight) / 2, parent.mWidth - width - 4, text.mHeight);
      layout.SetPosition(0, 0, parent.mWidth, height);
    }
  }
  function stretchNativeContent(layout) {
    // Native rows may reserve a transparent side area for their original grid.
    // Re-anchor the broad content frames using their own inset, before sizing
    // the row. Their nested right-pinned values then follow every later resize.
    var children = layout.mChildren || [], i;
    for (i = 0; i < children.length; i++) {
      var child = children[i];
      if (child.mLeft >= 0 && child.mWidth > layout.mWidth / 2) {
        child.SetPosition(child.mLeft, child.mTop, Math.max(0, layout.mWidth - 2 * child.mLeft), child.mHeight);
        child.SetPinning(scrui.cControlBase.kPinStretch, child.mVerticalPinType);
      }
    }
  }
  function centerFeedback(layout) {
    var ids = [222464035, 222514320], i, j;
    for (i = 0; i < ids.length; i++) {
      var branch = layout.FindControlByID(ids[i]);
      var text = branch && branch.FindControlByID(222285689);
      if (!text) continue;
      var ancestors = [], parent = text.mParentControl;
      while (parent && parent !== layout) { ancestors.push(parent); parent = parent.mParentControl; }
      if (parent !== layout) continue;
      // The normal/vacant feedback templates have different nested side areas.
      // Both text paths must span the row, not just their outer window.
      for (j = ancestors.length - 1; j >= 0; j--) {
        var frame = ancestors[j];
        frame.SetPosition(0, frame.mTop, frame.mParentControl.mWidth, frame.mHeight);
        frame.SetPinning(scrui.cControlBase.kPinStretch, frame.mVerticalPinType);
      }
      var inset = Math.max(0, Math.min(text.mLeft, text.mParentControl.mWidth - text.mLeft - text.mWidth));
      text.SetPosition(inset, text.mTop, Math.max(0, text.mParentControl.mWidth - 2 * inset), text.mHeight);
      text.SetPinning(scrui.cControlBase.kPinStretch, text.mVerticalPinType);
      text.Private_SetHorizontalAlignment(scrui.cText.kHAlignCenter);
    }
  }
  function fitFeedbackRow(info) {
    var row = info.layout;
    var branch = info.normalRollover && info.normalRollover.mIsVisible ? info.normalRollover : info.vacantRollover;
    var text = branch && branch.FindControlByID(222285689);
    if (!text || !text.mIsVisible) return;
    // Native feedback can measure its wrapped text while the row is hidden.
    // Measure again after visibility is set, before reserving its stack space.
    text.Private_SetVerticalAlignment(scrui.cText.kVAlignCenter);
    var height = Math.max(row.mHeight, text.GetTextHeight() + 8);
    row.SetPosition(row.mLeft, row.mTop, row.mWidth, height);
    var offset = 0, parent = text.mParentControl;
    while (parent && parent !== row) { offset += parent.mTop; parent = parent.mParentControl; }
    if (parent !== row) return;
    text.SetPosition(text.mLeft, -offset, text.mWidth, height);
    text.Private_UpdateTextSubComponents();
  }
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
    var original = component.towerOperations
      ? (window.OpenSCPPanelTowerBindings || {})[component.native]
      : simcity.RolloverItems[component.native];
    if (component.towerOperations && !original) return;
    if (!original) throw Error('PANEL_NATIVE_BINDING:' + component.native);
    var binding = copy(original);
    binding.panelSlot = true;
    if (component.singleUnit) binding.panelSingleUnit = true;
    if (component.towerOperations) binding.panelTowerOperations = true;
    // The native batch decoder otherwise aborts every block on the first null.
    // Missing simulation data hides only its own block; it is never invented.
    binding.allowNullResults = true;
    var commercialReasons = component.reasonList && /^commercial-/.test(component.id);
    var buildingProfit = component.id === 'profit';
    if (component.reasonList) binding.updateQueries = function (unit) {
      this.panelReasonQueries = original.updateQueries.call(this, unit);
      if (!commercialReasons) return this.panelReasonQueries;
      var counters = {}, queries = this.panelReasonQueries;
      Object.keys(queries).forEach(function (key) {
        var query = queries[key];
        if (query[0] === simcity.kDataUnits && query[2] === simcity.kDataBinAmount) counters[key] = query[3];
      });
      return simcity.kDataConnectedUnits ? withContainerCounters(unit, copy(queries), counters) : queries;
    };
    if (component.native === 'kBlockLandmarkProfit' && !component.metric) {
      binding.updateQueries = function (unit) {
        var queries = original.updateQueries.call(original, unit), used = {};
        var fields = ['incomeToday', 'expensesToday', 'incomeYesterday', 'expensesYesterday'], index;
        for (index = 0; index < fields.length; index++) used[fields[index]] = queries[fields[index]];
        // Only a managed building adds component accounts to its host ledger.
        return buildingProfit && simcity.kDataConnectedUnits ? withContainerCounters(unit, used, towerLedger) : used;
      };
    }
    if (component.metric) binding.updateQueries = function (unit) {
      return {
        current: [simcity.kDataUnits, unit, simcity.kDataBins, simcity['kResourceID' + component.metric.current]],
        previous: [simcity.kDataUnits, unit, simcity.kDataBins, simcity['kResourceID' + component.metric.previous]]
      };
    };
    if (component.counter) binding.layoutPath = simcity.RolloverItems.kHoursPark.layoutPath;
    binding.init = function (info, data) {
      info.panelNative = copy(original);
      if (component.counter) counterInit(info, info.panelNative);
      else if (original.init) original.init.call(info.panelNative, info, data);
      // Some native title layouts include a second title the handler never binds.
      if (info.textControlTitle && !info.textControlTitleRight) {
        var unusedTitle = info.layout.FindControlByID(simcity.cRolloverUI.kTxtIDRolloverItemTitleRight);
        if (unusedTitle) unusedTitle.SetVisibility(false);
      }
      // The office template has three adjacent meters. Expanding its first
      // track to the row width puts it underneath the other two tracks.
      if (component.id !== 'workers-by-wealth') stretchNativeContent(info.layout);
      if (component.feedback) centerFeedback(info.layout);
      fitSlot(info, component);
      info.layout.SetVisibility(false);
    };
    binding.update = function (info, data) {
      if (!info.panelNative) info.panelNative = copy(original);
      if (buildingProfit || commercialReasons) data = readContainerCounters(data, buildingProfit);
      if (component.reasonList) data = reasonData(this, data);
      info.panelMissingQueries = original.missingQueries ? original.missingQueries(data) : missingData(data);
      if (info.panelMissingQueries.length) { this.minimize = true; return; }
      if (component.metric) {
        var code = scrui.gUIManager.mLocaleManager.mLocaleCode;
        var labels = window.OpenSCPPanelMessages[/^zh/i.test(code) ? 'zh-CN' : 'en-US'];
        info.textControlTitle.SetText(labels.profitIndex);
        info.textControlLeft1.SetText(labels.current);
        info.textControlLeft2.SetText(labels.previous);
        info.textControlRight1.SetText(String(data.current.amount));
        info.textControlRight2.SetText(String(data.previous.amount));
      } else if (component.reasonList) updateReasons(this, info, data);
      else if (original.update) original.update.call(info.panelNative, info, data);
      this.minimize = !!info.panelNative.minimize || !!(hide && missing(info, data));
    };
    // Population updates must not receive the content info object.
    if (component.counter) binding.update = function (info, data) {
      info.panelMissingQueries = missingData(data);
      if (info.panelMissingQueries.length) { this.minimize = true; return; }
      original.update.call(info.panelNative, info.layout, data); info.panelCounter.SetVisibility(true); this.minimize = false;
    };
    simcity.RolloverItems['OpenSCPPanel_' + component.id + (component.feedback ? (hide ? '_hide' : '_native') : '')] = binding;
  }
  var all = window.OpenSCPPanelComponents, i;
  for (i = 0; i < all.length; i++) { register(all[i], false); if (all[i].feedback) register(all[i], true); }
  simcity.RolloverItems.OpenSCPPanel_headerNone = {
    allowNullResults: true,
    init: function (control) { control.SetVisibility(false); },
    update: function () {}
  };
  simcity.RolloverItems.OpenSCPPanel_empty = {
    panelSlot: true,
    layoutPath: simcity.RolloverItems.kEmptySpace10pt.layoutPath,
    init: function (info) { info.layout.SetPosition(0, 0, 0, 0); },
    update: function () { this.minimize = true; }
  };
  var nativePositions = simcity.cRolloverUI.prototype.UpdateItemPositions;
  simcity.cRolloverUI.prototype.UpdateItemPositions = function () {
    var items = this.mItemInfoArray || [], custom = false, i;
    for (i = 0; i < items.length; i++) if (items[i].dataBinding.panelSlot) custom = true;
    if (!custom) return nativePositions.call(this);
    var box = this.mCurrentLayout.FindControlByID(simcity.cRolloverUI.kWinWhiteBox);
    var container = this.mCurrentLayout.FindControlByID(simcity.cRolloverUI.kWinIDRolloverItemContainer);
    container.SetPosition(box.mLeft, box.mTop, box.mWidth, container.mHeight);
    container.SetPinning(scrui.cControlBase.kPinStretch, container.mVerticalPinType);
    for (i = 0; i < items.length; i++) if (items[i].dataBinding.panelSlot) {
      var row = items[i].layout;
      row.SetPosition(row.mLeft, row.mTop, box.mWidth, row.mHeight);
      row.SetPinning(scrui.cControlBase.kPinStretch, row.mVerticalPinType);
      if (row.mIsVisible) fitFeedbackRow(items[i]);
    }
    nativePositions.call(this);
    container.SetPosition(container.mLeft, container.mTop, container.mWidth, box.mHeight);
    // Native feedback backings/separators deliberately extend outside their row.
    // Clip those decorations to the authored content region after native sizing.
    container.mRootElement.style.overflow = 'hidden';
  };
}());
