export const UPDATE_CHECK_INTERVAL = 30 * 60_000
const RESUME_CHECK_INTERVAL = 10 * 60_000

export function releaseIsNewer(candidate: string, installed: string) {
  const parts = (value: string) => value.match(/^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(?:-([a-z]))?$/)
  const next = parts(candidate)
  const current = parts(installed)
  if (!next || !current) return false
  for (let index = 1; index <= 3; index++) {
    if (Number(next[index]) !== Number(current[index])) return Number(next[index]) > Number(current[index])
  }
  if (!next[4]) return !!current[4]
  return !!current[4] && next[4] > current[4]
}

export const displayReleaseVersion = (version: string) => version.replace(/-([a-z])$/, '$1')

// Keep checking while the app is open, including after a tray or network resume.
export function startUpdatePolling(
  checkNow: () => void,
  targetWindow: Pick<Window, 'setInterval' | 'clearInterval' | 'addEventListener' | 'removeEventListener'> = window,
  targetDocument: Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'> = document,
) {
  let lastCheckedAt = 0
  const check = () => {
    lastCheckedAt = Date.now()
    checkNow()
  }
  const resume = () => {
    if (targetDocument.visibilityState === 'visible' && Date.now() - lastCheckedAt >= RESUME_CHECK_INTERVAL) check()
  }
  check()
  const timer = targetWindow.setInterval(check, UPDATE_CHECK_INTERVAL)
  targetWindow.addEventListener('focus', resume)
  targetWindow.addEventListener('online', check)
  targetDocument.addEventListener('visibilitychange', resume)
  return {
    resume,
    published: check,
    stop: () => {
      targetWindow.clearInterval(timer)
      targetWindow.removeEventListener('focus', resume)
      targetWindow.removeEventListener('online', check)
      targetDocument.removeEventListener('visibilitychange', resume)
    },
  }
}
