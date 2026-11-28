import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { expect, it, vi } from 'vitest';

it('keeps native live tourist counts and reads configured capacity without the retail pool', () => {
  const counts = {
    TouristW1: ['connectedUnits', 'host', 'binAmount', 1],
    TouristW2: ['connectedUnits', 'host', 'binAmount', 2],
    TouristW3: ['connectedUnits', 'host', 'binAmount', 3],
  };
  const update = vi.fn();
  const native = {
    updateQueries: (_unit: string) => ({ ...counts, shoppers: ['retail'], openShoppers: ['retail-open'] }),
    update,
  };
  const items: Record<string, typeof native> = { OpenSCPPanel_tourists: native };
  const source = readFileSync(
    resolve('src/pages/studio/components/asset/panel-editor/native/container-tourists.js'),
    'utf8',
  );
  runInNewContext(source, {
    simcity: { RolloverItems: items, kDataUnits: 'units', kDataNonSimProperties: 'nonSimProperties' },
  });
  const binding = items.OpenSCPPanel_containerTourists;
  expect(binding.updateQueries('host')).toEqual({
    ...counts,
    shoppers: ['units', 'host', 'nonSimProperties', 0x94d45158],
  });
  const info = {};
  binding.update(info, { TouristW1: 5, TouristW2: 12, TouristW3: 3, shoppers: 450 });
  expect(update).toHaveBeenCalledWith(info, {
    TouristW1: 5, TouristW2: 12, TouristW3: 3, shoppers: 450, openShoppers: 0,
  });
  expect(items.OpenSCPPanel_tourists).toBe(native);
  expect(native.updateQueries('host').openShoppers).toEqual(['retail-open']);
});
