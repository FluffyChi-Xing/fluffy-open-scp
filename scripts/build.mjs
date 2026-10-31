import { mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const projectRoot = resolve(import.meta.dirname, '..')
const buildTemp = resolve(projectRoot, 'tmp', 'esbuild-temp')

mkdirSync(buildTemp, { recursive: true })

// esbuild creates and removes short-lived worker directories under the
// process temp directory. On some Windows installations the user TEMP path
// is readable/writable but denies deletion, which makes esbuild fail after a
// successful transform. Keep build-only temp files in the project workspace.
const env = {
  ...process.env,
  TEMP: buildTemp,
  TMP: buildTemp,
  TMPDIR: buildTemp
}

// This cache is disposable. A locked file should never prevent the build
// from starting; esbuild will recreate anything it needs.
try {
  rmSync(resolve(projectRoot, 'node_modules', '.tmp'), { recursive: true, force: true })
} catch {
  // Ignore Windows sharing violations in the disposable cache.
}

const isWindows = process.platform === 'win32'
const pnpm = isWindows ? 'pnpm' : 'pnpm'

for (const args of [
  ['exec', 'vue-tsc', '-b'],
  ['exec', 'vite', 'build']
]) {
  const command = isWindows ? process.env.ComSpec : pnpm
  const commandArgs = isWindows
    ? ['/d', '/s', '/c', [pnpm, ...args].join(' ')]
    : args
  const result = spawnSync(command, commandArgs, {
    cwd: projectRoot,
    env,
    stdio: 'inherit'
  })
  if (result.error) {
    console.error(result.error)
    process.exit(1)
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}
