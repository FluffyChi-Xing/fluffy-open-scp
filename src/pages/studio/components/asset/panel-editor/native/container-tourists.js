/* global simcity */
// Keep the native tourist labels and counts. Supplies the authored tourism
// capacity separately from the commercial shopper pool on connected units.
(function () {
  var native = simcity.RolloverItems.OpenSCPPanel_tourists;
  var binding = {}, key;
  for (key in native) {
    if (Object.prototype.hasOwnProperty.call(native, key)) binding[key] = native[key];
  }
  binding.updateQueries = function (unit) {
    var queries = native.updateQueries.call(this, unit);
    delete queries.openShoppers;
    queries.shoppers = [simcity.kDataUnits, unit, simcity.kDataNonSimProperties, 2496942424];
    return queries;
  };
  binding.update = function (info, data) {
    native.update.call(this, info, {
      TouristW1: data.TouristW1,
      TouristW2: data.TouristW2,
      TouristW3: data.TouristW3,
      shoppers: data.shoppers,
      openShoppers: 0
    });
  };
  simcity.RolloverItems.OpenSCPPanel_containerTourists = binding;
}());
