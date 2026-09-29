export function shuffleTracks<T>(tracks: readonly T[]): T[] {
  const shuffled = [...tracks]
  for (let index = shuffled.length - 1; index > 0; index--) {
    const next = Math.floor(Math.random() * (index + 1))
    ;[shuffled[index], shuffled[next]] = [shuffled[next], shuffled[index]]
  }
  return shuffled
}
