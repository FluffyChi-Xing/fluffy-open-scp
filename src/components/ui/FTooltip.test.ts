import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FTooltip from '@/components/ui/FTooltip.vue'

describe('FTooltip', () => {
  it('mounts with a trigger slot (disabled button proxy)', () => {
    const wrapper = mount(FTooltip, {
      props: { text: '物料', side: 'right' },
      slots: {
        trigger:
          '<span class="rail-item-wrap"><button type="button" class="rail-item" disabled>▲</button></span>',
      },
      attachTo: document.body,
    })
    expect(document.body.querySelector('[class*="rail-item"]')).not.toBeNull()
    wrapper.unmount()
  })

  it('mounts with an enabled button trigger and default text', () => {
    const wrapper = mount(FTooltip, {
      props: { text: 'Schema', side: 'bottom' },
      slots: { trigger: '<button type="button" class="editor-close" />' },
      attachTo: document.body,
    })
    expect(document.body.querySelector('.editor-close')).not.toBeNull()
    wrapper.unmount()
  })
})
