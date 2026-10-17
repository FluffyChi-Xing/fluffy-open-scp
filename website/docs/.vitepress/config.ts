import { defineConfig } from 'vitepress'
import tailwindcss from '@tailwindcss/vite'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { createSeoHead, resolveSiteUrl } from './seo'

// Set SITE_BASE=/openscp/ when publishing beneath a domain subdirectory.
const base = process.env.SITE_BASE || '/'
if (!base.startsWith('/') || !base.endsWith('/')) throw new Error('SITE_BASE must start and end with /')
const siteUrl = resolveSiteUrl(process.env.SITE_URL)
const preview = process.env.VERCEL_ENV === 'preview'
const verification = [
  ['google-site-verification', process.env.GOOGLE_SITE_VERIFICATION || 'yU8udrf9pd_cGdqLAWpMi9Y6NdGqwTBd8MU9w8C1SRE'],
  ['msvalidate.01', process.env.BING_SITE_VERIFICATION || '6C718B6C7AB6FA176E4E482759901E92']
] as const
export default defineConfig({
  title: 'OpenSCP',
  description: '探索 SimCity (2013) 的资源、渲染与模组制作。',
  base,
  cleanUrls: true,
  srcExclude: ['public/**'],
  appearance: true,
  head: [
    ['link', { rel: 'icon', type: 'image/png', href: `${base}images/logo.png` }],
    ['meta', { name: 'theme-color', content: '#f6f9fc', media: '(prefers-color-scheme: light)' }],
    ['meta', { name: 'theme-color', content: '#0a1422', media: '(prefers-color-scheme: dark)' }],
    ...verification.filter(([, value]) => value).map(([name, content]) => ['meta', { name, content: content! }] as ['meta', { name: string; content: string }])
  ],
  transformHead: ({ pageData }) => createSeoHead(pageData, base, siteUrl, preview),
  sitemap: siteUrl && !preview ? { hostname: `${siteUrl}${base}` } : undefined,
  async buildEnd(config) {
    const robots = siteUrl && !preview
      ? `User-agent: *\nAllow: /\nSitemap: ${siteUrl}${base}sitemap.xml\n`
      : 'User-agent: *\nDisallow: /\n'
    await writeFile(join(config.outDir, 'robots.txt'), robots)
  },
  locales: {
    root: { label: '简体中文', lang: 'zh-CN', title: 'OpenSCP', description: '探索 SimCity (2013) 的资源、渲染与模组制作。' },
    en: { label: 'English', lang: 'en-US', title: 'OpenSCP', description: 'Explore the assets, rendering and modding of SimCity (2013).' }
  },
  vite: { plugins: [tailwindcss()] },
  markdown: { anchor: { permalink: () => {} }, headers: { level: [2, 3] } }
})
