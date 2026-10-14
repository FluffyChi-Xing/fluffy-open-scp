import { computed } from 'vue'
import { useData, withBase } from 'vitepress'
import { messages } from '../messages'

export function useLocale() {
  const { lang, page } = useData()
  const english = computed(() => lang.value.startsWith('en'))
  const t = computed(() => messages[english.value ? 'en' : 'zh'])
  const link = (path = '') => withBase(`/${english.value ? 'en/' : ''}${path}`)
  const alternate = computed(() => {
    const path = page.value.relativePath.replace(/^en\//, '').replace(/index\.md$/, '').replace(/\.md$/, '')
    return withBase(`/${english.value ? '' : 'en/'}${path}`)
  })
  return { t, link, alternate, english }
}
