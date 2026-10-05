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
| Listening seconds, listen counts, last-played times, new-like dates | Per-account metadata | Unsent counters |
| Local wallpaper/font/skin paths, downloads, audio device, volume, EQ | No | Local settings/files |
| Current playback session, queue undo history, sleep deadline | No | Current device |
| SoundCloud tokens | Excluded from this API and backups | Existing authentication flow |

Wallpaper and font *files* are not uploaded. A new device receives colours and
other portable preferences but needs its own local image/font. Existing
SoundCloud likes and playlists remain owned by SoundCloud. The recommendation
engine's audio embeddings and internal learned signal cache remain local.

POST patches individual keys in `preferences`, `folders`, `smartPlaylists` and
`likedAt`; missing keys keep their server value. A null folder/rule deletes that
entity, without deleting the original SoundCloud playlist. Concurrent changes
to different keys survive; for the same key the last accepted write wins.
The local client sends only dirty keys and overlays edits made while a request
was in flight. It retains pending edits when the network/server is unavailable.

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
