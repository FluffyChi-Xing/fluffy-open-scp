import { test, expect } from '@playwright/test'

test('theme follows system until overridden and persists across routes and languages', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' })
  await page.goto('/')
  await expect(page.locator('html')).toHaveClass(/dark/)
  await expect(page.getByRole('switch', { name: '夜间模式' })).toBeChecked()
  await page.emulateMedia({ colorScheme: 'light' })
  await expect(page.locator('html')).not.toHaveClass(/dark/)
  await page.getByRole('switch', { name: '夜间模式' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.reload()
  await expect(page.getByRole('switch', { name: '夜间模式' })).toBeChecked()
  await page.getByRole('link', { name: 'Switch to English' }).click()
  await expect(page.getByRole('switch', { name: 'Dark mode' })).toBeChecked()
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.screenshot({ path: 'artifacts/dark-home.png', fullPage: true })
  await page.goto('/en/research/decals')
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.screenshot({ path: 'artifacts/dark-article.png', fullPage: true })
  await page.getByRole('switch', { name: 'Dark mode' }).focus()
  await page.keyboard.press('Space')
  await expect(page.locator('html')).not.toHaveClass(/dark/)
  await page.reload()
  await expect(page.getByRole('switch', { name: 'Dark mode' })).not.toBeChecked()
})

test('dark mode switch fits mobile and works without WebGL', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 })
  await page.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext
    HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, type: string, ...args: unknown[]) {
      return type === 'webgl' ? null : original.call(this, type, ...args)
    } as typeof original
  })
  await page.goto('/')
  await page.getByRole('switch', { name: '夜间模式' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.getByRole('button', { name: '导航菜单' }).click()
  await expect(page.locator('#mobile-nav')).toBeVisible()
  await page.screenshot({ path: 'artifacts/dark-mobile-menu.png', fullPage: false })
})

test('language switch preserves article and updates active UI', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/research/decals')
  await expect(page.locator('h1')).toHaveText('一张贴花，如何抵达建筑表面')
  await page.getByRole('link', { name: 'Switch to English' }).click()
  await expect(page).toHaveURL(/\/en\/research\/decals$/)
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US')
  await expect(page.locator('h1')).toHaveText('How a decal reaches a building')
  await expect(page.locator('.desktop-nav')).toContainText('Development')
  await page.getByRole('link', { name: '切换为简体中文' }).click()
  await expect(page.locator('h1')).toHaveText('一张贴花，如何抵达建筑表面')
  expect(errors).toEqual([])
})

test('workbench tabs support pointer and keyboard navigation', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('link', { name: '加入 Discord 社群' })).toHaveAttribute('href', 'https://discord.gg/VuRDH8px2')
  await page.getByRole('tab', { name: '地图工作台' }).click()
  await expect(page.getByRole('tabpanel')).toContainText('从区域全景')
  await page.getByRole('tab', { name: '地图工作台' }).press('ArrowRight')
  await expect(page.getByRole('tab', { name: 'Raster 绘制' })).toBeFocused()
  await expect(page.getByRole('tabpanel')).toContainText('可编辑的画布')
  await page.getByRole('link', { name: 'Switch to English' }).click()
  await expect(page.getByRole('tab', { name: 'Asset browser' })).toBeVisible()
  await expect(page.locator('h1')).toContainText('Open the assets.')
  await expect(page.getByRole('link', { name: 'Join our Discord' })).toHaveAttribute('href', 'https://discord.gg/VuRDH8px2')
  await page.getByRole('tab', { name: 'Models & materials' }).click()
  await expect(page.getByRole('tabpanel').locator('img')).toHaveAttribute('src', '/images/en-US/rendering.png')
  await page.getByRole('tab', { name: 'Asset browser' }).press('End')
  await expect(page.getByRole('tab', { name: 'Resource overview' })).toBeFocused()
  await expect(page.getByRole('tabpanel').locator('img')).toHaveAttribute('src', '/images/en-US/resource-overview.png')
  await page.getByRole('link', { name: '切换为简体中文' }).click()
  await page.getByRole('tab', { name: '模型与材质' }).click()
  await expect(page.getByRole('tabpanel').locator('img')).toHaveAttribute('src', '/images/zh-CN/rendering.png')
  await page.getByRole('tab', { name: '资源总览' }).click()
  await expect(page.getByRole('tabpanel').locator('img')).toHaveAttribute('src', '/images/zh-CN/resource-overview.png')
})

test('mobile navigation and layout', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/')
  await expect(page.locator('.work-tabs')).toHaveCSS('scrollbar-width', 'none')
  await page.getByRole('tab', { name: '资源浏览' }).focus()
  await page.keyboard.press('End')
  await expect(page.getByRole('tab', { name: '资源总览' })).toBeFocused()
  expect(await page.locator('.work-tabs').evaluate(el => el.scrollLeft)).toBeGreaterThan(0)
  await page.getByRole('button', { name: '导航菜单' }).click()
  await page.locator('#mobile-nav').getByRole('link', { name: '使用指南' }).click()
  await expect(page.locator('h1')).toHaveText('从第一个 .package 开始')
  await expect(page.locator('#mobile-nav')).toHaveCount(0)
  for (const path of ['/', '/en/', '/guide/workbench', '/en/research/', '/development']) {
    await page.goto(path)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  }
  await page.goto('/')
  await page.screenshot({ path: 'artifacts/mobile-zh.png', fullPage: true })
})

test('all pages, local links and images resolve', async ({ page, request }) => {
  const paths = ['', 'about', 'guide/getting-started', 'guide/workbench', 'development', 'research/', 'research/glassbox', 'research/decals', 'research/formats']
  const targets = new Set<string>()
  for (const locale of ['', 'en/']) {
    for (const path of paths) {
      const response = await page.goto(`/${locale}${path}`)
      expect(response?.status()).toBe(200)
      await expect(page.locator('h1')).toHaveCount(1)
      await expect(page.locator('html')).toHaveAttribute('lang', locale ? 'en-US' : 'zh-CN')
      const urls = await page.locator('a[href], img[src]').evaluateAll(elements => elements.map(element => element.getAttribute('href') || element.getAttribute('src') || '').filter(url => url.startsWith('/') && !url.startsWith('//')))
      urls.forEach(url => targets.add(url.split('#')[0]))
      expect(await page.locator('img').evaluateAll(images => images.every(img => (img as HTMLImageElement).complete && (img as HTMLImageElement).naturalWidth > 0))).toBe(true)
    }
  }
  for (const target of targets) expect((await request.get(target)).status(), target).toBe(200)
})

test('desktop visual review and reduced motion', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.emulateMedia({ reducedMotion: 'reduce' })
  for (const [path, file] of [['/', 'desktop-zh'], ['/en/', 'desktop-en']]) {
    await page.goto(path)
    await page.evaluate(() => document.fonts.ready)
    await page.screenshot({ path: `artifacts/${file}.png`, fullPage: true })
    await expect(page.locator('canvas')).toBeVisible()
  }
  await page.goto('/en/research/decals')
  await page.screenshot({ path: 'artifacts/article-en.png', fullPage: true })
})
