import { watch } from 'vue'
import type { Directive, DirectiveBinding, WatchStopHandle } from 'vue'
import type { PermissionContext } from '@/permissions/types'

type PermissionBinding = string | readonly string[]
interface OriginalAttributes { hidden: boolean; ariaHidden: string | null; inert: boolean }
const originalAttributes = new WeakMap<HTMLElement, OriginalAttributes>()
const permissionWatchers = new WeakMap<HTMLElement, WatchStopHandle>()

function requiredTokens(value: PermissionBinding, separator: string): string[] {
  const tokens = typeof value === 'string' ? value.split(separator) : value
  return tokens.map((token: string) => token.trim()).filter(Boolean)
}

function updateElement(el: HTMLElement, binding: DirectiveBinding<PermissionBinding>, context: PermissionContext, separator: string) {
  if (!originalAttributes.has(el)) originalAttributes.set(el, { hidden: el.hidden, ariaHidden: el.getAttribute('aria-hidden'), inert: el.inert })
  const required = requiredTokens(binding.value, separator)
  const permitted = required.length > 0 && context.hasAny(required)
  const original = originalAttributes.get(el)!
  el.hidden = permitted ? original.hidden : true
  el.inert = permitted ? original.inert : true
  if (permitted) {
    if (original.ariaHidden === null) el.removeAttribute('aria-hidden')
    else el.setAttribute('aria-hidden', original.ariaHidden)
  } else el.setAttribute('aria-hidden', 'true')
}

export function createPermissionDirective(context: PermissionContext, separator: string): Directive<HTMLElement, PermissionBinding> {
  return {
    mounted: (el, binding) => {
      updateElement(el, binding, context, separator)
      permissionWatchers.set(el, watch(context.tokens, () => updateElement(el, binding, context, separator)))
    },
    updated: (el, binding) => updateElement(el, binding, context, separator),
    unmounted: (el) => {
      permissionWatchers.get(el)?.()
      permissionWatchers.delete(el)
      originalAttributes.delete(el)
    }
  }
}
