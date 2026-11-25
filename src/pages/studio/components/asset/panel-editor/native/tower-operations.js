/* global scrui, simcity */
(function () {
  var ui = simcity.cRolloverUI;
  var items = {};
  var amount = function (unit, resource) { return [simcity.kDataConnectedUnits, unit, simcity.kDataBinAmount, resource]; };
  function withLocal(unit, queries) {
    Object.keys(queries).forEach(function (key) {
      var query = queries[key];
      if (query[0] === simcity.kDataConnectedUnits) {
        queries['local_' + key] = [simcity.kDataUnits].concat(query.slice(1));
      }
    });
    queries.singleBusiness = [simcity.kDataUnits, unit, simcity.kDataNonSimProperties, 0x739B3A2F];
    return queries;
  }
  function value(data, key) {
    var local = 'local_' + key;
    return (data.singleBusiness === 1 || data.singleBusiness === true) &&
      Object.prototype.hasOwnProperty.call(data, local) ? data[local] : data[key];
  }
  function requiredData(data) {
    return Object.keys(data).filter(function (key) {
      return key !== 'singleBusiness' && key.indexOf('local_') !== 0 &&
        !/^extraWorkers[123]$/.test(key) && value(data, key) === null;
    });
  }
  var control = function (info, name) { return info.layout.FindControlByID(ui[name]); };
  var locale = function (table, id) { return new scrui.cLocaleString(table, id, ''); };
  var tierSuffixes = ['', '2', '3'];

  items.kSkyTowerLevelOfficePopulationW1 = {
    layoutPath: 'Layouts/Rollovers/RolloverItemOfficeLevelWorkersW1.js',
    init: function (info) {
      info.workerBars = tierSuffixes.map(function (suffix, tier) {
        control(info, 'kWinIDRolloverItemIcon' + suffix).SetLocaleText(locale('BuildingRollover.json', ['0x0e5840a0', '0x0e5840a1', '0x0e5840a2'][tier]));
        var backing = control(info, 'kWinIDRolloverItemMeterBacking' + suffix);
        var text = control(info, 'kTxtIDRolloverItemValue' + suffix);
        text.SetPosition(0, text.mTop, backing.mWidth, text.mHeight);
        text.Private_SetHorizontalAlignment(scrui.cText.kHAlignCenter);
        return {
          backing: backing,
          fill: control(info, 'kWinIDRolloverItemMeterFillBar' + suffix),
          text: text
        };
      });
    },
    updateQueries: function (unit) {
      var result = {};
      for (var tier = 1; tier <= 3; tier++) {
        result['workers' + tier] = amount(unit, simcity['kResourceIDWorkerW' + tier]);
        result['jobs' + tier] = amount(unit, simcity['kResourceIDJobsW' + tier]);
        result['extraWorkers' + tier] = amount(unit, simcity['kResourceIDExtraWorkerW' + tier]);
      }
      return withLocal(unit, result);
    },
    update: function (info, data) {
      info.workerBars.forEach(function (bar, index) {
        var tier = index + 1;
        var workers = value(data, 'workers' + tier) + Math.max(0, value(data, 'extraWorkers' + tier) || 0);
        var jobs = value(data, 'jobs' + tier);
        bar.text.SetLocaleText(locale('BuildingRollover.json', '0x0e6ae893'), { amount: workers, capacity: jobs });
        bar.fill.SetWidth(jobs > 0 ? bar.backing.mWidth * Math.min(workers / jobs, 1) : 0);
      });
    }
  };

  items.kSkyTowerLevelMallPopulation = {
    layoutPath: 'Layouts/RolloverItemMallLevelShoppers.js',
    init: function (info) {
      info.mallText = control(info, 'kTxtIDRolloverItemValue');
      info.mallBacking = control(info, 'kWinIDRolloverItemMeterBacking');
      info.mallFill = control(info, 'kWinIDRolloverItemMeterFillBar');
      info.mallWarning = control(info, 'kWinIDRolloverItemMeterFillBar3');
      info.mallNeutral = control(info, 'kWinIDRolloverItemNeutralMeterFillBar');
    },
    updateQueries: function (unit) {
      return withLocal(unit, {
        normal: [simcity.kDataUnits, unit, simcity.kDataFlags, simcity.kUnitFlagNormal],
        average: [simcity.kDataConnectedUnits, unit, simcity.kDataBins, 0x13014ACE]
      });
    },
    update: function (info, data) {
      var average = value(data, 'average');
      var capacity = average ? Math.floor(average.capacity / 12) : 0;
      this.minimize = !data.normal || capacity <= 0;
      if (this.minimize) return;
      var shoppers = Math.min(average.amount, capacity);
      var ratio = shoppers / capacity;
      info.mallText.UpdateLocaleText({ amount: shoppers, capacity: capacity });
      info.mallWarning.SetVisibility(ratio < 0.1);
      info.mallNeutral.SetVisibility(ratio >= 0.1);
      info.mallFill.SetWidth(info.mallBacking.mWidth * ratio);
    }
  };

  items.kBlockSkyTowerLevelProfit = {
    layoutPath: 'Layouts/Rollovers/RolloverItemIncomeProfit.js',
    init: function (info) {
      info.profitRows = ['1', '2'].map(function (suffix) {
        var text = control(info, 'kTxtIDRolloverItemTextRight' + suffix);
        text.SetLocaleText(locale('PaletteInfoPanel.json', '0x0ee14f78'));
        return text;
      });
    },
    updateQueries: function (unit) {
      return withLocal(unit, {
        incomeToday: amount(unit, 0x0735A595), expensesToday: amount(unit, 0x0D0B5542),
        incomeYesterday: amount(unit, 0xCD51E152), expensesYesterday: amount(unit, 0xCCF9FA95)
      });
    },
    update: function (info, data) {
      ['Today', 'Yesterday'].forEach(function (day, index) {
        var profit = Math.max(value(data, 'income' + day), 0) - value(data, 'expenses' + day);
        var row = info.profitRows[index];
        row.UpdateLocaleText({ cost: profit });
        row.SetTextColor(simcity.RolloverItems[profit > 0 ? 'textColorNormal' : profit < 0 ? 'textColorUrgent' : 'textColorBlack']);
      });
    }
  };
  window.OpenSCPPanelTowerBindings = items;
  Object.keys(items).forEach(function (key) { items[key].missingQueries = requiredData; });
}());
