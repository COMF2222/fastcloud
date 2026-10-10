# Full playable recording alternatives

When SoundCloud exposes only a preview, playback resolves the original track
through `/v1/media/resolve`. The backend checks the source and searches the
**official API** for public full recordings with matching artist/title or ISRC,
duration and version markers. A remix, cover or excerpt is not an acceptable
substitute. A source without enough metadata can remain a preview.

The backend saves the source → selected recording URNs in its media SQLite DB.
Saved matches survive restarts and are shared across listeners. Metadata and
permissions are checked with the current listener's token before every use.
The full recording uses the existing bounded server HLS segment cache; listeners
and direct plays of the same upload reuse its audio rather than duplicate it.
Tokens and signed audio URLs are never saved in the correspondence table.

The client keeps the original library track, likes, queue identity and lyrics.
The HLS playlist supplies the actual playback duration, preview badge and seek
range. This also applies when preparing the next track for crossfade. A first
preview lookup has a longer timeout; ordinary tracks retain their normal timeout.
Older servers continue to work. If no verified full upload is available, the
existing preview remains available rather than playing an unrelated song.

Spotify and Yandex Music imports search only `access=playable` tracks and reject
snippet/non-streamable matches. A public uploader can qualify when the credited
artist appears in the title and duration/version checks also agree.

Deploy the backend changes before distributing the new client. The checked
backend deploy script runs the existing migrations and backups; the media-cache
correspondence table initializes automatically. No additional secrets or paid
recognition service are required. Actual catalog coverage depends on available
public recordings and reliable metadata, not a promised 100% match rate.
