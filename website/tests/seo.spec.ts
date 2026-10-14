import { test, expect } from '@playwright/test'
import { createSeoHead, resolveSiteUrl } from '../docs/.vitepress/seo'

test('localized SEO uses canonical paths and language alternates under a base path', () => {
  const head = createSeoHead({ relativePath: 'en/research/decals.md', title: 'Decals', description: 'Rendering research', frontmatter: { research: true } }, '/openscp/', 'https://example.org')
  expect(head).toContainEqual(['link', { rel: 'canonical', href: 'https://example.org/openscp/en/research/decals' }])
  expect(head).toContainEqual(['link', { rel: 'alternate', hreflang: 'zh-CN', href: 'https://example.org/openscp/research/decals' }])
  expect(head).toContainEqual(['meta', { property: 'og:locale', content: 'en_US' }])
  expect(head).toContainEqual(['meta', { property: 'og:type', content: 'article' }])
  const schema = head.find(tag => tag[0] === 'script')!
  expect(JSON.parse(schema[2]!).inLanguage).toBe('en-US')
})

test('unconfigured and preview deployments avoid indexing and fabricated canonical URLs', () => {
  const page = { relativePath: 'index.md', title: 'Open', description: '', frontmatter: {} }
  const unconfigured = createSeoHead(page, '/')
  expect(unconfigured.some(tag => tag[1].rel === 'canonical')).toBe(false)
  expect(unconfigured).toContainEqual(['meta', { name: 'robots', content: 'noindex, nofollow' }])
  expect(createSeoHead(page, '/', 'https://example.org', true)).toContainEqual(['meta', { name: 'robots', content: 'noindex, nofollow' }])
  expect(() => resolveSiteUrl('https://example.org/subpath/')).toThrow()
})

test('built page includes search and social metadata', async ({ page }) => {
  await page.goto('/en/research/decals')
  await expect(page.locator('head meta[name="description"]')).toHaveAttribute('content', /Property dictionaries/)
  await expect(page.locator('head meta[property="og:title"]')).toHaveAttribute('content', 'How a decal reaches a building | OpenSCP')
  await expect(page.locator('head meta[property="og:locale"]')).toHaveAttribute('content', 'en_US')
  await expect(page.locator('head meta[name="twitter:card"]')).toHaveAttribute('content', 'summary')
})
