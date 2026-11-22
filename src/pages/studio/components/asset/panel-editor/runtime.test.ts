import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { describe, expect, it } from 'vitest';

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
  const scrui = { cControlBase: { kPinStretch: 2 } };
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
