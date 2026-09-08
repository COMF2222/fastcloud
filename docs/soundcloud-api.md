# Working with the SoundCloud API

Notes for anyone (human or agent) touching `src/api/`, `src/auth/` or
`src/store.rs`. The short version: the API is narrower than it looks, and
guessing at endpoints wastes a day.

## Authoritative sources

Use these; do not infer routes, parameters or headers from other clients.

- [OpenAPI spec (YAML)](https://github.com/soundcloud/api/blob/master/openapi/api.yaml) — the reference
- [Swagger UI explorer](https://developers.soundcloud.com/docs/api/explorer/open-api)
- [API Guide](https://developers.soundcloud.com/docs/api/guide) — auth, playback, pagination, errors
- [Register an app](https://developers.soundcloud.com/docs/api/register-app)
- [Rate limits](https://developers.soundcloud.com/docs/api/rate-limits)
- [Terms of Use](https://developers.soundcloud.com/docs/api/terms-of-use) and
  [attribution](https://developers.soundcloud.com/docs/api/buttons-logos)

## Hard rules

1. **Hosts.** API calls go to `https://api.soundcloud.com`; tokens come from
   `https://secure.soundcloud.com`; app registration goes to
   `https://api-reg.soundcloud.com`. None of the three are interchangeable.
2. **Header.** `Authorization: OAuth <access_token>` — *not* `Bearer`. The
   registration API takes the same scheme.
3. **Paths take URNs.** `/tracks/soundcloud%3Atracks%3A123`, not `/tracks/123`.
   Numeric ids survive on a few legacy routes only. See `api::endpoints`.
4. **PKCE is required, and so is the secret.** OAuth 2.1 with
   `code_challenge_method=S256`. SoundCloud treats every client as
   confidential, so the token exchange also carries `client_secret` — which is
   why Fastcloud ships no keys and instead registers one per user
   (`auth::register`).
5. **Refresh tokens are single-use.** Persist the new pair before anything
   else uses it. `auth::Session` serialises refreshes behind one mutex so two
   callers racing a 401 cannot spend the same token twice.
6. **Client-credentials tokens are rate-limited hard**: 50 per 12 h per app,
   30 per hour per IP. Cache them; never mint one per launch.
7. **Never hardcode a client id, secret or token.** They live in the OS
   keyring or the environment. (`auth::register::PAIRING_CLIENT_ID` is the one
   exception, and it is SoundCloud's own published public client — see below.)
8. **Pagination** is `linked_partitioning=true` plus `next_href`; follow it
   until it is absent. Collections answer 50 by default, 200 at most.
9. **429** carries `Retry-After`; back off and say so in the interface
   (`api::client::NetActivity` drives the top bar's indicator).
10. **Not every track is playable.** `access` is `playable`, `preview` or
    `blocked`; `policy` may be `SNIP` or `BLOCK` and `monetization_model` may
    be `SUBSCRIPTION` (Go+). Handle all of them — `Track::access_badge`
    already classifies them.
11. **Play requests are limited to 15 000 per 24 h per `client_id`.** This is
    why each user registers their own app rather than sharing one: a shipped
    key would cap the whole project at a few hundred listeners.

## Endpoint coverage

Fastcloud wraps every operation in SoundCloud's published OpenAPI document
(64 GET/POST/PUT/DELETE operations as of 2026-09-08). The maintained wrappers
live in `src/api/endpoints.rs`; deprecated routes are kept for compatibility
but are not presented as primary UI actions.

| OpenAPI area | Fastcloud feature |
|---|---|
| `/me`, likes, playlists, tracks, reposts, followers/followings | Account profile and complete paginated Library tabs |
| `/me/feed`, `/me/feed/tracks` | Feed with track and playlist posts |
| `/me/recently-played/tracks` | Track-level History in server order; Home preserves the last user-started album/track context |
| `/tracks`, `/playlists`, `/users`, `/resolve` | Search and opening SoundCloud links |
| track detail, comments, favoriters, reposters, related | Track page, people lists and stations/autoplay; comment endpoints stay wrapped but comments are intentionally omitted from the UI |
| track upload/update/delete | Creator upload form and owner-only track tools |
| track storefront | Owner-only Artist Storefront editor |
| track preview and streams | Preview fallback and full HLS playback (AAC 160 preferred) |
| playlist create/update/delete/tracks/reposters | Real SoundCloud playlist creation and ordered editing |
| user detail/related/lists/web profiles | Public profile tabs and external profile links |
| likes, reposts and follows writes | Optimistic buttons reconciled against SoundCloud |
| `/sign-out` | Sign out control; the older disconnect wrapper remains for compatibility |
| deprecated activities/favorites/relationship detail routes | Compatibility wrappers only |

Every collection that represents a library or profile follows `next_href`
until exhausted. Discovery/search pages stay bounded to avoid burning the
account's API quota merely by opening a page.

## Registering the user's app

`auth::register` implements SoundCloud's device-pairing flow, the one their
[`sc-api-auth` CLI](https://developers.soundcloud.com/docs/api/register-app#alternative-register-from-the-command-line)
uses. One browser sign-in produces the user's own `client_id` and
`client_secret`:

```
POST /pairing/codes?client_id=…     → { code, poll_token, poll_interval_seconds }
                                      user opens secure.soundcloud.com/activate/<code>
GET  /pairing/codes/<code>?…        → { status: created | activated | expired }
POST /pairing/sign-in               → { session: { access_token } }
POST /me/apps                       → { client_id, client_secret, … }
```

Three things about it are load-bearing:

* **The 403 is the subscription check.** `application_creation_not_available`
  means the account has no Artist Pro, which is SoundCloud's requirement for
  API access. It is a separate `RegisterError` variant because the answer is a
  subscription, not a retry.
* **`user_already_has_application` is not a failure.** One app per account, so
  `GET /me/apps` returns the existing one and the flow succeeds.
* **A 404 while polling means "not yet".** The code is not visible to the
  pairing service the instant it is created.

None of `api-reg.soundcloud.com` is in the published OpenAPI spec, and the
pairing client id belongs to SoundCloud's CLI rather than to us — so this can
change without notice. That is why Settings keeps a manual key field: it is the
way back in if it does.

## What the public API does not have

Do not add an endpoint for these. Each already has a documented substitute:

| Missing | What we do instead |
|---------|--------------------|
| Charts, trending | A recent window of `/tracks?genres=…`, sorted by plays locally (`store::trending`) |
| Recommendations (Daily Drops, Made for you) | `/tracks/{urn}/related` seeded from history and likes (`App::shelf_key`) |
| Editorial selections | `/resolve` on known permalinks, or a playlist search |
| Stations | A seed track plus its related tracks with autoplay (`App::station_queue`) |
| Playlist snapshots/versioning | `PUT /playlists/{urn}` replaces the whole list; edits are optimistic (`crate::playlists`) |
| Progressive MP3 | HLS only (`hls_mp3_128_url`, `hls_aac_160_url`) plus `preview_mp3_128_url` |
| A "Liked Songs" playlist | `/me/likes/tracks`; the card is ours |
| Direct messages | A local inbox of links opened into the app (`ui::messages`) |
| Playback on another device | Playback is always local |

`/me/recently-played/tracks` exists but answers at most 25 unique track rows and
takes no `limit` and no pagination. It powers the track-level History tab. The
web site's heterogeneous listening contexts (album, playlist, station, artist)
are not exposed publicly, so Home remembers the last queue explicitly started
in Fastcloud. Automatic related tracks do not replace that album/track context.

## Testing against the real API

Set `FASTCLOUD_CLIENT_ID` and `FASTCLOUD_CLIENT_SECRET` and run without
`--demo` — the environment beats the keyring, so this works on a machine that
has a registration of its own.

Without credentials the app shows the connect screen rather than the interface,
so a UI change needs one of the two: either keys, or the offline demo library,
which needs neither network nor account:

```sh
cargo run -- --demo
```
