/* global simcity */
// Reuse the stock shopper label and layout. Connected ShopperW* bins count
// retail shoppers, while the authored capacity excludes tourism and park bins.
(function () {
  var native = simcity.RolloverItems.OpenSCPPanel_shoppers;
  var binding = {};
  for (var key in native) {
    if (Object.prototype.hasOwnProperty.call(native, key)) binding[key] = native[key];
  }
  binding.updateQueries = function (unit) {
    return {
      low: [simcity.kDataConnectedUnits, unit, simcity.kDataBinAmount, simcity.kResourceIDShopperW1],
      medium: [simcity.kDataConnectedUnits, unit, simcity.kDataBinAmount, simcity.kResourceIDShopperW2],
      high: [simcity.kDataConnectedUnits, unit, simcity.kDataBinAmount, simcity.kResourceIDShopperW3],
      capacity: [simcity.kDataUnits, unit, simcity.kDataNonSimProperties, 2795778287]
    };
  };
  binding.update = function (info, data) {
    var counts = [data.low, data.medium, data.high];
    if (!counts.some(validCount) || !validCount(data.capacity) || data.capacity === 0 ||
        counts.some(function (count) { return count !== null && !validCount(count); })) {
      this.minimize = true;
      return;
    }
    var amount = (data.low || 0) + (data.medium || 0) + (data.high || 0);
    native.update.call(this, info, { shoppers: { amount: amount }, openShoppers: { amount: data.capacity - amount } });
  };
  function validCount(value) { return typeof value === 'number' && isFinite(value) && value >= 0; }
  simcity.RolloverItems.OpenSCPPanel_containerShoppers = binding;
}());
