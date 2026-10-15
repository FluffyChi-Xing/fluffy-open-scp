import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FSheet from '@/components/ui/FSheet.vue'
import FCode from '@/components/ui/FCode.vue'

const Host = {
  components: { FSheet, FCode },
  data: () => ({ open: false, schemaJson: '{\n  "version": 1\n}' }),
  template: `
  <FSheet v-model:open="open" label="Schema" width="72vw">
    <div class="schema-sheet-body">
      <div class="schema-toolbar">
        <span class="schema-hint">hint</span>
        <button type="button" class="schema-export" :disabled="!schemaJson">导出</button>
      </div>
      <FCode :code="schemaJson" lang="json" class="schema-code" />
    </div>
  </FSheet>`,
}

describe('schema sheet slot 结构', () => {
  it('open 后 slot 内容出现在 body', async () => {
    const wrapper = mount(Host, { attachTo: document.body })
    await wrapper.setData({ open: true })
    await wrapper.vm.$nextTick()
    const body = document.body.querySelector('.schema-sheet-body')
    expect(body).not.toBeNull()
    expect(document.body.querySelector('.schema-export')).not.toBeNull()
    wrapper.unmount()
  })
})
