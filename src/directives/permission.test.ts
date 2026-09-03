import { defineComponent, nextTick, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { createPermissionContext } from '@/permissions/context'
import { createPermissionDirective } from './permission'

function mountFixture(initialTokens: string[] = []) {
  const context = createPermissionContext(initialTokens)
  const Fixture = defineComponent({
    directives: { permission: createPermissionDirective(context, '|') },
    setup() {
      const required = ref<string | string[]>('cms.article.read|cms.article.write')
      return { required }
    },
    template: '<button v-permission="required">编辑</button>'
  })
  return { context, wrapper: mount(Fixture) }
}

describe('v-permission', () => {
  it('allows an element when any required token matches', () => {
    const { wrapper } = mountFixture(['cms.article.write'])
    expect(wrapper.find('button').element.hidden).toBe(false)
  })

  it('restores the element after reactive token changes', async () => {
    const { context, wrapper } = mountFixture()
    const button = wrapper.find('button').element
    expect(button.hidden).toBe(true)
    expect(button.getAttribute('aria-hidden')).toBe('true')

    context.setTokens(['cms.article.read'])
    await nextTick()
    wrapper.vm.required = ['cms.article.read']
    await nextTick()

    expect(button.hidden).toBe(false)
    expect(button.hasAttribute('aria-hidden')).toBe(false)
  })
})
