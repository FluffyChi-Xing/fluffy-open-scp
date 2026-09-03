import { describe, expect, it } from 'vitest'
import { useLoading } from '@/composables/useLoading'

describe('useLoading', () => {
  it('stays loading until concurrent tasks finish', async () => {
    const loading = useLoading()
    let finishFirst!: () => void
    let finishSecond!: () => void
    const first = loading.run(() => new Promise<void>((resolve) => { finishFirst = resolve }))
    const second = loading.run(() => new Promise<void>((resolve) => { finishSecond = resolve }))

    expect(loading.loading.value).toBe(true)
    finishFirst()
    await first
    expect(loading.loading.value).toBe(true)
    finishSecond()
    await second
    expect(loading.loading.value).toBe(false)
  })

  it('clears loading after an error', async () => {
    const loading = useLoading()
    await expect(loading.run(async () => { throw new Error('failed') })).rejects.toThrow('failed')
    expect(loading.loading.value).toBe(false)
  })
})
