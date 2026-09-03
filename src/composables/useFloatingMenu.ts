import { nextTick, onBeforeUnmount, shallowRef, watch, type CSSProperties, type Ref } from 'vue'

export function useFloatingMenu(
  open: Ref<boolean>,
  anchor: Ref<HTMLElement | null>,
  panel: Ref<HTMLElement | null>,
  panelWidth = 220,
  gap = 8
) {
  const panelStyle = shallowRef<CSSProperties>({ top: '0' })

  async function position() {
    const el = anchor.value
    if (!el || !open.value) return
    await nextTick()
    const rect = el.getBoundingClientRect()
    const style: CSSProperties = { top: `${rect.bottom + gap}px` }
    if (rect.left + panelWidth <= window.innerWidth - gap) {
      style.left = `${Math.max(8, rect.left)}px`
    } else {
      style.right = `${Math.max(8, window.innerWidth - rect.right)}px`
    }
    panelStyle.value = style
  }

  function onPointerDown(event: Event) {
    const target = event.target as Node
    if (anchor.value?.contains(target) || panel.value?.contains(target)) return
    open.value = false
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') open.value = false
  }
  function onViewportChange() {
    if (open.value) open.value = false
  }

  watch(open, async (value) => {
    if (value) {
      document.addEventListener('pointerdown', onPointerDown, true)
      window.addEventListener('keydown', onKeydown)
      window.addEventListener('scroll', onViewportChange, true)
      window.addEventListener('resize', onViewportChange)
      await position()
    } else {
      document.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('keydown', onKeydown)
      window.removeEventListener('scroll', onViewportChange, true)
      window.removeEventListener('resize', onViewportChange)
    }
  })

  onBeforeUnmount(() => {
    document.removeEventListener('pointerdown', onPointerDown, true)
    window.removeEventListener('keydown', onKeydown)
    window.removeEventListener('scroll', onViewportChange, true)
    window.removeEventListener('resize', onViewportChange)
  })

  return { panelStyle, close: () => { open.value = false }, position }
}
