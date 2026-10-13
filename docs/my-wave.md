# My Wave taste updates

The recommendation engine keeps lifetime counters and up to 28 daily feedback
buckets per track in the existing local `my-wave-profile.json`. Actual playback,
early skips and completed listens update these buckets. Pauses, loading and large
seeks do not create listening time. Existing profiles load without a reset.

Daily feedback has a seven-day half-life. Lifetime counters keep a small,
capped contribution, so old favorites remain useful without dominating a new
listening pattern. Legacy profiles use the last-play date as an approximation
until daily feedback exists. A new listen then adds only its own daily feedback;
it does not refresh all the track's historical plays. Explicit dislikes never
expire and remain exclusions, including recognized reuploads.

When there are recently enjoyed tracks outside likes, up to three of the four
regular discovery sources come from them. Likes provide a cold-start fallback
and continue to contribute after the taste profile develops. Recently heard
tracks can be sources for unheard related music without being repeated in the
resulting queue. Artist diversity and session exclusions still apply.

The audio taste model uses the same time-weighted feedback. Up to two additional
sources are selected from already analyzed, positively received tracks with
similar sound and different performers. Their related pages are fetched in
parallel with the regular pages, with at most six related-page searches per refill.
There are no speculative audio-stream downloads for recommendation candidates.

CLAP metadata estimates are evaluated across all source pages in round-robin
order, up to 48 new candidates per batch. Genre and style tags supplement the
title and artist. Cached vectors are reused; inference retains the existing
700 ms wait budget and can finish in the background. Pending analysis is bounded
to 96 candidates across overlapping wave requests. A real audio embedding,
when available, takes precedence over a metadata estimate. Without audio taste
or an available CLAP runtime, ordinary metadata recommendations still work.

These are ranking improvements, not a guarantee that every recommendation will
match a listener's taste. Assess actual quality from completion, skips, dislikes
and performer diversity during real sessions. Existing undated likes cannot
reconstruct listening dates from before Fastcloud recorded them.

## Radio from a track, collection or artist

Radio uses the selected music as its source rather than the personal My Wave
profile. The track menu, track page and expanded player start track radio;
playlist/album pages and artist profiles provide their own radio action.
The opening song plays immediately while related pages load in the background.

Up to sixteen playable, distinct seeds cover the whole collection. Each refill
fetches at most four related pages concurrently, with a nine-second timeout per
page. Pages contribute in round-robin rank order. Artist radio aims for one song
from the source performer per three songs, with a maximum of eight per 24-song
batch; other performers contribute at most two per batch. Availability and
dislikes can reduce these counts. The personal discovery/mood mode does not
override an explicitly selected radio source.

Dislikes, preview policies, seen IDs and recording identities exclude candidates;
dislikes are checked again at queue append time. The original seeds and radio
kind are device-local playback state and survive restart and queue undo.
Radio continues independently of the ordinary playlist autoplay preference.
Clearing the upcoming queue, stopping or manually playing another queue cancels
the station. A previous station's pending results cannot append to a new session.

On Android, related-page requests explicitly use `linked_partitioning=true`.
Native JSON parsing expects an object containing `collection`; the unpartitioned
array response previously left track radio with only its opening song.
