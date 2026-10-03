export function releaseNotesUrl(version: string, english: boolean) {
  const url = new URL(english ? '/en/changes.html' : '/changes.html', 'https://fastcloud.comf.workers.dev')
  if (/^\d+\.\d+\.\d+(?:-?[a-z])?$/.test(version)) {
    url.hash = `release-v${version.replace(/-([a-z])$/, '$1')}`
  }
  return url.href
}
