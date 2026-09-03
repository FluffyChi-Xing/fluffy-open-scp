import { inject, readonly, ref } from 'vue'
import type { Ref } from 'vue'
import { permissionContextKey, type PermissionContext } from './types'

export function createPermissionContext(initialTokens: readonly string[] = []): PermissionContext {
  const tokens = ref([...initialTokens]) as Ref<string[]>
  function setTokens(nextTokens: readonly string[]) { tokens.value = [...new Set(nextTokens)] }
  function has(token: string) { return tokens.value.includes(token) }
  function hasAny(requiredTokens: readonly string[]) { return requiredTokens.some(has) }
  return { tokens: readonly(tokens), setTokens, has, hasAny }
}

export function usePermission(): PermissionContext {
  const context = inject(permissionContextKey)
  if (!context) throw new Error('Permission plugin is not installed')
  return context
}
