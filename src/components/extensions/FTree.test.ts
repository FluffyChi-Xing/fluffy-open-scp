import { mount } from '@vue/test-utils'
import { defineComponent, shallowRef } from 'vue'
import { describe, expect, it } from 'vitest'
import FTree from './FTree.vue'
import type { FTreeNode } from './tree'

const nodes: FTreeNode[] = [{
  key: 'root',
  label: 'Root',
  children: [
    { key: 'read', label: 'Read' },
    { key: 'write', label: 'Write' }
  ]
}]

describe('FTree', () => {
  it('updates a child checkbox after its parent is checked', async () => {
    const Host = defineComponent({
      components: { FTree },
      setup() {
        const checkedKeys = shallowRef(['root', 'read', 'write'])
        return { checkedKeys, nodes }
      },
      template: '<FTree :data="nodes" checkable default-expand-all v-model:checked-keys="checkedKeys" />'
    })
    const wrapper = mount(Host)
    const checkboxes = wrapper.findAll('button[role="checkbox"]')

    expect(checkboxes).toHaveLength(3)
    expect(checkboxes[0].attributes('aria-checked')).toBe('true')

    await checkboxes[1].trigger('click')

    expect(wrapper.vm.checkedKeys).toEqual(['root', 'write'])
    expect(checkboxes[0].attributes('aria-checked')).toBe('mixed')
  })
})
