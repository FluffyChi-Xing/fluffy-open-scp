import type { InjectionKey, Ref } from 'vue'

export interface PermissionContext {
  tokens: Readonly<Ref<readonly string[]>>
  setTokens: (tokens: readonly string[]) => void
  has: (token: string) => boolean
  hasAny: (tokens: readonly string[]) => boolean
}

export const permissionContextKey: InjectionKey<PermissionContext> = Symbol('fluffy-permission-context')
