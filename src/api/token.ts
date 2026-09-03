const accessTokenKey = 'fluffy_access_token'

function getStorage(): Storage | undefined {
  try {
    return typeof window === 'undefined' ? undefined : window.localStorage
  } catch {
    return undefined
  }
}

export function getAccessToken(): string | null {
  try {
    return getStorage()?.getItem(accessTokenKey) ?? null
  } catch {
    return null
  }
}

export function setAccessToken(token: string): void {
  try {
    getStorage()?.setItem(accessTokenKey, token)
  } catch {}
}

export function clearAccessToken(): void {
  try {
    getStorage()?.removeItem(accessTokenKey)
  } catch {}
}
