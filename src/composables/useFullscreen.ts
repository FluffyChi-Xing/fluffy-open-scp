import { onBeforeUnmount, shallowRef } from 'vue'

export function useFullscreen() {
  const isFullscreen = shallowRef(Boolean(document.fullscreenElement))

  function onFullscreenChange() {
    isFullscreen.value = Boolean(document.fullscreenElement)
  }

  async function toggleFullscreen() {
    if (document.fullscreenElement) {
      await document.exitFullscreen().catch(() => {})
    } else {
      await document.documentElement.requestFullscreen().catch(() => {})
    }
  }

  document.addEventListener('fullscreenchange', onFullscreenChange)
  onBeforeUnmount(() => document.removeEventListener('fullscreenchange', onFullscreenChange))

  return { isFullscreen, toggleFullscreen }
}
