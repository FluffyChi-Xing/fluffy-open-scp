import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import AudioPreview from "./AudioPreview.vue";
import VideoPreview from "./VideoPreview.vue";
import { i18n } from "@/locales";
import { useToast } from "@/composables/useToast";
import type { AudioPreview as AudioPreviewData, VideoPreview as VideoPreviewData } from "@/api/tauri";

const toast = useToast();

const audioPreview: AudioPreviewData = {
  kind: "audio",
  offset: 0,
  totalLength: 128,
  bytes: [],
  src: "asset://preview.wav",
  mime: "audio/wav",
  toolAvailable: true,
  toolName: "vgmstream",
  installCommand: "winget install --id vgmstream.vgmstream -e --source winget",
};

const missingAudio: AudioPreviewData = {
  ...audioPreview,
  src: null,
  toolAvailable: false,
};

const videoPreview: VideoPreviewData = {
  kind: "video",
  offset: 0,
  totalLength: 256,
  bytes: [],
  src: "asset://preview.mp4",
  mime: "video/mp4",
  toolAvailable: true,
  toolName: "ffmpeg",
  installCommand: "winget install --id Gyan.FFmpeg -e --source winget",
};

const missingVideo: VideoPreviewData = {
  ...videoPreview,
  src: null,
  toolAvailable: false,
};

describe("media preview viewers", () => {
  afterEach(() => toast.clear());

  it("renders an audio player when vgmstream is available", () => {
    const wrapper = mount(AudioPreview, {
      props: { preview: audioPreview },
      global: { plugins: [i18n] },
    });
    expect(wrapper.find("audio").exists()).toBe(true);
    expect(wrapper.find("audio").attributes("src")).toBe(audioPreview.src);
  });

  it("shows the vgmstream install command and toast when unavailable", () => {
    const wrapper = mount(AudioPreview, {
      props: { preview: missingAudio },
      global: { plugins: [i18n] },
    });
    expect(wrapper.find("audio").exists()).toBe(false);
    expect(wrapper.text()).toContain(missingAudio.installCommand);
    expect(toast.toasts.value.at(-1)?.tone).toBe("error");
  });

  it("renders a video player when ffmpeg is available", () => {
    const wrapper = mount(VideoPreview, {
      props: { preview: videoPreview },
      global: { plugins: [i18n] },
    });
    expect(wrapper.find("video").exists()).toBe(true);
    expect(wrapper.find("video").attributes("src")).toBe(videoPreview.src);
  });

  it("shows the ffmpeg install command and toast when unavailable", () => {
    const wrapper = mount(VideoPreview, {
      props: { preview: missingVideo },
      global: { plugins: [i18n] },
    });
    expect(wrapper.find("video").exists()).toBe(false);
    expect(wrapper.text()).toContain(missingVideo.installCommand);
    expect(toast.toasts.value.at(-1)?.tone).toBe("error");
  });
});
