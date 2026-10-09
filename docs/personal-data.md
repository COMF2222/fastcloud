# Personal data and playback additions

The desktop client keeps a local copy and syncs portable account data with the
backend every 30 seconds while signed in. Settings → General provides manual
sync and JSON backup. Account identity comes from the authenticated server
session, never from a user ID supplied by a client.

## Storage contract for desktop and future mobile clients

`GET /v1/me/personal` and `POST /v1/me/personal` use the same OAuth header as the
existing relay. The backend stores metadata in its existing SQLite volume.
No additional database service or paid API is required.

| Data | Server | Device |
| --- | --- | --- |
| Colours, sizes, opacity, lyric preferences, language | Authoritative portable fields | Offline copy |
| Custom appearance presets, musical taste, quick-access pins | Portable fields | Offline copy |
| Playlist folders and smart-playlist rules | Entities keyed by stable UUID | Pending edits/offline copy |
| Dislikes and their removals, with minimal public track metadata | Per-account `trackFeedback` | Offline copy and pending edits |
| Listening seconds, listen counts, last-played times, new-like dates | Per-account metadata | Unsent counters |
| Local wallpaper/font/skin paths, downloads, audio device, volume, EQ | No | Local settings/files |
| Current playback session, queue undo history, sleep deadline | No | Current device |
| SoundCloud tokens | Excluded from this API and backups | Existing authentication flow |

Wallpaper and font *files* are not uploaded. A new device receives colours and
other portable preferences but needs its own local image/font. Existing
SoundCloud likes and playlists remain owned by SoundCloud. The recommendation
engine's audio embeddings and internal learned signal cache remain local.

POST patches individual keys in `preferences`, `folders`, `smartPlaylists`,
`likedAt` and `trackFeedback`; missing keys keep their server value. A null folder/rule deletes that
entity, without deleting the original SoundCloud playlist. Concurrent changes
to different keys survive; for the same key the last accepted write wins.
The local client sends only dirty keys and overlays edits made while a request
was in flight. It retains pending edits when the network/server is unavailable.

Dislikes are keyed by the decimal SoundCloud track ID. Each value contains
`disliked`, `updatedAt` (Unix milliseconds), `device` (32 lowercase hex characters)
and `track`: `id`, `title`, `artist`, `durationMs`, `genre`, `artworkUrl`,
`permalinkUrl`, `isrc`. URLs are limited to public SoundCloud/sndcdn HTTPS hosts;
query strings, descriptions, stream addresses and credentials are not uploaded.
ISRC is optional and normalized to 12 alphanumeric characters.

For feedback the greatest `(updatedAt, device)` wins. Removing a dislike writes
`disliked: false` rather than deleting the row, so an older offline retry cannot
reintroduce it. GET results merge with pending local ratings before POST; edits
made during a request remain queued. Legacy dislike flags migrate only when the
local cache has a known account owner, using timestamp zero so they cannot
replace a newer removal on the server. Undo also clears matching reupload ratings.

The dislike view polls every 10 seconds; account synchronization runs every
30 seconds. Likes, playlists and followed accounts remain canonical in SoundCloud
and use the existing authenticated backend relay. Future mobile clients can use
these same endpoints; a second likes database is not needed.

Example collection patch:

```json
{
  "preferences": { "language": "English", "crossfade_ms": 4000 },
  "folders": {
    "road": { "name": "Road", "playlistIds": [123], "pinned": true, "order": 0 }
  },
  "smartPlaylists": {
    "old-rock": { "name": "Old favourites", "genre": "Rock", "addedDays": 0, "unplayedDays": 30, "limit": 50 }
  }
}
```

Listening uploads use a stable random 32-character hex device ID and cumulative
`ms`/`plays` counters for each UTC day and track. The server accepts the maximum
counter seen for that device/day/track, so retrying a request never doubles it.
Independent devices add together. A lost local cache gets a new device ID.
Pauses, buffering and seek jumps do not count as listening. A listen is counted
after 30 seconds or half a short track. Old listening time is not reconstructed;
unknown old like dates do not qualify for recent-like rules. Reports include
365 daily totals and up to 5000 lifetime track aggregates.

Limits: 2 MiB per request/backup, 100 listening rows per batch, 100 folders,
1000 playlist IDs per folder, 50 rules, 20 custom presets and 100 shortcuts.
Feedback uploads contain at most 100 changes per batch, with at most 5000 rating
rows (including removals) per account.
The server validates types, ranges, known fields and collection sizes.

## Playback behaviour

- Crossfade is optional (2/4/6/8 seconds), capped at half the outgoing track's
  duration. It starts only after the next full stream has been prepared.
- Gapless prepares the next track before the current one ends. If preparation
  fails, normal playback/loading handles the next track. Turn crossfade off for
  albums that should retain their original transitions.
- Normalization estimates RMS and peak from the opening PCM, applies a static
  per-track gain with at most 2× amplification and peak headroom. It is not a
  full-track LUFS scan and cannot guarantee equal loudness for every recording.
- Sleep deadlines fade during the last 10 seconds and pause without changing
  saved volume. An after-current timer disables transitions and is cancelled
  when another track is selected manually. Timers are not restored after exit.
- Queue undo retains at most five manual edits, preserves the currently playing
  track where possible, and restores the corresponding My Wave session.

Autoplay starts a recommendation continuation for a selected album/playlist
queue and refills near its end. Explicitly selected tracks are retained; suggested
tracks are filtered against current dislikes both before append and while queued.
The continuation is not labelled as My Wave. Turning autoplay off discards an
in-flight continuation batch. If a new batch arrives after EOF, the decoder
resumes it unless the user has paused playback.

## Sharing

Share controls on tracks, album/playlist cards and lists, detail pages and the
bottom player first copy the canonical public SoundCloud link, then open
`https://soundcloud.com/messages` in the default browser. Missing links are
resolved through the existing track/playlist detail API. A copy or browser error
is shown explicitly with a recoverable link. No message is sent automatically.
SoundCloud's public API has no messages endpoint or documented message-prefill
URL; the user chooses a conversation and pastes the copied link.

## Rollout and verification

Deploy the backend before releasing the client. The schema is created
idempotently inside the existing approvals SQLite database; keep its persistent
volume. Older clients continue to work. Back up that database with SQLite's
backup API (a plain copy of a live WAL database can omit recent writes).

Run backend tests with `python -m unittest discover -q`, desktop Rust tests with
`cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --all-targets`,
and frontend checks with `npm run check` from `desktop`. Network integration
tests use fixture OAuth sessions, localhost HTTP servers and temporary settings
paths. They do not use real user tokens or installed application settings.

Before release, verify actual output-device playback on two adjacent tracks,
album gapless playback, pausing/seeking during a transition, and synchronization
between two signed-in installed clients. Browser preview verifies UI interactions
but does not exercise native audio or a production server.
