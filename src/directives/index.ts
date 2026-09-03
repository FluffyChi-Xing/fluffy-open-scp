import type { App, Plugin } from 'vue'
import { createPermissionContext } from '@/permissions/context'
import { permissionContextKey } from '@/permissions/types'
import { createPermissionDirective } from './permission'

export interface PermissionPluginOptions { tokens?: readonly string[]; tokenSeparator?: string }

export function createPermissionPlugin(options: PermissionPluginOptions = {}): Plugin {
  const context = createPermissionContext(options.tokens)
  const separator = options.tokenSeparator ?? '|'
  return {
    install(app: App) {
      app.provide(permissionContextKey, context)
      app.directive('permission', createPermissionDirective(context, separator))
    }
  }
}
