import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { expect, it, vi } from "vitest";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import { i18n } from "@/locales";
import MenuBindingForm from "./MenuBindingForm.vue";
import { emptyMenuBinding } from "./menuBinding";

vi.mock("@/api", () => ({ tauriApi: { workspace: { pickImageFile: vi.fn() } } }));
vi.mock("@/api/tauri", () => ({ command: vi.fn() }));

it("refreshes the bound background after a same-path upload and translates its preview label", async () => {
  const asset = "assets/images/menu-marquee.jpg";
  let data = "data:image/jpeg;base64,b2xk";
  vi.mocked(tauriApi.workspace.pickImageFile).mockResolvedValue("source.png");
  vi.mocked(command).mockImplementation(async (_name, args) => {
    const request = args?.request as { action: string; payload: { asset: string } };
    if (request.action === "import-menu-image") {
      data = "data:image/jpeg;base64,bmV3";
      return { asset };
    }
    expect(request.payload.asset).toBe(asset);
    return { data };
  });

  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(MenuBindingForm, {
    global: { plugins: [i18n] },
    props: {
      project: "Example",
      menu: { ...emptyMenuBinding(), enabled: true, marqueePng: asset },
    },
  });
  try {
    await flushPromises();
    const background = wrapper.get("button.marquee");
    expect(background.get("img").attributes("src")).toBe(data);
    expect(background.get("img").attributes("alt")).toBe("菜单背景图");
    expect(background.get(".mf-upload-clear").attributes("title")).toBe("移除图片");

    await background.trigger("click");
    await flushPromises();
    expect(wrapper.emitted("update:menu")).toEqual([[{ marqueePng: asset }]]);
    expect(background.get("img").attributes("src")).toBe("data:image/jpeg;base64,bmV3");

    i18n.global.locale.value = "en-US";
    await nextTick();
    expect(background.get("img").attributes("alt")).toBe("Menu background image");
    expect(background.get(".mf-upload-clear").attributes("title")).toBe("Remove image");

    await wrapper.setProps({ menu: { ...wrapper.props("menu"), marqueePng: "" } });
    expect(background.find("img").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    i18n.global.locale.value = previousLocale;
  }
});
