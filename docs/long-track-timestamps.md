# Long track timestamps — 0.4.21

Track rows, the compact timeline, full-screen player, queue/detail labels and
collection totals share `duration` from `desktop/src/types.ts`. It displays m:ss
below one hour and h:mm:ss from one hour onward (140 minutes → 2:20:00). Missing,
negative and non-finite values resolve to 0:00. Hours do not wrap after 24.
Playlist totals now reuse this formatter instead of maintaining a second copy.

Timeline labels grow with their content and do not shrink. Full-player time
columns and track duration columns size to content so the hour prefix remains
visible at larger interface text sizes.

Verified on 2026-10-16: 35 frontend tests pass, production frontend build passes,
and `cargo check --locked` passes. A 140-minute demo recording shows 2:20:00 in
its result row, bottom player and full-screen player. Seeking to its end shows
2:20:00 for elapsed time as well. DOM measurements confirm the timestamp labels
fit their containers. This is browser/demo verification; a macOS runtime was
not exercised on Windows.
