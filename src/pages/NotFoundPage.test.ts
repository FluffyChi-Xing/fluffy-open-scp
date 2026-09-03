import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

const back = vi.fn()

vi.mock('vue-router', () => ({
  useRoute: () => ({ fullPath: '/missing-path?source=test' }),
  useRouter: () => ({ back })
}))

import NotFoundPage from '@/pages/NotFoundPage.vue'

describe('NotFoundPage', () => {
  it('renders the missing route and allows returning home', () => {
    const wrapper = mount(NotFoundPage, {
      global: {
        mocks: { $t: (key: string) => key },
        stubs: { RouterLink: { template: '<a><slot /></a>' } }
      }
    })

    expect(wrapper.text()).toContain('notFound.title')
    expect(wrapper.find('code').text()).toBe('/missing-path?source=test')
    expect(wrapper.find('a').text()).toContain('notFound.action')
  })
})
