import { defineComponent, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FForm from '@/components/form/FForm.vue'
import type { FormColumn } from '@/components/form/types'

const columns: FormColumn<Record<string, string | boolean>>[] = [
  { field: 'name', labelKey: 'form.name', type: 'text', required: true },
  { field: 'hidden', labelKey: 'form.hidden', type: 'text', visible: () => false }
]

function mountForm() {
  return mount(defineComponent({
    components: { FForm },
    setup() {
      const model = ref<Record<string, string | boolean>>({ name: '', hidden: '' })
      return { columns, model }
    },
    template: '<FForm v-model="model" :columns="columns" />'
  }), {
    global: { mocks: { $t: (key: string) => key } }
  })
}

describe('FForm', () => {
  it('validates required fields and excludes hidden columns', async () => {
    const wrapper = mountForm()
    expect(wrapper.find('label[for="field-hidden"]').exists()).toBe(false)

    await wrapper.find('form').trigger('submit')
    expect(wrapper.findComponent(FForm).emitted('submit')).toBeUndefined()
    expect(wrapper.text()).toContain('form.required')
  })

  it('updates its model and emits submitted values', async () => {
    const wrapper = mountForm()
    const input = wrapper.find('input#field-name')
    await input.setValue('Fluffy')
    await wrapper.find('form').trigger('submit')

    const autoForm = wrapper.findComponent(FForm)
    expect(autoForm.emitted('submit')).toEqual([[{ name: 'Fluffy', hidden: '' }]])
    expect(autoForm.emitted('update:modelValue')).toEqual([[{ name: 'Fluffy', hidden: '' }]])
  })
})
