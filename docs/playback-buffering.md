# Streaming buffer verification

The player used to request two HLS segments sequentially only after the
decoder reached the end of its current bytes. It appended neither segment
until both requests completed. A slow second response could therefore keep
already downloaded audio out of the decoder while the output buffer drained.
The same awaited requests also held up the background position update loop.

The player now starts an ordered download task when it loads the playlist.
The task reserves a queue slot before each request, limiting ready plus
in-flight segments to two. The decoder takes each segment as soon as it needs
it; the normal background loop polls without waiting on the network. A track
change, seek, stop, failed initial load or shutdown cancels the old task.
Temporary HTTP and transport failures get up to two retries; an authorization
denial is preserved. Network segment bodies are limited to 8 MiB each.

Automated regression checks cover a stalled following segment, bounded
lookahead, cancellation during an incomplete response, transient retries,
authorization denial, and a player-loop poll during a delayed download.

On 2026-10-02, an HTTPS probe through the deployed broker fetched the first
three segments of three public, playable SoundCloud tracks. Segment request
times ranged from 0.133 to 4.676 seconds; each segment contained about ten
seconds of audio. This demonstrates real request-time variation, but does not
establish the exact cause of a particular listener's interruption or distinguish
all server cache hits from misses. No cache was cleared or token refreshed.

The separate preview investigation used the public example
https://soundcloud.com/tewiq/popal (track 2216251388). For the saved listener
grant, the official API reported `access: preview`, duration 106344 ms, and
only `preview_mp3_128_url`; both full HLS fields were absent. The shared media
resolver returned 422 as designed. The player already prefers full HLS and
does not substitute a preview on a full-stream network error. The API response
did not state why this particular track had preview-only access.
