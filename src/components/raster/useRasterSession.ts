import { i18n } from "@/locales";
const t = i18n.global.t;
import { computed, ref, shallowRef } from "vue";
import { RasterDocument } from "@/lib/raster-editor/document";
import { RasterHistory } from "@/lib/raster-editor/history";
import { base64ToRgba, rgbaToBase64 } from "@/lib/raster-editor/encoders";
import {
  isMaskProfile,
  parseRasterSchema,
  validateParameters,
  validateSize,
  type ParameterTable,
  type RasterProfile,
  type RasterSchema,
  type RasterSource,
} from "@/lib/raster-editor/schema";

export function useRasterSession(initial?: RasterSchema) {
  const profile = ref<RasterProfile>(initial?.profile ?? "lot-mask");
  const name = ref(t("rasterEditor.text124"));
  const doc = shallowRef<RasterDocument | null>(null);
  const history = shallowRef<RasterHistory | null>(null);
  const parameters = ref<ParameterTable>({
    cols: 1,
    values: Array(16).fill(0),
  });
  const source = ref<RasterSource | null>(null);
  const dirty = ref(false);
  const version = ref(0);
  const sampling = ref({ generateMips: false, metersPerPixel: 0.75 });
  const lot = ref<RasterSchema["lot"]>({
    colors: [
      [0, 0, 0, 0],
      [255, 0, 0, 0],
      [0, 255, 0, 0],
      [0, 0, 255, 0],
    ],
    target: null,
  });
  const decal = ref<RasterSchema["decal"]>({
    colors: ["#ffffff", "#ffffff", "#ffffff", "#ffffff"],
    aspectRatio: 1,
    entryId: "",
    replaceInstance: "",
    atlas: null,
  });
  const mask = computed(() => isMaskProfile(profile.value));
  const canUndo = computed(() => {
    void version.value;
    return history.value?.canUndo ?? false;
  });
  const canRedo = computed(() => {
    void version.value;
    return history.value?.canRedo ?? false;
  });
  const ready = computed(() => profile.value === "slot0" || !!doc.value);
  function changed() {
    dirty.value = true;
    version.value++;
  }
  function install(
    image: RasterDocument,
    origin: RasterSource | null,
    title: string,
  ) {
    doc.value = image;
    history.value = new RasterHistory(image);
    source.value = origin;
    name.value = title;
    lot.value.target = null;
    decal.value.entryId = "";
    decal.value.replaceInstance = "";
    delete decal.value.entryTgi;
    decal.value.aspectRatio = image.width / image.height;
    changed();
  }
  function create(width: number, height: number) {
    validateSize(width, height);
    const image = new RasterDocument(width, height);
    if (profile.value === "slot2")
      for (let i = 0; i < image.pixels.length; i += 4)
        image.pixels.set([128, 128, 255, 255], i);
    install(image, null, t("rasterEditor.text125"));
  }
  function load(schema: RasterSchema) {
    const s = parseRasterSchema(schema);
    profile.value = s.profile;
    if (s.image && s.profile !== "slot0")
      install(
        new RasterDocument(
          s.image.width,
          s.image.height,
          base64ToRgba(s.image.data),
        ),
        s.source,
        s.name,
      );
    else {
      doc.value = null;
      history.value = null;
    }
    name.value = s.name;
    source.value = s.source;
    parameters.value = s.parameters ?? { cols: 1, values: Array(16).fill(0) };
    sampling.value = s.sampling;
    lot.value = s.lot;
    decal.value = s.decal;
    dirty.value = false;
  }
  function snapshot(): RasterSchema {
    if (!ready.value) throw new Error(t("rasterEditor.text126"));
    const image = doc.value;
    // JSON cloning unwraps Vue proxies before the strict, framework-free validator.
    return parseRasterSchema(
      JSON.parse(
        JSON.stringify({
          format: "openscp.raster",
          schema_version: 1,
          profile: profile.value,
          name: name.value,
          source: source.value,
          image:
            profile.value !== "slot0" && image
              ? {
                  width: image.width,
                  height: image.height,
                  encoding: "rgba8-base64",
                  data: rgbaToBase64(image.pixels),
                }
              : null,
          parameters:
            profile.value === "slot0"
              ? validateParameters(parameters.value)
              : null,
          sampling: sampling.value,
          lot: lot.value,
          decal: decal.value,
        }),
      ),
    );
  }
  function changeProfile(value: RasterProfile) {
    profile.value = value;
    lot.value.target = null;
    decal.value.atlas = null;
    sampling.value.generateMips = false;
    changed();
  }
  if (initial) load(initial);
  return {
    profile,
    name,
    doc,
    history,
    parameters,
    source,
    dirty,
    version,
    sampling,
    lot,
    decal,
    mask,
    ready,
    canUndo,
    canRedo,
    changed,
    install,
    create,
    load,
    snapshot,
    changeProfile,
  };
}
