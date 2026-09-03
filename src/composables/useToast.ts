import { readonly, shallowRef } from 'vue'

export type ToastTone = 'success' | 'info' | 'warning' | 'error'

export interface ToastOptions {
  duration?: number
  title?: string
}

export interface ToastItem {
  id: number
  message: string
  title?: string
  tone: ToastTone
  duration: number
}

const toastItems = shallowRef<ToastItem[]>([])
let nextToastId = 1

function push(tone: ToastTone, message: string, options: ToastOptions = {}) {
  const item: ToastItem = {
    id: nextToastId++,
    message,
    title: options.title,
    tone,
    duration: options.duration ?? 4000
  }
  toastItems.value = [...toastItems.value, item]
  return item.id
}

function dismiss(id: number) {
  toastItems.value = toastItems.value.filter((item) => item.id !== id)
}

function clear() {
  toastItems.value = []
}

export function useToast() {
  return {
    toasts: readonly(toastItems),
    success: (message: string, options?: ToastOptions) => push('success', message, options),
    info: (message: string, options?: ToastOptions) => push('info', message, options),
    warning: (message: string, options?: ToastOptions) => push('warning', message, options),
    error: (message: string, options?: ToastOptions) => push('error', message, options),
    dismiss,
    clear
  }
}
