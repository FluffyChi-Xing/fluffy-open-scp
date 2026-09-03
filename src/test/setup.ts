import { vi } from 'vitest'

class ResizeObserverMock {
  observe() {}
  disconnect() {}
  unobserve() {}
}

globalThis.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver

vi.mock('shiki', () => ({
  codeToHtml: vi.fn(async (code: string) => `<pre class="shiki"><code>${code}</code></pre>`)
}))
