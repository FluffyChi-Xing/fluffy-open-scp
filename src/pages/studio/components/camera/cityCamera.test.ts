import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { createPinia } from "pinia";
import type { EditorPackageScope } from "@/composables/useEditorPackages";
import { open } from "@tauri-apps/plugin-dialog";
import { command } from "@/api/tauri";
import { i18n } from "@/locales";
import Camera from "../../panels/camera.vue";
import CityCameraPreview from "./CityCameraPreview.vue";
import { cameraFields, cameraFrame, parseSamples } from "./cityCamera";
import { focusFields, focusPlane } from "./cityCameraFocus";
import { curveSampleLimits } from "./cameraCurveEditing";
import FSelect from "@/components/ui/FSelect.vue";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/api/tauri", () => ({ command: vi.fn() }));
const dependencies = vi.hoisted(() => ({ current: null as EditorPackageScope | null }));
vi.mock("@/composables/useEditorPackages", async original => {
  const vue = await import("vue");
  return {
    ...await original<typeof import("@/composables/useEditorPackages")>(),
    useEditorPackages: () => {
      const scope = vue.reactive({opened:[], directory:"", loading:false, missing:[], failed:false,
        ready:Promise.resolve(),load:vi.fn(),openFile:vi.fn()}) as EditorPackageScope;
      dependencies.current = scope;
      return scope;
    },
  };
});


const previousLocale = i18n.global.locale.value;
afterEach(() => {
  i18n.global.locale.value = previousLocale;
  vi.resetAllMocks();
});

it("keeps edited samples within the neighboring zoom distances and interpolated clip and pitch limits", () => {
  const values = [
    [75, 100, 150, 200],
    [35],
    [1, 7],
    [10, 12],
    [10, 15],
    [20],
    [270],
  ];
  const profile = {
    instance: 0x9e5a6813,
    curves: cameraFields.map(({ hash }, index) => ({
      hash,
      samples: values[index]!,
      inherited: false,
    })),
  };
  expect(curveSampleLimits(profile, "distance", 1)).toEqual([75, 150]);
  expect(curveSampleLimits(profile, "near", 1)[1]).toBeCloseTo(15);
  expect(curveSampleLimits(profile, "minPitch", 1)[1]).toBeCloseTo(23.33333);
});

it("previews edits above ground and exports the same field after switching language", async () => {
  const samples = [[75, 100], [35, 20], [10], [12000], [14], [80], [270, 315]];
  const instance = 0x9e5a6813;
  vi.mocked(open)
    .mockResolvedValueOnce("SimCity_App.package")
    .mockResolvedValueOnce(null)
    .mockResolvedValueOnce("D:/Camera Mods");
  vi.mocked(command)
    .mockResolvedValueOnce({
      revision: "revision",
      motionStrength: 1,
      profiles: [
        {
          instance,
          curves: [...cameraFields, ...focusFields].map(({ hash }, index) => ({
            hash,
            samples: [...samples, [0], [1024], [20], [500], [500]][index],
            inherited: false,
          })),
        },
      ],
    })
    .mockResolvedValueOnce("D:/Camera Mods/OpenSCP-City-Camera-2.package");
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(Camera, {
    global: {
      plugins: [i18n, createPinia()],
      stubs: { CityCameraPreview: true, FSelect: true },
    },
  });
  try {
    await wrapper.findAll("button").find(button => button.text() === i18n.global.t("studio.camera.open"))!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("城市镜头预览");
    const preview = () =>
      wrapper.getComponent(CityCameraPreview).props("frame");
    expect(preview().distance).toBe(75);
    expect(preview().eye[1]).toBeGreaterThan(0);
    const focusMode = wrapper
      .findAllComponents(FSelect)
      .find((item) => item.attributes("id") === "city-camera-focus-mode")!;
    focusMode.vm.$emit("change", "2");
    await nextTick();
    expect(wrapper.text()).toContain("对焦距离");
    await wrapper
      .findAll('.manual-fields input[type="number"]')[0]!
      .setValue("85");
    const shownFocus = () =>
      wrapper.getComponent(CityCameraPreview).props("profile");
    expect(focusPlane(shownFocus(), 20, 40, 35, 1.5)?.distance).toBe(85);
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "对比源包画面")!
      .trigger("click");
    expect(focusPlane(shownFocus(), 20, 40, 35, 1.5)).toBeNull();
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "返回草稿画面")!
      .trigger("click");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "播放转镜测试")!
      .trigger("click");
    expect(wrapper.getComponent(CityCameraPreview).props("playing")).toBe(true);
    await wrapper.get(".motion-controls input").setValue("0.375");
    expect(
      wrapper.getComponent(CityCameraPreview).props("motionStrength"),
    ).toBe(0.375);

    await wrapper.get("textarea").setValue("120 200");
    expect(preview().distance).toBe(120);
    await wrapper.get('input[type="range"]').setValue("1");
    expect(preview().distance).toBe(200);
    expect(preview().fov).toBe(27.5);
    expect(preview().eye[1]).toBeGreaterThan(0);
    await wrapper.get("textarea").setValue("200 120");
    expect(wrapper.findAll("button").find(button => button.text() === i18n.global.t("studio.camera.export"))?.attributes("disabled")).toBeDefined();
    await wrapper.get("textarea").setValue("120 200");

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(wrapper.text()).toContain("City camera preview");
    expect(wrapper.text()).toContain("Focus distance (m)");
    expect(wrapper.get("textarea").element.value).toBe("120 200");
    await wrapper.findAll("button").find(button => button.text() === i18n.global.t("studio.camera.export"))!.trigger("click");
    await flushPromises();
    expect(command).toHaveBeenCalledTimes(1);
    await wrapper.findAll("button").find(button => button.text() === i18n.global.t("studio.camera.export"))!.trigger("click");
    await flushPromises();
    expect(open).toHaveBeenLastCalledWith({
      directory: true,
      multiple: false,
      title: i18n.global.t("studio.camera.exportDirectory"),
    });
    expect(command).toHaveBeenLastCalledWith("export_city_camera", {
      request: {
        sourcePath: "SimCity_App.package",
        outputDirectory: "D:/Camera Mods",
        revision: "revision",
        motionStrength: 0.375,
        edits: [
          { instance, hash: cameraFields[0].hash, samples: [120, 200] },
          { instance, hash: focusFields[0].hash, samples: [2] },
          { instance, hash: focusFields[1].hash, samples: [85] },
        ],
      },
    });
    expect(wrapper.text()).toContain(
      "D:/Camera Mods/OpenSCP-City-Camera-2.package",
    );
  } finally {
    wrapper.unmount();
  }
});

it("loads an automatically acquired App package through the editor-owned handle", async () => {
  const pinia = createPinia();
  const wrapper = mount(Camera, {
    global: {
      plugins: [i18n, pinia],
      stubs: { CityCameraPreview: true, FSelect: true },
    },
  });
  try {
    const values = [[75, 100], [35], [1], [12000], [14], [80], [270]];
    vi.mocked(command).mockResolvedValueOnce({
      revision: "cached",
      motionStrength: 1,
      profiles: [
        {
          instance: 0x9e5a6813,
          curves: cameraFields.map(({ hash }, index) => ({
            hash,
            samples: values[index]!,
            inherited: false,
          })),
        },
      ],
    });
    dependencies.current!.opened = [{package:{packageId:23,path:"D:/SimCityData/SimCity_App.package"}}];
    await flushPromises();
    expect(open).not.toHaveBeenCalled();
    expect(command).toHaveBeenCalledExactlyOnceWith("read_city_camera", {
      packageId: 23,
    });
    const preview = wrapper.getComponent(CityCameraPreview);
    expect(preview.props("packageId")).toBe(23);
    const chart = wrapper.get("canvas.chart");
    await chart.trigger("keydown", { key: "ArrowRight" });
    expect(preview.props("frame").distance).toBe(100);
    const exact = wrapper.get(".sample-value input");
    await exact.setValue("125");
    await exact.trigger("change");
    expect(preview.props("frame").distance).toBe(125);
    await wrapper
      .findAll("button")
      .find((button) => button.text() === i18n.global.t("studio.camera.reset"))!
      .trigger("click");
    expect(preview.props("frame").distance).toBe(100);
    preview.vm.$emit("zoom", 1);
    await nextTick();
    expect(preview.props("frame").distance).toBe(100);
    preview.vm.$emit("tilt", -0.75);
    await nextTick();
    expect(preview.props("frame").pitch).toBe(14);
    preview.vm.$emit("reset");
    await nextTick();
    expect(preview.props("frame").distance).toBe(75);
  } finally {
    wrapper.unmount();
  }
});

it("uses signed pitch for the view direction while keeping the eye above the demonstration ground", () => {
  const minPitch = parseSamples(cameraFields[4].hash, "-20");
  expect(minPitch).toEqual([-20]);
  expect(parseSamples(cameraFields[4].hash, "-90")).toBeNull();
  const samples = [[75], [35], [1], [12000], minPitch!, [65], [0]];
  const frame = cameraFrame(
    {
      instance: 0x9e5a6813,
      curves: cameraFields.map(({ hash }, index) => ({
        hash,
        samples: samples[index]!,
        inherited: false,
      })),
    },
    0,
    0,
    0,
  )!;
  expect(frame.eye[1]).toBeGreaterThan(0);
  expect(frame.target[1]).toBeGreaterThan(frame.eye[1]);
  const run = Math.hypot(frame.eye[0], frame.eye[2]);
  expect(
    (Math.atan2(frame.target[1] - frame.eye[1], run) * 180) / Math.PI,
  ).toBeCloseTo(20);
});
