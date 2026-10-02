# Spotify import

In Settings → Integrations, open Exportify, export a playlist or Liked Songs,
then choose the downloaded CSV/ZIP in Fastcloud. All nonempty collections are
selected automatically; the destination can be corrected if a renamed CSV
does not identify its likes collection. No Spotify keys are configured in
Fastcloud. Exportify is an external service; its availability depends on Spotify.

The importer also accepts Spotify account data (`Playlist*.json`,
`YourLibrary.json`, or the account ZIP). Account details, payments, search and
listening history are not imported. Files are parsed locally; only song metadata
is used in searches through the existing official SoundCloud API relay.

Playlists are private, maintain source order, and omit repeated SoundCloud IDs.
Collections exceeding 500 matched tracks become ordered playlist parts. Likes
are actual SoundCloud likes; existing likes are fetched across all pages before
adding tracks. Matches are reused across collections. Nothing is downloaded
from Spotify, and SoundCloud playback restrictions still apply.

Matching checks title, artist credits, recording variants, duration when present,
and ISRC when both platforms provide it. Full tracks rank above previews. This
is metadata matching, not audio recognition; unmatched tracks are reported, with
up to 200 names displayed per collection. HTTP errors stop the operation and
retain its partial report instead of counting affected songs as unavailable.

Cancel retains successfully written likes/playlist parts. An incomplete playlist
is not created from a partly searched collection. Signing out or changing the
connection cancels and waits for the current request to finish before switching
accounts. Import has an isolated API client and shares the session's refresh
lock. Reimporting a playlist creates a new private copy; existing playlists are
not overwritten. Import state and source files are not persisted by Fastcloud.

Limits: 100 selected files, 16 MiB per text/expanded ZIP entry, 128 MiB per
archive/combined input, 2,000 ZIP entries, 1,000 collections, 50,000 source tracks.
ZIP contents are never extracted. UTF-8/BOM and UTF-16 exports are supported.

Validation: CSV quoting/newlines and long collections; official library and
playlist JSON with podcasts/local files; compressed nested ZIPs and expanded
size limits; variant/artist/duration rejection and preview ranking; HTTP fixture
covering pagination, private playlist order, deduplicated likes, query reuse,
search failures, cancellation and account changes. Live import with an actual
Spotify export is not exercised without a user-selected file/account.

Sources:
- [Exportify](https://github.com/pavelkomarov/exportify)
- [Spotify account data](https://support.spotify.com/us/article/understanding-your-data/)
- [SoundCloud playlist limits](https://help.soundcloud.com/hc/en-us/articles/360005673974-Playlist-Limits)
