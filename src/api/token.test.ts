import { afterEach, describe, expect, it, vi } from 'vitest'
import { clearAccessToken, getAccessToken, setAccessToken } from './token'

afterEach(() => {
  vi.restoreAllMocks()
  window.localStorage.clear()
})

describe('access token storage', () => {
  it('stores, reads, and clears the access token', () => {
    expect(getAccessToken()).toBeNull()

    setAccessToken('token-value')
    expect(getAccessToken()).toBe('token-value')

    clearAccessToken()
    expect(getAccessToken()).toBeNull()
  })

  it('does not throw when storage is unavailable', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new Error('Storage is unavailable')
    })

    expect(getAccessToken()).toBeNull()
    expect(() => setAccessToken('token-value')).not.toThrow()
    expect(() => clearAccessToken()).not.toThrow()
  })
})
