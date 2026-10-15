import type { HeadConfig, PageData } from 'vitepress'

export function resolveSiteUrl(value?: string): string | undefined {
  if (!value?.trim()) return undefined
  const url = new URL(value)
  if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error('SITE_URL must be a public HTTP(S) origin, such as https://openscp.example; configure a subpath with SITE_BASE.')
  }
  return url.origin
}

export function createSeoHead(page: Pick<PageData, 'relativePath' | 'title' | 'description' | 'frontmatter'>, base: string, siteUrl?: string, preview = false): HeadConfig[] {
  const english = page.relativePath.startsWith('en/')
  const locale = english ? 'en_US' : 'zh_CN'
  const description = page.description || (english
    ? 'OpenSCP: an open-source SimCity (2013) asset browser and modding toolchain. Explore usage guides, development progress and engine research.'
    : 'OpenSCP：面向 SimCity (2013) 的开源资源浏览器与模组工具链，提供使用指南、开发进度和游戏引擎研究档案。')
  const title = page.title ? `${page.title} | OpenSCP` : 'OpenSCP'
  const missing = /(^|\/)404\.md$/.test(page.relativePath)
  const head: HeadConfig[] = [
    ['meta', { name: 'robots', content: preview || !siteUrl || missing ? 'noindex, nofollow' : 'index, follow, max-image-preview:large' }],
    ['meta', { property: 'og:site_name', content: 'OpenSCP' }],
    ['meta', { property: 'og:title', content: title }],
    ['meta', { property: 'og:description', content: description }],
    ['meta', { property: 'og:type', content: page.frontmatter.research ? 'article' : 'website' }],
    ['meta', { property: 'og:locale', content: locale }],
    ['meta', { property: 'og:locale:alternate', content: english ? 'zh_CN' : 'en_US' }],
    ['meta', { name: 'twitter:card', content: 'summary' }],
    ['meta', { name: 'twitter:title', content: title }],
    ['meta', { name: 'twitter:description', content: description }]
  ]
  if (!siteUrl || missing) return head
  const path = page.relativePath.replace(/index\.md$/, '').replace(/\.md$/, '')
  const neutralPath = path.replace(/^en\//, '')
  const canonical = `${siteUrl}${base}${path}`
  const image = `${siteUrl}${base}images/logo.png`
  head.push(
    ['link', { rel: 'canonical', href: canonical }],
    ['link', { rel: 'alternate', hreflang: 'zh-CN', href: `${siteUrl}${base}${neutralPath}` }],
    ['link', { rel: 'alternate', hreflang: 'en-US', href: `${siteUrl}${base}en/${neutralPath}` }],
    ['link', { rel: 'alternate', hreflang: 'x-default', href: `${siteUrl}${base}${neutralPath}` }],
    ['meta', { property: 'og:url', content: canonical }],
    ['meta', { property: 'og:image', content: image }],
    ['meta', { property: 'og:image:alt', content: 'OpenSCP' }],
    ['meta', { name: 'twitter:image', content: image }],
    ['meta', { name: 'twitter:image:alt', content: 'OpenSCP' }],
    ['script', { type: 'application/ld+json' }, JSON.stringify({
      '@context': 'https://schema.org', '@type': 'WebPage', name: title,
      description, url: canonical, inLanguage: english ? 'en-US' : 'zh-CN',
      isPartOf: { '@type': 'WebSite', name: 'OpenSCP', url: `${siteUrl}${base}` }
    }).replace(/</g, '\\u003c')]
  )
  return head
}
