import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { describe, expect, it, vi } from 'vitest';
import messages from '@/lib/panel-messages.json';

const runtime = readFileSync(resolve('src/pages/studio/components/asset/panel-editor/native/runtime.js'), 'utf8');

function control(width = 105, height = 15) {
  return {
    mLeft: -8, mTop: -11, mWidth: width, mHeight: height, mScale: 1, mIsVisible: true,
    mHorizontalPinType: 0, mVerticalPinType: 0,
    mRootElement: { style: { overflow: '' } },
    SetPosition(left: number, top: number, w?: number, h?: number) {
      this.mLeft = left; this.mTop = top; this.mWidth = w ?? this.mWidth; this.mHeight = h ?? this.mHeight;
    },
    SetVisibility(visible: boolean) { this.mIsVisible = visible; },
    SetPinning(horizontal: number, vertical = 0) { this.mHorizontalPinType = horizontal; this.mVerticalPinType = vertical; },
  };
}

function setup() {
  let updates = 0, nativePositions = 0;
  const original = {
    layoutPath: 'native-layout',
    init(info: { textControl?: unknown; layout: { FindControlByID: (id: number) => unknown } }) {
      info.textControl = info.layout.FindControlByID(2);
    },
    update() { updates++; },
  };
  const simcity = {
    RolloverItems: {
      kMaintenance: original, kShoppers: original,
      kEmptySpace10pt: { layoutPath: 'native-empty' },
    } as Record<string, Record<string, unknown>>,
    cRolloverUI: Object.assign(function () {}, {
      prototype: { UpdateItemPositions() { nativePositions++; } },
      kWinIDRolloverItemIcon: 1, kWinWhiteBox: 3, kWinIDRolloverItemContainer: 4,
    }),
  };
  const scrui = { cControlBase: { kPinStretch: 2 }, cText: { kVAlignCenter: 1 } };
  runInNewContext(runtime, {
    window: { OpenSCPPanelComponents: [
      { id: 'maintenance', native: 'kMaintenance' }, { id: 'shoppers', native: 'kShoppers' },
    ] }, simcity, scrui,
  });
  const icon = { ...control(33, 34), mScale: 0.5 };
  const text = control(83, 15), parent = control(279, 200);
  const row = { ...control(), mParentControl: parent, FindControlByID: (id: number) => id === 1 ? icon : text };
  // Deliberately dynamic: the production script is evaluated across a VM boundary.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const bindings = simcity.RolloverItems as Record<string, any>;
  return { bindings, simcity, icon, text, parent, row, original,
    updates: () => updates, positions: () => nativePositions };
}

describe('panel runtime adapters', () => {
  it('reports local hotel occupancy in the active language and preserves real zero values', () => {
    const simcity = {
      RolloverItems: { kCasinoLodgingTitle: {}, kEmptySpace10pt: {} } as Record<string, {
        updateQueries?: (unit: number) => Record<string, unknown[]>;
        update?: (info: unknown, data: Record<string, unknown>) => void;
      }>,
      cRolloverUI: { prototype: {} },
      kDataUnits: 'unit', kDataBinAmount: 'amount', kDataProperties: 'properties',
      kResourceIDGuestW2: 0x18C38F50, kResourceIDOpenGuestsW2: 0x4E35BFA5,
    };
    const locale = { mLocaleCode: 'en-us' };
    runInNewContext(runtime, {
      window: { OpenSCPPanelComponents: ['hotel-occupancy', 'controlnet-workers', 'controlnet-production', 'fusion-generation', 'fusion-charge', 'fusion-controlnet']
        .map(id => ({ id, native: 'kCasinoLodgingTitle' })),
        OpenSCPPanelMessages: messages },
      simcity, scrui: { gUIManager: { mLocaleManager: locale } },
    });
    const binding = simcity.RolloverItems['OpenSCPPanel_hotel-occupancy'];
    expect(binding.updateQueries!(42)).toEqual({
      occupied: ['unit', 42, 'amount', 0x18C38F50],
      available: ['unit', 42, 'amount', 0x4E35BFA5],
      capacity: ['unit', 42, 'properties', 0x0E2660C6],
    });
    const title = vi.fn(), status = vi.fn();
    const info = { textControlTitle: { SetText: title },
      textControlTitleRight: { SetText: status, SetVisibility: vi.fn() } };
    binding.update!(info, { occupied: 0, available: 84, capacity: 84 });
    expect(title).toHaveBeenLastCalledWith('Room occupancy');
    expect(status).toHaveBeenLastCalledWith('0/84 occupied, 84 available');
    locale.mLocaleCode = 'zh-tw';
    binding.update!(info, { occupied: 84, available: 0, capacity: 84 });
    expect(title).toHaveBeenLastCalledWith('客房入住');
    expect(status).toHaveBeenLastCalledWith('84/84 间已入住，0 间空房');
    binding.update!(info, { occupied: null, available: 34, capacity: 84 });
    expect(status).toHaveBeenLastCalledWith('暂无客房数据');

    const tiered = simcity.RolloverItems['OpenSCPPanel_hotel-occupancy-tiered'];
    expect(tiered.updateQueries!(42).occupiedW3).toEqual(['unit', 42, 'amount', 0x18C38F51]);
    tiered.update!(info, { wealth: 3, occupied: 40, occupiedW3: 12, availableW3: 48, capacity: 60 });
    expect(status).toHaveBeenLastCalledWith('12/60 间已入住，48 间空房');

    const workforce = simcity.RolloverItems['OpenSCPPanel_controlnet-workers'];
    expect(workforce.updateQueries!(42).w1).toEqual(['unit', 42, 'amount', 0x97235AA7]);
    workforce.update!(info, { w1: 0, j1: 25, w2: 0, j2: 50, w3: 0, j3: 100 });
    expect(status).toHaveBeenLastCalledWith('低 0/25 · 中 0/50 · 高 0/100');
    const production = simcity.RolloverItems['OpenSCPPanel_controlnet-production'];
    expect(production.updateQueries!(42).generated).toEqual(['unit', 42, 'amount', 0xFCAB1E3F]);
    locale.mLocaleCode = 'en-us';
    production.update!(info, { generated: 0, maximum: 5250 });
    expect(status).toHaveBeenLastCalledWith('Cycle 0 · Max 5250/h');
    production.update!(info, { generated: null, maximum: 5250 });
    expect(status).toHaveBeenLastCalledWith('ControlNet data unavailable');

    const generation = simcity.RolloverItems['OpenSCPPanel_fusion-generation'];
    expect(generation.updateQueries!(42)).toEqual({
      amount: ['unit', 42, 'amount', 0x3EB59739], limit: ['unit', 42, 'properties', 0x0AFB9881],
    });
    generation.update!(info, { amount: 0, limit: 15555 });
    expect(status).toHaveBeenLastCalledWith('Cycle 0 · Limit 15555');
    const charge = simcity.RolloverItems['OpenSCPPanel_fusion-charge'];
    expect(charge.updateQueries!(42).amount).toEqual(['unit', 42, 'amount', 0x8E8B4E10]);
    const stock = simcity.RolloverItems['OpenSCPPanel_fusion-controlnet'];
    expect(stock.updateQueries!(42).amount).toEqual(['unit', 42, 'amount', 0x5737C2E4]);
    stock.update!(info, { amount: null });
    expect(stock).toHaveProperty('minimize', true);
    stock.update!(info, { amount: 0 });
    expect(stock).toHaveProperty('minimize', false);
    expect(status).toHaveBeenLastCalledWith('0');
  });

  it('reads the selected Unit for a single business and keeps connected totals for existing containers', () => {
    const adapters = readFileSync(resolve('src/pages/studio/components/asset/panel-editor/native/tower-operations.js'), 'utf8');
    const window: { OpenSCPPanelTowerBindings?: Record<string, {
      updateQueries(unit: number): Record<string, unknown[]>;
      update(info: unknown, data: Record<string, number>): void;
      missingQueries(data: Record<string, number | boolean | null>): string[];
    }> } = {};
    runInNewContext(adapters, {
      window, scrui: { cLocaleString: class {} },
      simcity: {
        kDataUnits: 'unit', kDataConnectedUnits: 'connected', kDataBinAmount: 'amount',
        kDataNonSimProperties: 'nonSim', kDataBins: 'bins', cRolloverUI: {}, RolloverItems: {},
        kResourceIDExtraWorkerW1: 1627828683,
      },
    });
    const bindings = window.OpenSCPPanelTowerBindings!;
    const profit = bindings.kBlockSkyTowerLevelProfit;
    expect(profit.updateQueries(42).local_incomeToday).toEqual(['unit', 42, 'amount', 0x0735A595]);
    expect(bindings.kSkyTowerLevelMallPopulation.updateQueries(42).local_average).toEqual(['unit', 42, 'bins', 0x13014ACE]);
    const workers = bindings.kSkyTowerLevelOfficePopulationW1;
    const workerQueries = workers.updateQueries(42);
    expect(workerQueries.local_jobs1.slice(0, 3)).toEqual(['unit', 42, 'amount']);
    expect(workerQueries.extraWorkers1).toEqual(['connected', 42, 'amount', 1627828683]);
    expect(workerQueries.local_extraWorkers1).toEqual(['unit', 42, 'amount', 1627828683]);
    expect(workers.missingQueries({
      singleBusiness: null, workers1: 15, jobs1: 450, extraWorkers1: null,
      local_workers1: null, local_jobs1: null, local_extraWorkers1: null,
    })).toEqual([]);
    expect(workers.missingQueries({
      singleBusiness: true, workers1: null, jobs1: null, extraWorkers1: null,
      local_workers1: 15, local_jobs1: 350, local_extraWorkers1: null,
    })).toEqual([]);
    expect(workers.missingQueries({
      singleBusiness: 1, workers1: 15, jobs1: 450, local_workers1: null, local_jobs1: 350,
    })).toEqual(['workers1']);
    const workerReadings: Array<{ amount: number; capacity: number }> = [];
    const workerBar = {
      text: { SetLocaleText(_label: unknown, reading: { amount: number; capacity: number }) { workerReadings.push(reading); } },
      backing: { mWidth: 100 }, fill: { SetWidth() {} },
    };
    const workersData = {
      workers1: 15, jobs1: 450, extraWorkers1: 40,
      local_workers1: 5, local_jobs1: 350, local_extraWorkers1: 136,
    };
    workers.update({ workerBars: [workerBar] }, { ...workersData, singleBusiness: 1 });
    workers.update({ workerBars: [workerBar] }, workersData);
    expect(workerReadings).toEqual([{ amount: 141, capacity: 350 }, { amount: 55, capacity: 450 }]);
    const shown: number[] = [];
    const row = { UpdateLocaleText({ cost }: { cost: number }) { shown.push(cost); }, SetTextColor() {} };
    const data = {
      incomeToday: 9, expensesToday: 4, incomeYesterday: 8, expensesYesterday: 5,
      local_incomeToday: 25, local_expensesToday: 10, local_incomeYesterday: 31, local_expensesYesterday: 11,
    };
    profit.update({ profitRows: [row, row] }, { ...data, singleBusiness: 1 });
    profit.update({ profitRows: [row, row] }, data);
    expect(shown).toEqual([15, 20, 5, 3]);
  });

  it('retains the native spacing between all three labor bars when fitting the panel row', () => {
    const tracks = [
      { ...control(142), mLeft: 18 },
      { ...control(47), mLeft: 165 },
      { ...control(41), mLeft: 217 },
    ];
    const parent = control(279);
    const row = { ...control(278), mParentControl: parent, mChildren: tracks };
    const bindings: Record<string, unknown> = { kEmptySpace10pt: {} };
    runInNewContext(runtime, {
      window: {
        OpenSCPPanelComponents: [{ id: 'workers-by-wealth', native: 'kOffice', towerOperations: true }],
        OpenSCPPanelTowerBindings: { kOffice: {} },
      },
      simcity: { RolloverItems: bindings, cRolloverUI: { prototype: {} } },
      scrui: { cControlBase: { kPinStretch: 2 } },
    });
    const binding = bindings['OpenSCPPanel_workers-by-wealth'] as { init(info: unknown): void };
    binding.init({ layout: row });
    expect(row.mWidth).toBe(279);
    expect(tracks.map(({ mLeft, mWidth }) => [mLeft, mWidth])).toEqual([[18, 142], [165, 47], [217, 41]]);
  });

  it('renders up to three native reasons and clears stale lines as the inputs change', () => {
    const active = [control(), control(), control()];
    const inactive = [control(), control(), control()];
    const untranslated = new Set<string>();
    class LocaleString {
      mLoaded = false;
      constructor(_table: string, private readonly key: string) {}
      LoadString() { this.mLoaded = !untranslated.has(this.key); }
    }
    const info = {
      panelNative: {}, textControlLeft1: active[0], textControlLeft2: active[1], textControlLeft3: active[2],
      textControlRight1: inactive[0], textControlRight2: inactive[1], textControlRight3: inactive[2],
    };
    const update = (
      function (this: { getRequiredCount(): number; titleText: string; minimize: boolean; additionalUpdate(info: unknown, data: unknown): [number, string[]][] },
        row: unknown, data: { normal: boolean; textFeedbackFile: string }) {
        expect(this.titleText).toBe('native-title');
        expect(data.textFeedbackFile).toBe('native-feedback');
        const reasons = this.additionalUpdate(row, data).sort((a, b) => b[0] - a[0]);
        this.minimize = !data.normal || reasons.length === 0;
        active.slice(0, this.getRequiredCount()).forEach(line => line.SetVisibility(true));
      }
    );
    const original = {
      update,
      titleText: '',
      getRequiredCount: () => 3,
      updateQueries() {
        this.titleText = 'native-title';
        return { first: ['binAmount'], second: ['bins'], third: ['flags'] };
      },
      additionalUpdate: (_info: unknown, data: {first: number; second: { amount: number }; third: boolean}) =>
        [[data.first, ['first']], [data.second.amount, ['second']], [Number(data.third), ['third']]],
    };
    const bindings: Record<string, unknown> = { kNativeReasons: original, kEmptySpace10pt: {} };
    runInNewContext(runtime, {
      window: { OpenSCPPanelComponents: [{ id: 'reasons', native: 'kNativeReasons', reasonList: true }] },
      simcity: { RolloverItems: bindings, kDataBinAmount: 'binAmount', kDataBins: 'bins', kDataFlags: 'flags', cRolloverUI: { prototype: {} } },
      scrui: { gUIManager: { mLocaleManager: { mAsynchronous: false } }, cLocaleString: LocaleString },
    });
    const binding = bindings.OpenSCPPanel_reasons as { updateQueries(unit: string): unknown; update(info: unknown, data: unknown): void; minimize: boolean };
    binding.updateQueries('1,0');
    const data = { normal: true, textFeedbackFile: 'native-feedback', happinessDisplay: 3, first: 8, second: { amount: 7 }, third: true };
    // The real binding creates a separate native receiver for every panel row.
    info.panelNative = { ...original, titleText: '' };
    binding.update(info, data);
    expect(active.map(line => line.mIsVisible)).toEqual([true, true, true]);
    untranslated.add('first');
    binding.update(info, data);
    expect(active.map(line => line.mIsVisible)).toEqual([true, true, false]);
    untranslated.clear();
    binding.update(info, { ...data, third: null });
    expect(active.map(line => line.mIsVisible)).toEqual([true, true, false]);
    binding.update(info, { ...data, first: null, third: null });
    expect(active.map(line => line.mIsVisible)).toEqual([true, false, false]);
    binding.update(info, { ...data, first: null, second: null, third: null });
    expect(binding.minimize).toBe(true);
    expect(active.map(line => line.mIsVisible)).toEqual([false, false, false]);
    binding.update(info, { ...data, textFeedbackFile: null });
    expect(binding.minimize).toBe(true);
    expect(original.titleText).toBe('');
  });

  it('opts custom blocks into null delivery and hides only the missing block, then recovers', () => {
    const f = setup(), binding = f.bindings.OpenSCPPanel_maintenance;
    const info = { layout: f.row, panelMissingQueries: [] };
    binding.init(info, {});
    expect(f.row.mIsVisible).toBe(false);
    expect(binding.allowNullResults).toBe(true);
    binding.update(info, { maintCost: null });
    expect(binding.minimize).toBe(true);
    expect([...info.panelMissingQueries]).toEqual(['maintCost']);
    expect(f.updates()).toBe(0);
    binding.update(info, { maintCost: { amount: 52500 } });
    expect(binding.minimize).toBe(false);
    expect(f.updates()).toBe(1);
    expect(f.original).not.toHaveProperty('allowNullResults');
  });

  it('fits shoppers icon and text in the row without negative coordinates', () => {
    const f = setup(), binding = f.bindings.OpenSCPPanel_shoppers;
    binding.init({ layout: f.row }, {});
    expect(f.row.mWidth).toBe(f.parent.mWidth);
    expect(f.icon.mLeft).toBeGreaterThanOrEqual(0);
    expect(f.icon.mTop).toBeGreaterThanOrEqual(0);
    expect(f.text.mLeft).toBeGreaterThanOrEqual(f.icon.mWidth * f.icon.mScale);
    expect(f.text.mLeft + f.text.mWidth).toBeLessThanOrEqual(f.row.mWidth);
    expect(f.row.mHeight).toBeGreaterThanOrEqual(f.icon.mHeight * f.icon.mScale);
  });

  it('hides the unused population header during init, before any query response', () => {
    const f = setup(), header = control();
    f.bindings.OpenSCPPanel_headerNone.init(header);
    expect(header.mIsVisible).toBe(false);
  });

  it('replaces the native grid side reservation with symmetric stretch anchors', () => {
    const f = setup();
    const frame = { ...control(267, 60), mLeft: 3, mTop: 2, mVerticalPinType: 0,
      SetPinning(horizontal: number) { this.horizontal = horizontal; }, horizontal: -1 };
    Object.assign(f.row, { mWidth: 309, mChildren: [frame] });
    f.bindings.OpenSCPPanel_maintenance.init({ layout: f.row }, {});
    expect(frame.mWidth).toBe(309 - 2 * frame.mLeft);
    expect(frame.horizontal).toBe(2);
  });

  it('aligns custom rows with the native white box and clips decorative overflow', () => {
    const f = setup(), box = { ...control(265, 80), mLeft: 6 };
    const ui = { mItemInfoArray: [{ dataBinding: f.bindings.OpenSCPPanel_shoppers, layout: f.row }],
      mCurrentLayout: { FindControlByID: (id: number) => id === 3 ? box : f.parent } };
    f.simcity.cRolloverUI.prototype.UpdateItemPositions.call(ui);
    expect(f.parent.mLeft).toBe(box.mLeft);
    expect(f.parent.mTop).toBe(box.mTop);
    expect(f.parent.mWidth).toBe(box.mWidth);
    expect(f.row.mWidth).toBe(box.mWidth);
    expect(f.parent.mHorizontalPinType).toBe(2);
    expect(f.row.mHorizontalPinType).toBe(2);
    expect(f.parent.mHeight).toBe(box.mHeight);
    expect(f.parent.mRootElement.style.overflow).toBe('hidden');
    expect(f.positions()).toBe(1);
  });

  it('leaves inherited panels on the original position handler', () => {
    const f = setup();
    f.simcity.cRolloverUI.prototype.UpdateItemPositions.call({ mItemInfoArray: [] });
    expect(f.positions()).toBe(1);
  });

  it('reflows visible feedback inside the row before stacking other controls', () => {
    const f = setup();
    const box = control(279);
    const row = { ...control(279, 28), mIsVisible: true };
    const branch = { ...control(279, 28), mTop: -4, mParentControl: row };
    const frame = { ...control(279, 17), mTop: 6, mParentControl: branch };
    const refresh = vi.fn();
    const text = {
      ...control(279, 28), mParentControl: frame,
      GetTextHeight: () => 34,
      Private_SetVerticalAlignment: vi.fn(),
      Private_UpdateTextSubComponents: refresh,
    };
    const normalRollover = { ...branch, FindControlByID: () => text };
    f.simcity.cRolloverUI.prototype.UpdateItemPositions.call({
      mItemInfoArray: [{ layout: row, normalRollover, dataBinding: { panelSlot: true } }],
      mCurrentLayout: { FindControlByID: (id: number) => id === 3 ? box : f.parent },
    });
    expect(row.mHeight).toBe(42);
    expect(text.mHeight).toBe(row.mHeight);
    expect(text.mTop + frame.mTop + branch.mTop).toBe(0);
    expect(refresh).toHaveBeenCalledOnce();
  });

  it.each([222464035, 222514320])('centers the nested feedback text path (%s)', branchId => {
    const row = { ...control(317, 40), mParentControl: control(317, 200),
      FindControlByID: (() => null) as (id: number) => unknown };
    const branch = { ...control(261, 19), mParentControl: row,
      FindControlByID: (() => null) as (id: number) => unknown };
    const text = { ...control(244, 25), mLeft: 9, mParentControl: branch, mHorizontalAlignment: 0,
      Private_SetHorizontalAlignment(value: number) { this.mHorizontalAlignment = value; } };
    branch.FindControlByID = () => text;
    row.FindControlByID = id => id === branchId ? branch : null;
    const simcity = { RolloverItems: { kFeedback: {}, kEmptySpace10pt: { layoutPath: 'empty' } } as Record<string, Record<string, unknown>>,
      cRolloverUI: { prototype: { UpdateItemPositions() {} } } };
    runInNewContext(runtime, { window: { OpenSCPPanelComponents: [{ id: 'feedback', native: 'kFeedback', feedback: true }] },
      simcity, scrui: { cControlBase: { kPinStretch: 2 }, cText: { kHAlignCenter: 1 } } });
    const binding = simcity.RolloverItems.OpenSCPPanel_feedback_native;
    (binding.init as (info: unknown) => void)({ layout: row });
    expect(branch.mLeft).toBe(0);
    expect(branch.mWidth).toBe(row.mWidth);
    expect(text.mLeft + text.mWidth / 2).toBe(branch.mWidth / 2);
    expect(text.mHorizontalAlignment).toBe(1);
    expect(text.mHorizontalPinType).toBe(2);
  });
});
