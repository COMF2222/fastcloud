import { libraryCollections } from './types'
import { useEffect, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Heart, MapPin, Music2, Play, Repeat2, Send, Share2, Shuffle } from 'lucide-react'
import { api } from './api'
import { Artwork, DetailActions, Empty, LibraryPlaylists, LibraryTracks, PlaylistCards, SectionTitle, Status, TrackRows, type LibraryView } from './App'
import { useApp } from './store'
import { RemoteImage } from './RemoteImage'
import { shuffleTracks } from './shuffle'
import { GenreCarousel } from './GenreCarousel'
import { releaseGenres, releaseMatchesGenre, sameGenre } from './genres'
import { artist, duration, type Data, type Playlist, type Track, type User } from './types'

function useRemote<T>(key: unknown[], load: () => Promise<T>, enabled = true, staleTime = 0) {
  return useQuery({ queryKey: key, queryFn: load, enabled, staleTime, refetchInterval: result => {
    const status = (result.state.data as { status?: string } | undefined)?.status
    return status === 'loading' || status === 'unavailable' ? 1200 : status === 'failed' ? 6000 : false
  } })
}

function People({ users }: { users: User[] }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  if (!users.length) return <Empty message={english ? 'No artists yet' : 'Авторов пока нет'} />
  return <div className="artist-grid">{users.map(user => <button className="artist-card" key={user.id} onClick={() => useApp.getState().openArtist(user.id, user.username)}>{user.avatar_url ? <RemoteImage className="artist-avatar" src={user.avatar_url} pixels={500} alt="" loading="lazy" /> : <span className="artist-avatar">{user.username.slice(0, 1).toUpperCase()}</span>}<strong>{user.username}</strong><small>{user.followers_count ? `${user.followers_count.toLocaleString(english ? 'en-US' : 'ru-RU')} ${english ? 'followers' : 'подписчиков'}` : english ? 'SoundCloud artist' : 'Автор SoundCloud'}</small></button>)}</div>
}

const isAlbum = (item: Playlist) => item.is_album || item.playlist_type?.toLowerCase() === 'album' || item.set_type?.toLowerCase() === 'album'
const playlistDuration = (ms: number) => {
  const seconds = Math.floor(ms / 1000)
  const hours = Math.floor(seconds / 3600)
  return hours ? `${hours}:${String(Math.floor(seconds / 60) % 60).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}` : duration(ms)
}

function ProfileCollections({ created, saved, albums, filter, english = false }: { created?: Data<Playlist[]>; saved?: Data<Playlist[]>; albums: boolean; filter: string; english?: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const belongs = (item: Playlist) => isAlbum(item) === albums && [item.title, item.user?.username || ''].some(value => value.toLocaleLowerCase().includes(filter))
  return <>
    {saved && (saved.status !== 'ready' || saved.data.some(belongs)) && <><SectionTitle title={albums ? t('Сохранённые альбомы', 'Saved albums') : t('Сохранённые плейлисты', 'Saved playlists')} />
      <Status value={saved}>{items => <PlaylistCards playlists={items.filter(belongs)} />}</Status></>}
    {(created?.status !== 'ready' || created.data.some(belongs)) && <><SectionTitle title={albums ? t('Мои альбомы', 'My albums') : t('Мои плейлисты', 'My playlists')} />
      <Status value={created}>{items => <PlaylistCards playlists={items.filter(belongs)} />}</Status></>}
    {(saved == null || saved.status === 'ready') && created?.status === 'ready' && !(saved?.status === 'ready' && saved.data.some(belongs)) && !created.data.some(belongs) && <Empty message={filter ? t('Ничего не найдено', 'Nothing found') : albums ? t('Альбомов пока нет', 'No albums yet') : t('Плейлистов пока нет', 'No playlists yet')} />}
  </>
}

export function ArtistPage({ id, name }: { id: number; name: string }) {
  const queryClient = useQueryClient()
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const [tab, setTab] = useState<'tracks' | 'popular' | 'albums' | 'playlists' | 'likes' | 'reposts'>('tracks')
  const [filter, setFilter] = useState('')
  useEffect(() => { setTab('tracks'); setFilter('') }, [id])
  const { data: me } = useRemote(['my-profile'], api.myProfile)
  const isOwn = me?.status === 'ready' && me.data.id === id
  const { data: user } = useRemote(['user', id], () => api.user(id))
  const { data: profiles } = useRemote(['user-profiles', id], () => api.userProfiles(id))
  const { data: tracks } = useRemote(isOwn ? ['tracks', 'uploads'] : ['profile-tracks', id], () => api.tracks(isOwn ? 'uploads' : 'artist', undefined, id), tab === 'tracks' || tab === 'popular')
  const { data: likes } = useRemote(isOwn ? ['tracks', 'likes', undefined, undefined] : ['profile-likes', id], () => api.tracks(isOwn ? 'likes' : 'artist_likes', undefined, id), tab === 'likes')
  const { data: reposts } = useRemote(isOwn ? ['tracks', 'reposts'] : ['profile-reposts', id], () => api.tracks(isOwn ? 'reposts' : 'artist_reposts', undefined, id), tab === 'reposts')
  const { data: savedLists } = useRemote(['playlists', 'liked'], () => api.playlists('liked'), isOwn && (tab === 'albums' || tab === 'playlists'))
  const { data: lists } = useRemote(isOwn ? ['playlists', 'mine'] : ['profile-playlists', id], () => api.playlists(isOwn ? 'mine' : 'artist', String(id)), tab === 'albums' || tab === 'playlists')
  const { data: repostedLists } = useRemote(['profile-reposted-playlists', id, isOwn], () => api.playlists(isOwn ? 'reposts' : 'artist_reposts', String(id)), tab === 'reposts')
  const { data: related } = useRemote(['related-users', id], () => api.relatedUsers(id), !isOwn)
  const [error, setError] = useState('')
  const owner = user?.status === 'ready' ? user.data : null
  const username = owner?.username || (isOwn && me?.status === 'ready' ? me.data.username : name)
  const avatar = owner?.avatar_url || (isOwn && me?.status === 'ready' ? me.data.avatar_url : null)
  const trackCount = tracks?.status === 'ready' ? tracks.data.length : owner?.track_count
  const allLists = [...(lists?.status === 'ready' ? lists.data : []), ...(isOwn && savedLists?.status === 'ready' ? savedLists.data : [])]
  const distinctLists = [...new Map(allLists.map(item => [item.id, item])).values()]
  const playlistCount = lists?.status === 'ready' || savedLists?.status === 'ready' ? distinctLists.filter(item => !isAlbum(item)).length : owner?.public_playlists_count
  const albumCount = lists?.status === 'ready' || savedLists?.status === 'ready' ? distinctLists.filter(isAlbum).length : undefined
  const search = filter.trim().toLocaleLowerCase()
  const matchingTracks = (items: Track[]) => search ? items.filter(item => [item.title, artist(item), item.genre || ''].some(value => value.toLocaleLowerCase().includes(search))) : items
  const station = async () => {
    if (tracks?.status !== 'ready' || !tracks.data.length) return
    try { await api.setSetting('autoplay', true); await api.play(tracks.data, 0) } catch (cause) { setError(String(cause)) }
  }
  const playLikesShuffled = async () => {
    if (likes?.status !== 'ready' || !likes.data.length) return
    try { await api.play(shuffleTracks(likes.data), 0); await queryClient.invalidateQueries({ queryKey: ['player'] }) } catch (cause) { setError(String(cause)) }
  }
  return <div className="page-content profile-page"><div className="profile-hero"><div className="profile-avatar"><span>{username.slice(0, 1).toUpperCase()}</span>{avatar && <RemoteImage src={avatar} previewSrc={avatar} pixels={1080} alt={t('Аватар', 'Avatar') + ` ${username}`} />}</div><div className="profile-summary"><span className="page-kicker">FASTCLOUD / {isOwn ? t('МОЙ ПРОФИЛЬ', 'MY PROFILE') : t('АВТОР', 'ARTIST')}</span><h1>{username}</h1>{owner?.full_name && owner.full_name !== username && <p className="profile-full-name">{owner.full_name}</p>}{owner?.description && <p className="profile-description">{owner.description}</p>}{(owner?.city || owner?.country_code) && <p className="profile-location"><MapPin size={14} />{[owner.city, owner.country_code].filter(Boolean).join(', ')}</p>}<div className="profile-stats"><div><strong>{trackCount ?? '—'}</strong><span>{t('треков', 'tracks')}</span></div><div><strong>{playlistCount ?? '—'}</strong><span>{t('плейлистов', 'playlists')}</span></div><div><strong>{owner?.followers_count ?? (isOwn && me?.status === 'ready' ? me.data.followers_count : null) ?? '—'}</strong><span>{t('подписчиков', 'followers')}</span></div></div><div className="profile-actions"><button className="primary-button" disabled={tracks?.status !== 'ready' || !tracks.data.length} onClick={() => void station()}><Play size={16} fill="currentColor" /> {t('Слушать треки', 'Play tracks')}</button>{!isOwn && <DetailActions page="artist" id={id} name={name} />}{owner?.permalink_url && <button className="secondary-button" onClick={() => void api.openSoundCloud(owner.permalink_url!).catch(cause => setError(String(cause)))}>SoundCloud ↗</button>}</div>{error && <p className="error-text">{error}</p>}{profiles?.status === 'ready' && <div className="profile-links">{profiles.data.filter(profile => /^https?:\/\//.test(profile.url)).map(profile => <a href={profile.url} target="_blank" rel="noreferrer" key={profile.url}>{profile.title || profile.service || t('Сайт автора', 'Artist website')}</a>)}</div>}</div></div><div className="profile-content-tools"><div className="tabs profile-tabs">{([['tracks', t('Треки', 'Tracks'), trackCount], ['popular', t('Популярное', 'Popular'), undefined], ['albums', t('Альбомы', 'Albums'), albumCount], ['playlists', t('Плейлисты', 'Playlists'), playlistCount], ['likes', t('Лайки', 'Likes'), undefined], ['reposts', t('Репосты', 'Reposts'), undefined]] as const).map(([value, label, count]) => <button key={value} className={tab === value ? 'active' : ''} onClick={() => { setTab(value); setFilter('') }}>{label}{count != null && <small>{count}</small>}</button>)}</div><input className="profile-filter" aria-label={t('Поиск в профиле', 'Search profile')} placeholder={tab === 'albums' ? t('Найти альбом…', 'Find album…') : tab === 'playlists' ? t('Найти плейлист…', 'Find playlist…') : t('Найти трек…', 'Find track…')} value={filter} onChange={event => setFilter(event.target.value)} /></div>
    {tab === 'likes' && <div className="profile-collection-actions"><button className="secondary-button" disabled={likes?.status !== 'ready' || !likes.data.length} onClick={() => void playLikesShuffled()}><Shuffle size={16} /> {t('Перемешать всё', 'Shuffle all')}</button></div>}
    {tab === 'tracks' && <Status value={tracks}>{items => <TrackRows tracks={matchingTracks(items)} />}</Status>}
    {tab === 'popular' && <Status value={tracks}>{items => <TrackRows tracks={matchingTracks([...items].sort((a, b) => (b.playback_count || 0) - (a.playback_count || 0)))} />}</Status>}
    {tab === 'albums' && <ProfileCollections created={lists} saved={isOwn ? savedLists ?? { status: 'loading' } : undefined} albums filter={search} english={english} />}
    {tab === 'playlists' && <ProfileCollections created={lists} saved={isOwn ? savedLists ?? { status: 'loading' } : undefined} albums={false} filter={search} english={english} />}
    {tab === 'likes' && <Status value={likes}>{items => <TrackRows tracks={matchingTracks(items)} />}</Status>}
    {tab === 'reposts' && <><Status value={reposts}>{items => <TrackRows tracks={matchingTracks(items)} />}</Status><SectionTitle title={t('Репосты плейлистов', 'Reposted playlists')} /><Status value={repostedLists}>{items => <PlaylistCards playlists={items.filter(item => [item.title, item.user?.username || ''].some(value => value.toLocaleLowerCase().includes(search)))} />}</Status></>}
    {!isOwn && <><SectionTitle title={t('Похожие авторы', 'Similar artists')} /><Status value={related}>{items => <People users={items} />}</Status></>}
  </div>
}

export function PlaylistPage({ id, name }: { id: number; name: string }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data: detail } = useRemote(['playlist', id], () => api.playlist(id))
  const { data: ownLists } = useRemote(['playlists', 'mine'], () => api.playlists('mine'))
  const { data: tracks } = useRemote(['tracks', 'playlist', id], () => api.tracks('playlist', undefined, id))
  const { data: liked } = useRemote(['playlists', 'liked'], () => api.playlists('liked'))
  const { data: reposted } = useRemote(['playlists', 'reposts'], () => api.playlists('reposts'))
  const queryClient = useQueryClient()
  const [error, setError] = useState('')
  const owned = ownLists?.status === 'ready' ? ownLists.data.find(list => list.id === id) : undefined
  const saved = liked?.status === 'ready' ? liked.data.find(list => list.id === id) : undefined
  const item = detail?.status === 'ready' ? detail.data : owned || saved || null
  const embedded = item?.tracks || []
  const contents: Data<Track[]> | undefined = embedded.length && (tracks?.status !== 'ready' || !tracks.data.length) ? { status: 'ready', data: embedded } : tracks
  const isLiked = liked?.status === 'ready' && liked.data.some(item => item.id === id)
  const isReposted = reposted?.status === 'ready' && reposted.data.some(item => item.id === id)
  const toggle = async (kind: 'like' | 'repost') => {
    try { if (kind === 'like') await api.likePlaylist(id, !isLiked); else await api.repost('playlist', id, !isReposted); await queryClient.invalidateQueries({ queryKey: ['playlists'] }) } catch (cause) { setError(String(cause)) }
  }
  const play = async () => { if (contents?.status === 'ready' && contents.data.length) { try { await api.play(contents.data, 0); await queryClient.invalidateQueries({ queryKey: ['player'] }) } catch (cause) { setError(String(cause)) } } }
  const playShuffled = async () => {
    if (contents?.status !== 'ready' || !contents.data.length) return
    try { await api.play(shuffleTracks(contents.data), 0); await queryClient.invalidateQueries({ queryKey: ['player'] }) } catch (cause) { setError(String(cause)) }
  }
  const visibleTracks = contents?.status === 'ready' ? contents.data : []
  const count = item?.track_count || visibleTracks.length
  const genres = item ? releaseGenres(item, visibleTracks) : []
  return <div className="page-content detail-page">
    <section className="playlist-hero detail-hero">
      <div className="detail-cover">{item ? <Artwork item={item} size="hero" /> : <div className="art art-hero"><Music2 size={48} strokeWidth={1.4} /></div>}</div>
      <div className="detail-main">
        <span className="detail-kicker">{item && isAlbum(item) ? t('АЛЬБОМ', 'ALBUM') : t('ПЛЕЙЛИСТ', 'PLAYLIST')}</span>
        <h1>{item?.title || name}</h1>
        <div className="detail-genres">{genres.length ? genres.map(genre => <span key={genre}>{genre}</span>) : <span>{t('Жанр не указан', 'Genre not specified')}</span>}</div>
        <div className="detail-chip-row"><span>{count} {t('треков', 'tracks')}</span>{item?.duration_ms ? <span>{playlistDuration(item.duration_ms)}</span> : null}</div>
        <div className="track-detail-buttons">
          <button className="primary-button" disabled={!visibleTracks.length} onClick={() => void play()}><Play size={16} fill="currentColor" /> {t('Воспроизвести', 'Play')}</button>
          <button className="secondary-button" disabled={!visibleTracks.length} onClick={() => void playShuffled()}><Shuffle size={16} /> {t('Перемешать', 'Shuffle')}</button>
          <button className="secondary-button" onClick={() => void toggle('like')}><Heart size={16} fill={isLiked ? 'currentColor' : 'none'} /> {isLiked ? t('Убрать лайк', 'Unlike') : t('Нравится', 'Like')}</button>
          <button className="secondary-button" onClick={() => void toggle('repost')}><Repeat2 size={16} /> {isReposted ? t('Убрать репост', 'Remove repost') : t('Репост', 'Repost')}</button>
          {item?.permalink_url && <button className="secondary-button" onClick={() => void navigator.clipboard.writeText(item.permalink_url!)}><Share2 size={16} /> {t('Ссылка', 'Link')}</button>}
        </div>
        {item?.user && <div className="detail-owner"><span>{t('СОБРАЛ', 'CURATED BY')}</span><button onClick={() => useApp.getState().openArtist(item.user!.id, item.user!.username)}>{item.user.avatar_url ? <RemoteImage src={item.user.avatar_url} pixels={100} alt="" loading="lazy" /> : <span className="detail-owner-fallback">{item.user.username.slice(0, 1).toUpperCase()}</span>}<strong>{item.user.username}</strong></button></div>}
        <DetailActions page="playlist" id={id} name={item?.title || name} />
        {error && <p className="error-text">{error}</p>}
      </div>
    </section>
    <div className="detail-metrics"><span><strong>{count}</strong>{t('треков', 'tracks')}</span>{item?.duration_ms ? <span><strong>{playlistDuration(item.duration_ms)}</strong>{t('длительность', 'duration')}</span> : null}<span><strong>{genres.length}</strong>{t('жанров', 'genres')}</span></div>
    {visibleTracks.length > 0 && <div className="playlist-ribbon" aria-label={t('Треки плейлиста', 'Playlist tracks')}>{visibleTracks.slice(0, 100).map((track, index) => <button key={track.id + '-' + index} title={track.title} aria-label={t('Слушать', 'Play') + ': ' + track.title} style={{ backgroundColor: 'hsl(' + ((index * 47 + 12) % 360) + ' 48% 62%)' }} onClick={() => void api.play(visibleTracks, index).then(() => queryClient.invalidateQueries({ queryKey: ['player'] }))} />)}</div>}
    <section className="detail-content-panel"><SectionTitle title={t('Треки', 'Tracks')} /><Status value={contents}>{items => <TrackRows tracks={items} />}</Status></section>
  </div>
}

export function RepostsPage() {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const { data: tracks } = useRemote(['tracks', 'reposts'], () => api.tracks('reposts'))
  const { data: lists } = useRemote(['playlists', 'reposts'], () => api.playlists('reposts'))
  return <div className="page-content"><SectionTitle title={english ? 'My reposts' : 'Мои репосты'} subtitle={english ? 'Music you shared' : 'Музыка, которой ты поделился'} /><Status value={tracks}>{items => <TrackRows tracks={items} />}</Status><SectionTitle title={english ? 'Playlists' : 'Плейлисты'} /><Status value={lists}>{items => <PlaylistCards playlists={items} />}</Status></div>
}

export function LibraryExtra({ tab, filter, view }: { tab: 'liked_playlists' | 'albums' | 'uploads' | 'stations' | 'history'; filter: string; view: LibraryView }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data: liked } = useRemote(['playlists', 'liked'], () => api.playlists('liked'), tab === 'liked_playlists' || tab === 'albums')
  const { data: mine } = useRemote(['playlists', 'mine'], () => api.playlists('mine'), tab === 'albums')
  const { data: uploads } = useRemote(['tracks', 'uploads'], () => api.tracks('uploads'), tab === 'uploads')
  const { data: history } = useRemote(['tracks', 'history'], () => api.tracks('history'), tab === 'history' || tab === 'stations')
  const { data: likes } = useRemote(['tracks', 'likes', undefined, undefined], () => api.tracks('likes'), tab === 'stations')
  const queryClient = useQueryClient()
  const [error, setError] = useState('')
  const station = async (track: Track) => {
    try { await api.setSetting('autoplay', true); const related = await api.tracks('related', undefined, track.id); await api.play([track, ...(related.status === 'ready' ? related.data : [])], 0); await queryClient.invalidateQueries({ queryKey: ['player'] }) } catch (cause) { setError(String(cause)) }
  }
  if (tab === 'liked_playlists') return <Status value={liked}>{items => <LibraryPlaylists playlists={items.filter(item => !isAlbum(item))} filter={filter} view={view} />}</Status>
  if (tab === 'albums') return <Status value={libraryCollections([liked, mine], true)}>{items => <LibraryPlaylists playlists={items} filter={filter} view={view} />}</Status>
  if (tab === 'uploads') return <><UploadForm /><SectionTitle title={t('Загруженные треки', 'Uploaded tracks')} /><Status value={uploads}>{items => <LibraryTracks tracks={items} filter={filter} view={view} />}</Status></>
  if (tab === 'history') return <Status value={history}>{items => <LibraryTracks tracks={items} filter={filter} view={view} />}</Status>
  const seeds = history?.status === 'ready' && history.data.length ? history.data.slice(0, 8) : likes?.status === 'ready' ? likes.data.slice(0, 8) : []
  return <><p className="muted">{t('Выбери трек: после него будут играть похожие записи.', 'Choose a track; similar music will follow.')}</p>{error && <p className="error-text">{error}</p>}{seeds.length ? <div className="cards">{seeds.map(track => <button className="card" key={track.id} onClick={() => void station(track)}><Artwork item={track} size="card" /><strong>{track.title}</strong><small>{artist(track)} · {t('Станция', 'Station')}</small></button>)}</div> : <Empty message={t('Пока нет отправной точки', 'No starting track yet')} detail={t('Послушай или сохрани трек, чтобы начать станцию.', 'Play or save a track to start a station.')} />}</>
}

export function LibraryOverview() {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data: likes } = useRemote(['tracks', 'likes'], () => api.tracks('likes'))
  const { data: history } = useRemote(['tracks', 'history'], () => api.tracks('history'))
  const { data: lists } = useRemote(['playlists', 'mine'], () => api.playlists('mine'))
  const { data: saved } = useRemote(['playlists', 'liked'], () => api.playlists('liked'))
  const { data: following } = useRemote(['following'], api.following)
  const { data: fresh } = useRemote(['tracks', 'following'], () => api.tracks('following'))
  const { data: me } = useRemote(['my-profile'], api.myProfile)
  const tracks = likes?.status === 'ready' ? likes.data : []
  const genres = Object.entries(tracks.reduce<Record<string, number>>((counts, track) => { const genre = track.genre?.trim(); if (genre) counts[genre] = (counts[genre] || 0) + 1; return counts }, {})).sort((a, b) => b[1] - a[1]).slice(0, 7)
  const total = genres.reduce((sum, [, count]) => sum + count, 0)
  const savedLists = saved?.status === 'ready' ? saved.data : []
  const ownLists = lists?.status === 'ready' ? lists.data : []
  const albums = savedLists.filter(isAlbum)
  const savedPlaylists = savedLists.filter(item => !isAlbum(item))
  const ownPlaylists = ownLists.filter(item => !isAlbum(item))
  const open = (tab: 'tracks' | 'playlists' | 'albums' | 'artists' | 'history') => useApp.getState().setLibraryTab(tab)
  return <>
    {me?.status === 'ready' && <button className="library-identity" onClick={() => useApp.getState().openArtist(me.data.id, me.data.username)}>{me.data.avatar_url ? <RemoteImage src={me.data.avatar_url} pixels={160} alt="" /> : <span>{me.data.username.slice(0, 1).toUpperCase()}</span>}<span><small>{t('Твоя медиатека', 'Your library')}</small><strong>{me.data.username}</strong></span><span className="library-identity-action">{t('Открыть профиль', 'Open profile')} →</span></button>}
    <div className="library-stats"><button onClick={() => open('tracks')}><strong>{likes?.status === 'ready' ? tracks.length : '—'}</strong><span>{t('Любимых треков', 'Liked tracks')}</span></button><button onClick={() => open('playlists')}><strong>{saved?.status === 'ready' && lists?.status === 'ready' ? new Set([...savedPlaylists, ...ownPlaylists].map(item => item.id)).size : '—'}</strong><span>{t('Плейлистов', 'Playlists')}</span></button><button onClick={() => open('albums')}><strong>{saved?.status === 'ready' ? albums.length : '—'}</strong><span>{t('Сохранённых альбомов', 'Saved albums')}</span></button></div>
    {fresh?.status === 'ready' && fresh.data.length > 0 && <><SectionTitle title={t('Новое от подписок', 'From your artists')} subtitle={t('Свежие треки авторов, которых ты слушаешь', 'New tracks from artists you follow')} /><TrackRows tracks={fresh.data.slice(0, 8)} /></>}
    <SectionTitle title={t('Недавно слушал', 'Recently played')} action={t('Вся история', 'History')} onAction={() => open('history')} /><Status value={history}>{items => <TrackRows tracks={items.slice(0, 8)} />}</Status>
    <SectionTitle title={t('Твой саундпринт', 'Your soundprint')} subtitle={t('Жанры любимой музыки', 'Genres in your likes')} />{genres.length ? <><div className="soundprint-lane">{genres.map(([genre, count], index) => <span key={genre} style={{ width: `${100 * count / total}%`, background: `hsl(${18 + index * 34} 75% ${55 + index % 2 * 9}%)` }} title={`${genre}: ${count}`} />)}</div><div className="soundprint-tags">{genres.map(([genre, count]) => <span key={genre}>{genre} <b>{count}</b></span>)}</div></> : likes?.status === 'ready' ? <Empty message={t('Саундпринт появится после лайков', 'Your soundprint will appear after you like tracks')} /> : null}
    {ownPlaylists.length > 0 && <><SectionTitle title={t('Мои плейлисты', 'My playlists')} action={t('Все плейлисты', 'All playlists')} onAction={() => open('playlists')} /><PlaylistCards playlists={ownPlaylists.slice(0, 5)} /></>}
    {savedPlaylists.length > 0 && <><SectionTitle title={t('Сохранённые плейлисты', 'Saved playlists')} action={t('Все плейлисты', 'All playlists')} onAction={() => open('playlists')} /><PlaylistCards playlists={savedPlaylists.slice(0, 5)} /></>}
    {albums.length > 0 && <><SectionTitle title={t('Сохранённые альбомы', 'Saved albums')} action={t('Все альбомы', 'All albums')} onAction={() => open('albums')} /><PlaylistCards playlists={albums.slice(0, 5)} /></>}
    {following?.status === 'ready' && following.data.length > 0 && <><SectionTitle title={t('Подписки', 'Following')} action={t('Все авторы', 'All artists')} onAction={() => open('artists')} /><People users={following.data.slice(0, 10)} /></>}
    {tracks.length > 0 && <><SectionTitle title={t('Любимые треки', 'Liked tracks')} action={t('Все лайки', 'All likes')} onAction={() => open('tracks')} /><TrackRows tracks={tracks.slice(0, 8)} /></>}
  </>
}

export function CatalogPage() {
  const queryClient = useQueryClient()
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const [query, setQuery] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')
  const [genre, setGenre] = useState('')
  const [tab, setTab] = useState<'albums' | 'artists'>('albums')
  const [kind, setKind] = useState<'all' | 'album' | 'ep' | 'single' | 'compilation'>('all')
  const [sort, setSort] = useState<'new' | 'popular' | 'tracks' | 'az'>('new')
  const [featuredError, setFeaturedError] = useState('')
  useEffect(() => { const timer = window.setTimeout(() => setDebouncedQuery(query.trim()), 250); return () => window.clearTimeout(timer) }, [query])
  const [day, setDay] = useState(() => Math.floor(Date.now() / 86400000))
  useEffect(() => { const timer = window.setInterval(() => setDay(Math.floor(Date.now() / 86400000)), 60_000); return () => window.clearInterval(timer) }, [])
  const { data: lists } = useRemote(['catalog-releases', day, debouncedQuery], () => debouncedQuery ? api.playlists('search', debouncedQuery) : api.catalogReleases(day), tab === 'albums', debouncedQuery ? 0 : 10 * 60_000)
  const { data: users } = useRemote(['users', debouncedQuery], () => api.users(debouncedQuery), tab === 'artists' && !!debouncedQuery)
  const { data: tracks } = useRemote(['tracks', 'genre', genre], () => api.tracks('genre', genre), !debouncedQuery && !!genre)
  const genreTracks = tracks?.status === 'ready' ? tracks.data.filter(track => sameGenre(track.genre, genre)) : []
  const searched = lists?.status === 'ready' ? lists.data : []
  const featuredArtists = [...new Map((genre ? genreTracks : searched).filter(item => item.user).map(item => [item.user!.id, { ...item.user!, followers_count: 0 }])).values()].slice(0, 15)
  const isRelease = (item: Playlist) => item.is_album || ['album', 'ep', 'single', 'compilation'].includes((item.set_type || item.playlist_type || '').toLocaleLowerCase())
  const albums = searched.filter(item => isRelease(item) && (!genre || debouncedQuery || releaseMatchesGenre(item, genre)))
  const collections = searched.filter(item => !isRelease(item) && (!genre || debouncedQuery || releaseMatchesGenre(item, genre)))
  const loadPersonalAlbums = tab === 'albums' && !debouncedQuery && lists?.status === 'ready' && !albums.length
  const { data: own } = useRemote(['playlists', 'mine'], () => api.playlists('mine'), loadPersonalAlbums)
  const { data: saved } = useRemote(['playlists', 'liked'], () => api.playlists('liked'), loadPersonalAlbums)
  const personalAlbums = [...new Map([...(saved?.status === 'ready' ? saved.data : []), ...(own?.status === 'ready' ? own.data : [])].filter(isAlbum).map(item => [item.id, item])).values()]
  const showPersonalAlbums = !debouncedQuery && lists?.status === 'ready' && !albums.length && personalAlbums.length > 0
  const personalAlbumsPending = !debouncedQuery && !albums.length && (!own || !saved || own.status === 'loading' || saved.status === 'loading')
  const catalogAlbums = albums.length ? albums : showPersonalAlbums ? personalAlbums : []
  const releaseKind = (item: Playlist) => (item.set_type || item.playlist_type || (item.is_album ? 'album' : '')).toLocaleLowerCase()
  const visibleAlbums = catalogAlbums.filter(item => kind === 'all' || releaseKind(item).includes(kind)).sort((a, b) => sort === 'popular' ? (b.likes_count || 0) - (a.likes_count || 0) : sort === 'tracks' ? (b.track_count || 0) - (a.track_count || 0) : sort === 'az' ? a.title.localeCompare(b.title) : (b.created_at || '').localeCompare(a.created_at || ''))
  const featured = catalogAlbums[0] || collections[0]
  const surprise = () => { const choices = [...catalogAlbums, ...collections]; const picked = choices[Math.floor(Math.random() * choices.length)]; if (picked) useApp.getState().openPlaylist(picked.id, picked.title) }
  const playFeatured = async () => {
    if (!featured) return
    setFeaturedError('')
    try {
      const result = await api.tracks('playlist', undefined, featured.id)
      const playlistTracks = result.status === 'ready' && result.data.length ? result.data : featured.tracks || []
      if (!playlistTracks.length) throw new Error(t('В этой подборке пока нет доступных треков.', 'No playable tracks in this collection yet.'))
      await api.play(playlistTracks, 0)
      await queryClient.invalidateQueries({ queryKey: ['player'] })
    } catch (cause) { setFeaturedError(String(cause)) }
  }
  return <div className="page-content catalog-page"><section className="catalog-hero"><div className="catalog-hero-emblem"><Music2 size={48} strokeWidth={1.2} /></div><div className="catalog-hero-copy"><span className="page-kicker">FASTCLOUD / {t('КАТАЛОГ', 'CATALOG')}</span><h1>{t('Каталог', 'Catalog')}</h1><p>{t('Альбомы, авторы и новые звуки в одном месте.', 'Albums, artists and new sounds in one place.')}</p><button className="primary-button" disabled={!featured} onClick={surprise}><Shuffle size={16} /> {t('Удиви меня', 'Surprise me')}</button></div><div className="catalog-hero-stats"><span><strong>{catalogAlbums.length || '—'}</strong>{t('альбомов найдено', 'albums found')}</span><span><strong>{featuredArtists.length || '—'}</strong>{t('авторов в жанре', 'artists in genre')}</span><span><strong>{collections.length || '—'}</strong>{t('подборок найдено', 'collections found')}</span></div></section>
    {featured && <section className="catalog-feature"><Artwork item={featured} size="hero" /><div><small>{t('ВЫБОР КАТАЛОГА', 'CATALOG PICK')}</small><h2><button onClick={() => useApp.getState().openPlaylist(featured.id, featured.title)}>{featured.title}</button></h2><span>{featured.user?.username || t('Подборка', 'Collection')} · {featured.track_count || 0} {t('треков', 'tracks')}</span></div><button className="catalog-feature-play" aria-label={`${t('Слушать', 'Play')} ${featured.title}`} onClick={() => void playFeatured()}><Play size={23} fill="currentColor" /></button></section>}{featuredError && <p className="error-text" role="alert">{featuredError}</p>}
    <GenreCarousel selected={debouncedQuery ? undefined : genre} onSelect={item => { setQuery(''); setDebouncedQuery(''); setGenre(item) }} english={english} showAll />
    <div className="catalog-toolbar"><div className="tabs"><button className={tab === 'albums' ? 'active' : ''} onClick={() => setTab('albums')}>{t('Альбомы', 'Albums')}</button><button className={tab === 'artists' ? 'active' : ''} onClick={() => setTab('artists')}>{t('Авторы', 'Artists')}</button></div><input className="catalog-search" value={query} onChange={event => setQuery(event.target.value)} placeholder={t('Название или автор…', 'Title or artist…')} aria-label={t('Поиск в каталоге', 'Search catalog')} /></div>
    {tab === 'albums' ? <><div className="catalog-filter-row"><div className="catalog-filter-group" role="group" aria-label={t('Тип релиза', 'Release type')}>{([['all', t('Все', 'All')], ['album', t('Альбом', 'Album')], ['ep', 'EP'], ['single', t('Сингл', 'Single')], ['compilation', t('Сборник', 'Compilation')]] as const).map(([value, label]) => <button key={value} className={kind === value ? 'active' : ''} onClick={() => setKind(value)}>{label}</button>)}</div><div className="catalog-filter-group" role="group" aria-label={t('Сортировка', 'Sort by')}>{([['new', t('Свежие', 'Newest')], ['popular', t('Популярные', 'Popular')], ['tracks', t('Треков', 'Tracks')], ['az', 'А–Я']] as const).map(([value, label]) => <button key={value} className={sort === value ? 'active' : ''} onClick={() => setSort(value)}>{label}</button>)}</div></div><SectionTitle title={showPersonalAlbums ? t('Из твоей коллекции', 'From your collection') : debouncedQuery ? `${t('По запросу', 'Matching')} “${debouncedQuery}”` : genre ? `${t('Релизы', 'Releases')} · ${genre}` : t('Свежие релизы', 'Fresh releases')} subtitle={`${visibleAlbums.length} ${t('релизов', 'releases')}`} /><Status value={lists}>{() => visibleAlbums.length ? <PlaylistCards playlists={visibleAlbums} /> : personalAlbumsPending ? <p className="muted" role="status">{t('Загружаем альбомы…', 'Loading albums…')}</p> : <Empty message={t('Релизы не найдены', 'No releases found')} detail={t('Попробуй другой жанр или тип релиза.', 'Try another genre or release type.')} />}</Status>{collections.length > 0 && <><SectionTitle title={t('Подборки', 'Collections')} /><PlaylistCards playlists={collections} /></>}</> : debouncedQuery ? <><SectionTitle title={`${t('Авторы по запросу', 'Artists matching')} “${debouncedQuery}”`} /><Status value={users}>{items => <People users={items} />}</Status></> : <><SectionTitle title={genre ? `${t('Авторы', 'Artists')} · ${genre}` : t('Авторы релизов', 'Release artists')} subtitle={t('Авторы треков этого жанра', 'Artists in this genre')} />{(genre ? tracks : lists)?.status === 'ready' ? <People users={featuredArtists} /> : <Status value={genre ? tracks : lists}>{() => null}</Status>}</>}
    {!debouncedQuery && genreTracks.length > 0 && <><SectionTitle title={`${t('Треки', 'Tracks')} · ${genre}`} subtitle={t('Слушай прямо из каталога', 'Play directly from the catalog')} /><TrackRows tracks={genreTracks.slice(0, 12)} /></>}
  </div>
}

export function TrackComments({ track }: { track: Track }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data } = useRemote(['comments', track.id], () => api.comments(track.id))
  const queryClient = useQueryClient()
  const [body, setBody] = useState('')
  const [atTime, setAtTime] = useState(false)
  const [error, setError] = useState('')
  const submit = async () => {
    try { const player = await api.player(); await api.postComment(track.id, body, atTime && player.queue[player.current ?? -1]?.id === track.id ? player.positionMs : undefined); setBody(''); await queryClient.invalidateQueries({ queryKey: ['comments', track.id] }) } catch (cause) { setError(String(cause)) }
  }
  const seekToComment = async (timestampMs: number) => {
    try { const player = await api.player(); if (player.queue[player.current ?? -1]?.id !== track.id) await api.play([track], 0); await api.transport('seek', timestampMs); await queryClient.invalidateQueries({ queryKey: ['player'] }) } catch (cause) { setError(String(cause)) }
  }
  return <section className="comments"><SectionTitle title={t('Комментарии', 'Comments')} subtitle={`${track.comment_count || 0} ${t('обсуждений', 'comments')}`} /><form className="comment-form" onSubmit={event => { event.preventDefault(); void submit() }}><textarea value={body} onChange={event => setBody(event.target.value)} placeholder={t('Написать комментарий…', 'Write a comment…')} aria-label={t('Комментарий', 'Comment')} /><label><input type="checkbox" checked={atTime} onChange={event => setAtTime(event.target.checked)} /> {t('Привязать к текущей секунде', 'Post at current time')}</label><button className="secondary-button" type="submit" disabled={!body.trim()}><Send size={15} /> {t('Отправить', 'Post')}</button></form>{error && <p className="error-text">{error}</p>}<Status value={data}>{items => items.length ? <div className="comment-list">{items.map(comment => <div className="comment" key={comment.id}><strong>{comment.user?.username || t('Слушатель', 'Listener')}</strong>{comment.timestamp_ms != null && <button className="comment-timestamp" title={t('Слушать с этого момента', 'Play from this moment')} onClick={() => void seekToComment(comment.timestamp_ms!)}>{duration(comment.timestamp_ms)}</button>}<p>{comment.body}</p></div>)}</div> : <Empty message={t('Комментариев пока нет', 'No comments yet')} />}</Status></section>
}

export function UploadForm() {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const [form, setForm] = useState({ path: '', title: '', artist: '', description: '', genre: '', tags: '', public: true })
  const uploadPath = useApp(state => state.uploadPath)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const queryClient = useQueryClient()
  const set = (key: keyof typeof form, value: string | boolean) => setForm(current => ({ ...current, [key]: value }))
  useEffect(() => { if (uploadPath) { setForm(current => ({ ...current, path: uploadPath, title: current.title || uploadPath.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') || '' })); useApp.getState().setUploadPath(null) } }, [uploadPath])
  const chooseFile = async () => {
    try { const path = await api.pickAudioFile(); if (path) set('path', path) }
    catch (cause) { setMessage(String(cause)) }
  }
  const submit = async () => {
    setBusy(true); setMessage('')
    try { const uploaded = await api.uploadTrack(form); setMessage(`${t('Загружено', 'Uploaded')}: ${uploaded.title}`); await queryClient.invalidateQueries({ queryKey: ['tracks', 'uploads'] }); setForm(current => ({ ...current, path: '', title: '' })) }
    catch (cause) { setMessage(String(cause)) }
    finally { setBusy(false) }
  }
  return <form className="creator-form" onSubmit={event => { event.preventDefault(); void submit() }}><h3>{t('Загрузить трек', 'Upload track')}</h3><p className="muted">{t('Выбери аудиофайл на этом компьютере.', 'Choose an audio file on this computer.')}</p><div className="inline-form"><input value={form.path} onChange={event => set('path', event.target.value)} placeholder={t('Путь к аудиофайлу', 'Audio file path')} aria-label={t('Путь к аудиофайлу', 'Audio file path')} required /><button className="secondary-button" type="button" disabled={api.preview} onClick={() => void chooseFile()}>{t('Выбрать файл…', 'Choose file…')}</button></div><input value={form.title} onChange={event => set('title', event.target.value)} placeholder={t('Название трека', 'Track title')} aria-label={t('Название трека', 'Track title')} required /><div className="form-grid"><input value={form.artist} onChange={event => set('artist', event.target.value)} placeholder={t('Исполнитель', 'Artist')} aria-label={t('Исполнитель', 'Artist')} /><input value={form.genre} onChange={event => set('genre', event.target.value)} placeholder={t('Жанр', 'Genre')} aria-label={t('Жанр', 'Genre')} /></div><input value={form.tags} onChange={event => set('tags', event.target.value)} placeholder={t('Теги через пробел', 'Space-separated tags')} aria-label={t('Теги', 'Tags')} /><textarea value={form.description} onChange={event => set('description', event.target.value)} placeholder={t('Описание', 'Description')} aria-label={t('Описание', 'Description')} /><label><input type="checkbox" checked={form.public} onChange={event => set('public', event.target.checked)} /> {t('Публичный трек', 'Public track')}</label><button className="secondary-button" type="submit" disabled={busy || !form.path.trim() || !form.title.trim()}>{busy ? t('Загружаем…', 'Uploading…') : t('Загрузить в SoundCloud', 'Upload to SoundCloud')}</button>{message && <p role="status">{message}</p>}</form>
}

export function TrackCreatorTools({ track }: { track: Track }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const t = (ru: string, en: string) => settings?.language === 'English' ? en : ru
  const { data: uploads } = useRemote(['tracks', 'uploads'], () => api.tracks('uploads'))
  const owns = uploads?.status === 'ready' && uploads.data.some(item => item.id === track.id)
  const [editing, setEditing] = useState(false)
  const [form, setForm] = useState({ title: track.title, artist: artist(track), description: track.description || '' })
  const [storefront, setStorefront] = useState({ id: track.id, title: '', kind: 'buy', link: '', linkTitle: '', description: '', price: '' })
  const [message, setMessage] = useState('')
  const queryClient = useQueryClient()
  if (!owns) return null
  const save = async () => {
    try { await api.editTrack(track.id, form.title, form.artist, form.description); await queryClient.invalidateQueries({ queryKey: ['track', track.id] }); await queryClient.invalidateQueries({ queryKey: ['tracks', 'uploads'] }); setEditing(false); setMessage(t('Изменения сохранены', 'Changes saved')) } catch (cause) { setMessage(String(cause)) }
  }
  const remove = async () => {
    if (!window.confirm(t(`Удалить трек «${track.title}» из SoundCloud?`, `Delete “${track.title}” from SoundCloud?`))) return
    try { await api.deleteTrack(track.id); await queryClient.invalidateQueries({ queryKey: ['tracks', 'uploads'] }); useApp.getState().setPage('library') } catch (cause) { setMessage(String(cause)) }
  }
  return <section className="creator-form"><SectionTitle title={t('Инструменты автора', 'Creator tools')} /><div className="track-detail-buttons"><button className="secondary-button" onClick={() => setEditing(!editing)}>{editing ? t('Закрыть редактор', 'Close editor') : t('Изменить трек', 'Edit track')}</button><button className="secondary-button danger-button" onClick={() => void remove()}>{t('Удалить трек', 'Delete track')}</button></div>{editing && <><div className="form-grid"><input value={form.title} onChange={event => setForm(current => ({ ...current, title: event.target.value }))} aria-label={t('Название трека', 'Track title')} /><input value={form.artist} onChange={event => setForm(current => ({ ...current, artist: event.target.value }))} aria-label={t('Исполнитель', 'Artist')} /></div><textarea value={form.description} onChange={event => setForm(current => ({ ...current, description: event.target.value }))} aria-label={t('Описание трека', 'Track description')} /><button className="secondary-button" onClick={() => void save()} disabled={!form.title.trim()}>{t('Сохранить трек', 'Save track')}</button><h3>{t('Витрина', 'Storefront')}</h3><div className="form-grid"><input value={storefront.title} onChange={event => setStorefront(current => ({ ...current, title: event.target.value }))} placeholder={t('Заголовок', 'Title')} aria-label={t('Заголовок витрины', 'Storefront title')} /><select value={storefront.kind} onChange={event => setStorefront(current => ({ ...current, kind: event.target.value }))} aria-label={t('Тип витрины', 'Storefront type')}><option value="buy">{t('Купить', 'Buy')}</option><option value="download">{t('Скачать', 'Download')}</option><option value="stream">{t('Слушать', 'Stream')}</option></select></div><input value={storefront.link} onChange={event => setStorefront(current => ({ ...current, link: event.target.value }))} placeholder="https://…" aria-label={t('Ссылка витрины', 'Storefront link')} /><input value={storefront.linkTitle} onChange={event => setStorefront(current => ({ ...current, linkTitle: event.target.value }))} placeholder={t('Подпись ссылки', 'Link label')} aria-label={t('Подпись ссылки', 'Link label')} /><div className="form-grid"><input value={storefront.price} onChange={event => setStorefront(current => ({ ...current, price: event.target.value }))} placeholder={t('Цена', 'Price')} aria-label={t('Цена', 'Price')} /><input value={storefront.description} onChange={event => setStorefront(current => ({ ...current, description: event.target.value }))} placeholder={t('Описание', 'Description')} aria-label={t('Описание витрины', 'Storefront description')} /></div><button className="secondary-button" disabled={!storefront.title.trim() || !storefront.link.trim()} onClick={() => void api.saveStorefront(storefront).then(() => setMessage(t('Витрина сохранена', 'Storefront saved'))).catch(cause => setMessage(String(cause)))}>{t('Сохранить витрину', 'Save storefront')}</button></>}{message && <p role="status">{message}</p>}</section>
}
