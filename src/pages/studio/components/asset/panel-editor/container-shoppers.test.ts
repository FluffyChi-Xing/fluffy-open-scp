import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { expect, it, vi } from 'vitest';

it('reads connected retail bins and shows the authored retail capacity without using tourist bins', () => {
  const update = vi.fn();
  const native = { update, updateQueries: (_unit: string) => ({}), minimize: false };
  const items: Record<string, typeof native> = { OpenSCPPanel_shoppers: native };
  const source = readFileSync(resolve('src/pages/studio/components/asset/panel-editor/native/container-shoppers.js'), 'utf8');
  runInNewContext(source, { simcity: {
    RolloverItems: items, kDataUnits: 'units', kDataConnectedUnits: 'connectedUnits', kDataBinAmount: 'binAmount',
    kDataNonSimProperties: 'nonSimProperties', kResourceIDShopperW1: 1, kResourceIDShopperW2: 2, kResourceIDShopperW3: 3,
  } });
  const binding = items.OpenSCPPanel_containerShoppers;
  expect(binding.updateQueries('host')).toEqual({
    low: ['connectedUnits', 'host', 'binAmount', 1], medium: ['connectedUnits', 'host', 'binAmount', 2],
    high: ['connectedUnits', 'host', 'binAmount', 3], capacity: ['units', 'host', 'nonSimProperties', 0xA6A430EF],
  });
  const info = {};
  binding.update(info, { low: 32, medium: 50, high: null, capacity: 200 });
  expect(update).toHaveBeenCalledWith(info, { shoppers: { amount: 82 }, openShoppers: { amount: 118 } });
  update.mockClear();
  binding.update(info, { low: null, medium: null, high: null, capacity: 200 });
  expect(binding.minimize).toBe(true);
  expect(update).not.toHaveBeenCalled();
  expect(native.minimize).toBe(false);
});
