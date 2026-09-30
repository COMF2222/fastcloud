import { genreKey, trackGenre } from './genres'
import { creditedArtists, type Track } from './types'

const genresOf = (track: Track) => (trackGenre(track) || '').split(/\s*[,/|;]\s*/)
  .map(genreKey).filter(Boolean)
const artistsOf = (track: Track) => creditedArtists(track)
  .map(name => name.normalize('NFKC').trim().toLocaleLowerCase()).filter(name => name !== '—')

export function favouriteGenres(likes: Track[], choices: readonly string[], day: number): string[] {
  const counts = new Map<string, number>()
  for (const track of likes) for (const genre of new Set(genresOf(track))) {
    counts.set(genre, (counts.get(genre) || 0) + 1)
  }
  const ranked = choices.filter(genre => counts.has(genreKey(genre)))
    .sort((a, b) => (counts.get(genreKey(b)) || 0) - (counts.get(genreKey(a)) || 0))
  if (!ranked.length) return [choices[day % choices.length], choices[(day + 1) % choices.length]]
  if (ranked.length === 1) return ranked
  return [ranked[0], ranked[1 + day % Math.min(3, ranked.length - 1)]]
}

function jitter(id: number, seed: number): number {
  let value = (id ^ seed) >>> 0
  value = Math.imul(value ^ (value >>> 16), 0x7feb352d)
  value = Math.imul(value ^ (value >>> 15), 0x846ca68b)
  return ((value ^ (value >>> 16)) >>> 0) / 0x100000000
}

export function searchRecommendations(likes: Track[], candidates: Track[], recent: Track[], seed: number, limit = 16): Track[] {
  const pool = [...new Map(candidates.filter(track => !['block', 'blocked', 'snip', 'preview']
    .includes((track.access || track.policy || 'allow').toLocaleLowerCase()))
    .map(track => [track.id, track])).values()]
  if (!likes.length) return pool.sort((a, b) => jitter(a.id, seed) - jitter(b.id, seed)).slice(0, limit)

  const likedIds = new Set(likes.map(track => track.id))
  const recentIds = new Set(recent.slice(0, 20).map(track => track.id))
  const genreCounts = new Map<string, number>()
  const artistCounts = new Map<string, number>()
  for (const track of likes) {
    for (const genre of new Set(genresOf(track))) genreCounts.set(genre, (genreCounts.get(genre) || 0) + 1)
    for (const name of new Set(artistsOf(track))) artistCounts.set(name, (artistCounts.get(name) || 0) + 1)
  }

  const ranked = pool.filter(track => !likedIds.has(track.id)).map(track => {
    const genres = genresOf(track)
    const artists = artistsOf(track)
    const genreAffinity = Math.max(0, ...genres.map(genre => genreCounts.get(genre) || 0))
    const artistAffinity = Math.max(0, ...artists.map(name => artistCounts.get(name) || 0))
    return { track, genres, artists, score: Math.log1p(genreAffinity) * 4 + Math.log1p(artistAffinity) * 6
      - (recentIds.has(track.id) ? 20 : 0) + jitter(track.id, seed) * 0.2 }
  })
  const selected: Track[] = []
  const shownGenres = new Map<string, number>()
  const shownArtists = new Map<string, number>()
  while (selected.length < limit && ranked.length) {
    let best = 0
    let bestScore = -Infinity
    for (let index = 0; index < ranked.length; index++) {
      const item = ranked[index]
      const artistRepeats = Math.max(0, ...item.artists.map(name => shownArtists.get(name) || 0))
      const genreRepeats = Math.max(0, ...item.genres.map(genre => shownGenres.get(genre) || 0))
      const score = item.score - artistRepeats * 5 - genreRepeats * 0.7
      if (score > bestScore) { best = index; bestScore = score }
    }
    const item = ranked.splice(best, 1)[0]
    selected.push(item.track)
    for (const genre of item.genres) shownGenres.set(genre, (shownGenres.get(genre) || 0) + 1)
    for (const name of item.artists) shownArtists.set(name, (shownArtists.get(name) || 0) + 1)
  }
  return selected
}
