import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:4175', headless: true, channel: process.env.PW_CHANNEL || undefined },
  webServer: { command: 'npm run preview -- --port 4175', url: 'http://127.0.0.1:4175', reuseExistingServer: !process.env.CI },
  reporter: 'list'
})
