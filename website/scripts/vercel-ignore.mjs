import { execFileSync } from 'node:child_process'

// Vercel runs this command from the configured Root Directory (website).
// Exit 0 skips a deployment; exit 1 keeps the website build enabled.
const previous = process.env.VERCEL_GIT_PREVIOUS_SHA
const current = process.env.VERCEL_GIT_COMMIT_SHA || 'HEAD'

if (!previous) {
  process.exit(1)
}

try {
  execFileSync('git', ['diff', '--quiet', previous, current, '--', '.'], { stdio: 'ignore' })
  process.exit(0)
} catch (error) {
  if (error?.status === 1) process.exit(1)
  process.exit(1)
}
