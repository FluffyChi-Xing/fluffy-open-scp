import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { i18n } from "@/locales";
import { RasterDocument } from "@/lib/raster-editor/document";
import { RasterHistory } from "@/lib/raster-editor/history";
import RasterCanvas from "./RasterCanvas.vue";

it("pans and zooms a rectangular Lot without re-rasterizing and batches dirty brush regions", async () => {
  const context = { fillRect: vi.fn(), setTransform: vi.fn(), clearRect: vi.fn() };
  const getContext = vi.spyOn(HTMLCanvasElement.prototype, "getContext")
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  const toDataURL = vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue("");
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frames.push(callback));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const doc = new RasterDocument(256, 512);
  const history = new RasterHistory(doc);
  const paintPreview = vi.fn();
  const wrapper = mount(RasterCanvas, {
    props: {
      doc, history, paintPreview, tool: "brush", brushSize: 1,
      northUp: true, lotSize: [90, 203], color: [255, 0, 0, 0],
    },
    global: { plugins: [i18n] },
  });
  try {
    Object.defineProperties(wrapper.element, {
      clientWidth: { value: 600 }, clientHeight: { value: 650 },
      setPointerCapture: { value: vi.fn() }, hasPointerCapture: { value: () => false },
    });
    wrapper.vm.fit();
    await nextTick();
    paintPreview.mockClear();
    context.setTransform.mockClear();
    const plane = wrapper.get(".canvas-plane");
    const transform = plane.attributes("style");
    const surface = wrapper.get("canvas");
    const originalBuffer = [surface.attributes("width"), surface.attributes("height")];
    await wrapper.trigger("wheel", { deltaY: -100, clientX: 300, clientY: 325 });
    expect(plane.attributes("style")).not.toBe(transform);
    const zoomed = plane.attributes("style");
    await wrapper.trigger("pointerdown", { button: 1, pointerId: 1, clientX: 280, clientY: 325 });
    await wrapper.trigger("pointermove", { pointerId: 1, clientX: 300, clientY: 325 });
    await wrapper.trigger("pointerup", { pointerId: 1, clientX: 300, clientY: 325 });
    expect(plane.attributes("style")).not.toBe(zoomed);
    expect(paintPreview).not.toHaveBeenCalled();
    expect(context.setTransform).not.toHaveBeenCalled();
    expect([surface.attributes("width"), surface.attributes("height")]).toEqual(originalBuffer);

    await surface.trigger("pointerdown", { button: 0, pointerId: 1, clientX: 320, clientY: 325 });
    expect(paintPreview).toHaveBeenCalledOnce();
    paintPreview.mockClear();
    frames.length = 0;
    await surface.trigger("pointermove", { pointerId: 1, clientX: 330, clientY: 325 });
    await surface.trigger("pointermove", { pointerId: 1, clientX: 345, clientY: 335 });
    expect(frames).toHaveLength(1);
    expect(paintPreview).not.toHaveBeenCalled();
    frames.shift()?.(0);
    expect(paintPreview).toHaveBeenCalledOnce();
    const [, , damage] = paintPreview.mock.calls[0];
    expect(damage.w * damage.h).toBeLessThan(1024);

    await surface.trigger("pointerup", { pointerId: 1, clientX: 345, clientY: 335 });
    expect(history.canUndo).toBe(true);
    expect(doc.pixels.some((value) => value === 255)).toBe(true);
    wrapper.vm.undo();
    frames.shift()?.(0);
    expect(doc.pixels.every((value) => value === 0)).toBe(true);
    expect(history.canRedo).toBe(true);

    await wrapper.setProps({ tool: "rect-fill" });
    paintPreview.mockClear();
    await surface.trigger("pointerdown", { button: 0, pointerId: 1, clientX: 320, clientY: 325 });
    await surface.trigger("pointermove", { pointerId: 1, clientX: 385, clientY: 390 });
    frames.shift()?.(0);
    const larger = doc.pixels.filter((value) => value === 255).length;
    expect(larger).toBeGreaterThan(100);
    await surface.trigger("pointermove", { pointerId: 1, clientX: 330, clientY: 335 });
    frames.shift()?.(0);
    const smaller = doc.pixels.filter((value) => value === 255).length;
    expect(smaller).toBeGreaterThan(0);
    expect(smaller).toBeLessThan(larger);
    expect(paintPreview.mock.calls[1][2]).toEqual(paintPreview.mock.calls[0][2]);
    wrapper.vm.cancelDraft();
    expect(doc.pixels.every((value) => value === 0)).toBe(true);
    expect(paintPreview.mock.calls.every(([, , region]) => region != null)).toBe(true);
  } finally {
    wrapper.unmount();
    getContext.mockRestore();
    toDataURL.mockRestore();
    vi.unstubAllGlobals();
  }
});
