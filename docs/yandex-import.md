# Yandex Music likes import

Settings → Integrations accepts a Yandex Music OAuth token and finds matching
SoundCloud tracks. The token is used only for direct Yandex requests during the
import; it is not saved in settings or sent to the Fastcloud server.

Metadata is read in batches of 50 using the Yandex `/tracks` form endpoint.
Duplicate source IDs are omitted, unavailable metadata is counted as missing,
and an API failure stops the import instead of counting songs as not found.

Search is limited to full playable recordings. Preview/Go snippets are not
imported as matches; credited artist names in public upload titles are accepted
only when title, duration and version checks also agree.

Matched SoundCloud IDs retain their source order, with duplicate matches
removed. Each destination playlist is private. More than 500 matches produce
ordered `Yandex Music likes - part N` playlists; for example, 1003 matches
produce parts containing 500, 500 and 3 tracks. This follows the
[SoundCloud playlist limit](https://help.soundcloud.com/hc/en-us/articles/360005673974-Playlist-Limits).

After each creation the importer reads all playlist-track pages and verifies
their ordered IDs. If creation omitted tracks, it replaces the contents of
that same playlist once, then verifies again. It never retries playlist
creation blindly. Confirmed earlier parts remain available if a later part
fails; the final result reports completed parts and the failure. Changing
SoundCloud accounts prevents subsequent writes to the new account.

The client refreshes its playlist list when import finishes. A playlist made
empty by an older client is not repaired retroactively: repeat the import
using the updated client. The importer searches for available SoundCloud
recordings; it cannot guarantee every Yandex song exists there.

Verification uses local HTTP fixtures, including 1003 ordered matches,
pagination, partial write failure, silently empty creations, failed repairs,
unavailable metadata and failed searches. Real user libraries are not used
in automated tests.
