import { computed, ref, toValue, type MaybeRefOrGetter } from "vue";
import { isTauri, tauriApi } from "@/api";
import type { AudioPreview, ImagePreview, Tgi } from "@/api/tauri";
import { useToast } from "@/composables/useToast";

export type ResourceExportFormat = "png" | "jpg" | "gif" | "wav" | "mp3" | "ogg" | "flac";
export type ExportablePreview = ImagePreview | AudioPreview;

const DEFAULT_AUDIO_FORMAT: ResourceExportFormat = "wav";

export function useResourceExport(preview: MaybeRefOrGetter<ExportablePreview>) {
  const toast = useToast();
  const exporting = ref(false);
  const currentPreview = computed(() => toValue(preview));

  async function exportResource(format: ResourceExportFormat) {
    const activePreview = currentPreview.value;
    if (!isTauri() || !activePreview.packageId || !activePreview.tgi) {
      toast.warning("资源导出需要在 Tauri 桌面应用中使用");
      return;
    }
    exporting.value = true;
    try {
      const extension = format;
      const outputPath = await tauriApi.packages.saveFile(
        defaultFileName(activePreview.tgi, extension),
        extension,
      );
      if (!outputPath) return;
      const accepted = await tauriApi.packages.export(
        activePreview.packageId,
        activePreview.tgi,
        format,
        outputPath,
      );
      await waitForExport(accepted.jobId);
      toast.success(`已导出 ${extension.toUpperCase()} 文件`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "资源导出失败");
    } finally {
      exporting.value = false;
    }
  }

  return { exporting, exportResource };
}

export function defaultImageFormat(mime: string): ResourceExportFormat {
  if (mime.includes("jpeg") || mime.includes("jpg")) return "jpg";
  if (mime.includes("gif")) return "gif";
  return "png";
}

export function defaultAudioExportFormat(): ResourceExportFormat {
  return DEFAULT_AUDIO_FORMAT;
}

function defaultFileName(tgi: Tgi, extension: string) {
  return `openscp-${tgi.instance.toString(16).padStart(8, "0")}.${extension}`;
}

async function waitForExport(jobId: number) {
  for (let attempt = 0; attempt < 600; attempt += 1) {
    const status = await tauriApi.packages.exportStatus(jobId);
    if (status.phase === "succeeded") return;
    if (status.phase === "failed") {
      throw new Error(status.error ?? "资源导出失败");
    }
    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
  throw new Error("资源导出超时");
}
