import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { describe, expect, it, vi } from 'vitest';

type Data = Record<string, unknown>;
type Info = { layout: { SetVisibility: ReturnType<typeof vi.fn> }; componentPending?: boolean };
type Binding = {
  initQueries: (unit: string) => Record<string, unknown[]>;
  updateQueries: (unit: string) => Record<string, unknown[]>;
  init: (info: Info, data?: Data) => void;
  update: (info: Info, data: Data) => void;
  minimize?: boolean;
};
const runtime = readFileSync(resolve('src/pages/studio/components/asset/panel-editor/native/component-queries.js'), 'utf8');
function setup(item = 'residents') {
  const init = vi.fn(), update = vi.fn();
  const original: Binding = {
    initQueries: unit => ({ feedbackFile: ['units', unit, 'nonSimProperties', 224007796] }),
    updateQueries: unit => ({ residents: ['connectedUnits', unit, 'amount', 123], normal: ['selectedUnit', 'flags', 14] }),
    init, update,
  };
  const items: Record<string, Binding> = { ['OpenSCPPanel_' + item]: original };
  runInNewContext(runtime, {
    window: {}, simcity: { RolloverItems: items, kDataUnits: 'units', kDataConnectedUnits: 'connectedUnits',
      kDataSelectedUnit: 'selectedUnit', kDataNonSimProperties: 'nonSimProperties' },
    scrui: { RequestStringFromUnitRef: (ref: { slot: number; stamp: number }) => `${ref.slot},${ref.stamp}` },
  });
  const binding = items['OpenSCPPanel_component_' + item];
  const info: Info = { layout: { SetVisibility: vi.fn() } };
  binding.init(info);
  return { binding, info, init, update };
}
const position = (x: number) => ({ x, y: 2, z: 3 });
function meta(host = { slot: 1, stamp: 7 }, x = 10): Data {
  return { _panelTarget: 0x84f16306, _panelHostRef: host, _panelHostPosition: position(x) };
}
function discover(binding: Binding, info: Info, host = '1,7', x = 10, ref = '11,9') {
  binding.updateQueries(host);
  const [slot, stamp] = host.split(',').map(Number);
  const [childSlot, childStamp] = ref.split(',').map(Number);
  binding.update(info, meta({ slot, stamp }, x));
  const query = binding.updateQueries(host);
  expect(query._panelCandidates).toEqual(['units', 'match', 'all', 'unitID-2230412038',
    'ignoreUnitRefs', 'backupUnitRefs', 'component', 'unitRef']);
  expect(query._panelCandidatePositions).toEqual(['units', 'match', 'all', 'unitID-2230412038',
    'ignoreUnitRefs', 'backupUnitRefs', 'component', 'position']);
  binding.update(info, { ...meta({ slot, stamp }, x),
    _panelCandidates: [{ slot: childSlot, stamp: childStamp }, { slot: 99, stamp: 2 }],
    _panelCandidatePositions: [position(x), position(x + 100)] });
}

describe('native component panel queries', () => {
  it('routes occupancy to the hotel component at the selected placement', () => {
    const { binding, info } = setup('hotel-occupancy');
    expect(binding.updateQueries('1,7')._panelTarget).toEqual(['units', '1,7', 'nonSimProperties', 0xA00FE313]);
    discover(binding, info);
    expect(binding.updateQueries('1,7').residents).toEqual(['units', '11,9', 'amount', 123]);
  });

  it.each(['park-visitors', 'park-hours', 'park-visitors-title', 'park-feedback_hide', 'park-feedback_native'])(
    'routes %s to the stamped park component without mixing residential or retail data', item => {
      const { binding, info, update } = setup(item);
      expect(binding.updateQueries('1,7')._panelTarget).toEqual(['units', '1,7', 'nonSimProperties', 0xA00FE312]);
      discover(binding, info);
      expect(binding.updateQueries('1,7').residents).toEqual(['units', '11,9', 'amount', 123]);
      binding.update(info, { ...meta(), _panelResolvedRef: { slot: 11, stamp: 9 },
        _panelChildPosition: position(10), residents: 0 });
      expect(update).toHaveBeenCalledWith(info, { residents: 0 });
    },
  );
  it('resolves the placement once, scopes every native query, and keeps real zero values', () => {
    const { binding, info, init, update } = setup();
    discover(binding, info);
    expect(init).not.toHaveBeenCalled();
    const queries = binding.updateQueries('1,7');
    expect(queries._panelCandidates).toBeUndefined();
    expect(queries.residents).toEqual(['units', '11,9', 'amount', 123]);
    expect(queries.normal).toEqual(['units', '11,9', 'flags', 14]);
    expect(queries.feedbackFile).toEqual(['units', '11,9', 'nonSimProperties', 224007796]);
    binding.update(info, { ...meta(), _panelResolvedRef: { slot: 11, stamp: 9 }, _panelChildPosition: position(10),
      residents: 0, normal: true, feedbackFile: 'native-residential.json' });
    expect(init).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledWith(info, { residents: 0, normal: true, feedbackFile: 'native-residential.json' });
    expect(binding.updateQueries('1,7')._panelCandidates).toBeUndefined();
  });

  it('keeps two placements separate even when their query callbacks arrive out of order', () => {
    const { binding, info, update } = setup();
    discover(binding, info);
    const second: Info = { layout: { SetVisibility: vi.fn() } };
    binding.init(second);
    discover(binding, second, '2,8', 200, '22,10');
    binding.updateQueries('1,7'); binding.updateQueries('2,8');
    binding.update(info, { ...meta(), _panelResolvedRef: { slot: 11, stamp: 9 }, _panelChildPosition: position(10), residents: 123 });
    expect(update).toHaveBeenLastCalledWith(info, { residents: 123 });
    binding.update(second, { ...meta({ slot: 2, stamp: 8 }, 200), _panelResolvedRef: { slot: 22, stamp: 10 }, _panelChildPosition: position(200), residents: 456 });
    expect(update).toHaveBeenLastCalledWith(second, { residents: 456 });
    expect(binding.updateQueries('1,7').residents[1]).toBe('11,9');
    expect(binding.updateQueries('2,8').residents[1]).toBe('22,10');
  });

  it('ignores old discovery batches and invalidates a destroyed or moved stamped reference', () => {
    const { binding, info, update } = setup();
    discover(binding, info);
    binding.update(info, meta());
    expect(binding.updateQueries('1,7').residents[1]).toBe('11,9');
    binding.update(info, { ...meta(), _panelResolvedRef: null, _panelChildPosition: null });
    expect(update).not.toHaveBeenCalled();
    expect(binding.minimize).toBe(true);
    expect(binding.updateQueries('1,7')._panelCandidates).toBeDefined();
  });

  it('does not choose an ambiguous placement or repeatedly scan the city when it is missing', () => {
    const { binding, info, update } = setup();
    binding.updateQueries('1,7'); binding.update(info, meta());
    binding.updateQueries('1,7');
    binding.update(info, { ...meta(), _panelCandidates: [{ slot: 11, stamp: 1 }, { slot: 22, stamp: 2 }],
      _panelCandidatePositions: [position(10), position(10)] });
    for (let index = 0; index < 30; index++) expect(binding.updateQueries('1,7')._panelCandidates).toBeUndefined();
    expect(update).not.toHaveBeenCalled();
    expect(binding.minimize).toBe(true);
  });

  it('rejects mismatched native result arrays instead of treating array indices as references', () => {
    const { binding, info, update } = setup();
    binding.updateQueries('1,7'); binding.update(info, meta());
    binding.updateQueries('1,7');
    binding.update(info, { ...meta(), _panelCandidates: [{ slot: 11, stamp: 9 }],
      _panelCandidatePositions: [position(10), position(20)] });
    expect(update).not.toHaveBeenCalled();
    expect(binding.minimize).toBe(true);
    expect(binding.updateQueries('1,7').residents).toBeUndefined();
  });
});
