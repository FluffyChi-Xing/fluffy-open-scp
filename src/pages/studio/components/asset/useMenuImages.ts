import { reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import type { WorkbenchMenuBinding } from "./menuBinding";

const menuImageFields = { icon: "iconPng", marquee: "marqueePng" } as const;
type MenuImageKind = keyof typeof menuImageFields;

export function useMenuImages(
  getProject: () => string | undefined,
  getMenu: () => WorkbenchMenuBinding,
  updateMenu: (patch: Partial<WorkbenchMenuBinding>) => void,
) {
  const { t } = useI18n();
  const toast = useToast();
  const importingImage = ref(false);
  const previews = reactive({ icon: "", marquee: "" });
  const uploadRevision = reactive({ icon: 0, marquee: 0 });

  for (const kind of ["icon", "marquee"] as const) {
    watch(
      [getProject, () => getMenu()[menuImageFields[kind]], () => uploadRevision[kind]],
      async ([project, asset], _, onCleanup) => {
        let current = true;
        onCleanup(() => {
          current = false;
        });
        previews[kind] = "";
        if (!project || !asset) return;
        try {
          const result = await command<{ data: string | null }>("code_flow", {
            request: { project, action: "read-menu-image", payload: { kind, asset } },
          });
          if (current) previews[kind] = result.data ?? "";
        } catch (cause) {
          if (current) console.warn("[menu] image preview failed:", cause);
        }
      },
      { immediate: true },
    );
  }

  async function uploadMenuImage(kind: MenuImageKind): Promise<void> {
    const project = getProject();
    if (!project || importingImage.value) return;
    importingImage.value = true;
    try {
      const path = await tauriApi.workspace.pickImageFile(
        t(kind === "icon" ? "studio.assetWorkbench.menuIcon" : "studio.assetWorkbench.menuMarquee"),
      );
      if (typeof path !== "string" || getProject() !== project) return;
      const imported = await command<{ asset: string }>("code_flow", {
        request: { project, action: "import-menu-image", payload: { path, kind } },
      });
      if (getProject() !== project) return;
      updateMenu({ [menuImageFields[kind]]: imported.asset });
      // Uploads overwrite a fixed path, so a new file need not change the binding.
      uploadRevision[kind] += 1;
    } catch (cause) {
      toast.error(t("studio.assetWorkbench.importFailed"));
      console.warn("[menu] image upload failed:", cause);
    } finally {
      importingImage.value = false;
    }
  }

  return { previews, importingImage, uploadMenuImage };
}
