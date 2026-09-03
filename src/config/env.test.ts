import { afterEach, describe, expect, it, vi } from 'vitest'
import { readAppEnv } from '@/config/env'

describe('readAppEnv', () => {
  afterEach(() => vi.unstubAllEnvs())

  it('reads configured env keys', () => {
    vi.stubEnv('VITE_APP_TITLE', 'Console')
    vi.stubEnv('VITE_APP_ENV', 'production')
    vi.stubEnv('VITE_FLUFFY_OSS_BASE_URL', 'https://oss.example.com/api')
    vi.stubEnv('VITE_FLUFFY_OSS_APP_ID', 'oss-app')
    vi.stubEnv('VITE_FLUFFY_OSS_SECRET', 'sk_123')
    vi.stubEnv('VITE_FLUFFY_OSS_PROXY_TARGET', 'http://localhost:3100')
    vi.stubEnv('VITE_FLUFFY_LOG_APP_ID', 'log-app')
    vi.stubEnv('VITE_FLUFFY_LOG_BASE_URL', 'https://logs.example.com/api/v1')
    vi.stubEnv('VITE_FLUFFY_LOG_CREDENTIAL', 'flt_pub_123')
    vi.stubEnv('VITE_FLUFFY_LOG_PROXY_TARGET', 'http://localhost:3500')

    const env = readAppEnv()
    expect(env.title).toBe('Console')
    expect(env.env).toBe('production')
    expect(env.oss.baseUrl).toBe('https://oss.example.com/api')
    expect(env.oss.appId).toBe('oss-app')
    expect(env.oss.secret).toBe('sk_123')
    expect(env.oss.proxyTarget).toBe('http://localhost:3100')
    expect(env.logTrace.appId).toBe('log-app')
    expect(env.logTrace.baseUrl).toBe('https://logs.example.com/api/v1')
    expect(env.logTrace.credential).toBe('flt_pub_123')
    expect(env.logTrace.proxyTarget).toBe('http://localhost:3500')
  })

  it('falls back to empty defaults', () => {
    const env = readAppEnv()
    expect(env.title).toBe('')
    expect(env.env).toBe('development')
    expect(env.oss.baseUrl).toBe('')
    expect(env.oss.appId).toBe('')
    expect(env.oss.secret).toBe('')
    expect(env.oss.proxyTarget).toBe('')
    expect(env.logTrace.appId).toBe('')
    expect(env.logTrace.baseUrl).toBe('')
    expect(env.logTrace.credential).toBe('')
    expect(env.logTrace.proxyTarget).toBe('')
  })
})
