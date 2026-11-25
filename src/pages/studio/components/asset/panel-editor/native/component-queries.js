/* global simcity, scrui */
// ES5 for retail WebKit. Native query grammar: BuildUnitsJSValue at 779121-
// 779254 in SimCity-dev-ida.c. Discovery runs only until a stamped UnitRef is
// resolved, not every update. Never sum flags or select a different placement.
(function () {
  if (window.OpenSCPPanelComponentQueryVersion) return;
  window.OpenSCPPanelComponentQueryVersion = 1;
  var cache = {}, order = [], limit = 64;
  var roles = {
    'residence-feedback': 0xA00FE310, residents: 0xA00FE310,
    'land-value-residential': 0xA00FE310,
    'residential-happiness-reasons': 0xA00FE310,
    'residential-sadness-reasons': 0xA00FE310,
    'tourism-revenue': 0xA00FE311,
    'park-feedback': 0xA00FE312, 'park-hours': 0xA00FE312,
    'park-visitors-title': 0xA00FE312, 'park-visitors': 0xA00FE312
  };
  function copy(value) {
    var result = {}, key;
    for (key in value) if (Object.prototype.hasOwnProperty.call(value, key)) result[key] = value[key];
    return result;
  }
  function refKey(ref) {
    if (typeof ref === 'string' || typeof ref === 'number') return String(ref);
    return scrui.RequestStringFromUnitRef(ref);
  }
  function stateFor(host, role) {
    var key = refKey(host) + '|' + role;
    if (!Object.prototype.hasOwnProperty.call(cache, key)) {
      if (order.length >= limit) delete cache[order.shift()];
      order.push(key); cache[key] = { id: 0, ref: null, pending: 0, retry: 0 };
    }
    return cache[key];
  }
  function point(value) {
    if (!value) return null;
    var result = Array.isArray(value) ? value : [value.x, value.y, value.z];
    return result.length === 3 && result.every(function (v) { return typeof v === 'number' && isFinite(v); }) ? result : null;
  }
  function samePosition(a, b) {
    a = point(a); b = point(b);
    return !!a && !!b && a.every(function (v, i) { return Math.abs(v - b[i]) <= 0.001; });
  }
  function localQueries(binding, method, unit) {
    var result = binding[method] ? binding[method].call(this, unit) : {}, key;
    result = copy(result || {});
    for (key in result) if (Object.prototype.hasOwnProperty.call(result, key)) {
      var query = result[key];
      if (!Array.isArray(query)) continue;
      query = query.slice();
      if (query[0] === simcity.kDataConnectedUnits) query[0] = simcity.kDataUnits;
      if (query[0] === simcity.kDataSelectedUnit) query = [simcity.kDataUnits, unit].concat(query.slice(1));
      result[key] = query;
    }
    return result;
  }
  function query(host, role, inner) {
    var state = stateFor(host, role), result = {}, key, initial;
    if (state.ref) {
      result = localQueries.call(this, inner, 'updateQueries', state.ref);
      initial = localQueries.call(this, inner, 'initQueries', state.ref);
      for (key in initial) if (Object.prototype.hasOwnProperty.call(initial, key) && !Object.prototype.hasOwnProperty.call(result, key)) result[key] = initial[key];
      result._panelResolvedRef = [simcity.kDataUnits, state.ref, 'unitRef'];
      result._panelChildPosition = [simcity.kDataUnits, state.ref, 'position'];
    } else if (state.pending) {
      if (++state.pending > 20) state.pending = 0;
    } else if (state.retry) {
      state.retry--;
    } else if (state.id) {
      // Native match returns arrays, not a dictionary keyed by UnitRef.
      // Both filters enumerate slot order. The stamped-ref/position check in
      // the next batch rejects a result if units changed between these reads.
      result._panelCandidates = [simcity.kDataUnits, 'match', 'all', 'unitID-' + state.id,
        'ignoreUnitRefs', 'backupUnitRefs', 'component', 'unitRef'];
      result._panelCandidatePositions = result._panelCandidates.slice(0, -1).concat(['position']);
      state.pending = 1;
    }
    result._panelTarget = [simcity.kDataUnits, host, simcity.kDataNonSimProperties, role];
    result._panelHostPosition = [simcity.kDataUnits, host, 'position'];
    result._panelHostRef = [simcity.kDataUnits, host, 'unitRef'];
    return result;
  }
  function read(binding, role, data) {
    if (!data._panelHostRef) return null;
    var state = stateFor(data._panelHostRef, role), id = data._panelTarget, key;
    if (typeof id !== 'number' || !isFinite(id) || !(id >>> 0)) return null;
    id = id >>> 0;
    if (state.id !== id) { state.id = id; state.ref = null; state.pending = 0; state.retry = 0; return null; }
    if (!state.ref) {
      if (!Object.prototype.hasOwnProperty.call(data, '_panelCandidates')) return null;
      state.pending = 0;
      var candidates = data._panelCandidates, positions = data._panelCandidatePositions, matches = [], index;
      if (Array.isArray(candidates) && Array.isArray(positions) && candidates.length === positions.length) {
        for (index = 0; index < candidates.length; index++) {
          if (candidates[index] && samePosition(positions[index], data._panelHostPosition)) matches.push(refKey(candidates[index]));
        }
      }
      if (matches.length === 1) state.ref = matches[0];
      else state.retry = 60;
      return null;
    }
    // Another row can resolve this role while an earlier metadata-only batch
    // is still in flight. It must not invalidate the freshly resolved ref.
    if (!Object.prototype.hasOwnProperty.call(data, '_panelResolvedRef')) return null;
    if (!data._panelResolvedRef || refKey(data._panelResolvedRef) !== refKey(state.ref) || !samePosition(data._panelChildPosition, data._panelHostPosition)) {
      state.ref = null; return null;
    }
    var result = {};
    for (key in data) if (Object.prototype.hasOwnProperty.call(data, key) && key.indexOf('_panel') !== 0) result[key] = data[key];
    return result;
  }
  function register(item, role, suffix) {
    var inner = simcity.RolloverItems['OpenSCPPanel_' + item + suffix];
    if (!inner) return;
    var binding = copy(inner);
    binding.initQueries = function (host) { return query.call(this, host, role, inner); };
    binding.updateQueries = function (host) { return query.call(this, host, role, inner); };
    binding.init = function (info) {
      info.componentPending = true; this.minimize = true;
      info.layout.SetVisibility(false);
    };
    binding.update = function (info, data) {
      var current = read(this, role, data);
      if (!current) { this.minimize = true; return; }
      if (info.componentPending) {
        inner.init.call(this, info, current); info.componentPending = false;
      }
      inner.update.call(this, info, current);
    };
    simcity.RolloverItems['OpenSCPPanel_component_' + item + suffix] = binding;
  }
  Object.keys(roles).forEach(function (item) {
    if (item === 'residence-feedback' || item === 'park-feedback') {
      register(item, roles[item], '_hide'); register(item, roles[item], '_native');
    } else register(item, roles[item], '');
  });
}());
