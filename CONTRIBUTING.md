# Contributing

Bug reports, patches and skins are all welcome. The bar is low, but it is a
bar: this is an audio player, and audio players are judged on whether they
glitch.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

All three are enforced by CI on Linux, macOS and Windows, so a failure there
is a failure everywhere.

One test needs a note: `desktop::single_instance::tests::ipc_bind_roundtrip_and_exclusive`
binds the singleton port, so it fails while a Fastcloud window is open. Close
the app, or run `cargo test -- --skip ipc_bind`.

## What a good change looks like

**Behaviour changes come with a test.** Not a smoke test — one that would
have caught the bug. The existing ones are worth reading as examples: they
name the property rather than the function
(`the_reported_position_survives_compaction`,
`a_wheel_notch_is_one_volume_step`, `the_challenge_matches_the_rfc_vector`).

**Comments explain why, not what.** `// increment i` is noise;
`// A hole needs its own segment: only single-codepoint segments can be
redirected to the missing glyph` is why the code looks strange. If a constant
is not obvious, say where it came from — most of ours cite either
soundcloud.com's stylesheet or Winamp's own source.

**Don't invent endpoints.** The SoundCloud API is narrower than it looks. If
something is missing, it is probably missing on purpose — read
[docs/soundcloud-api.md](docs/soundcloud-api.md), which lists what does not
exist and the substitute we use for each, and check the
[OpenAPI spec](https://github.com/soundcloud/api/blob/master/openapi/api.yaml)
before adding a route.

**Never ship a key.** SoundCloud requires a client secret for every token, so a
key compiled into this build would be published — and its rate limits shared by
every user, which caps the whole project at a few hundred listeners. Each user
gets their own through `auth::register`. The one hardcoded id there is
SoundCloud's own public client for that flow.

**Nothing blocks the interface thread.** egui redraws many times a second and
cannot await. Fetches go through [`src/store.rs`](src/store.rs), which
answers synchronously from a cache and spawns the request; writes go out on
the runtime and report back through a channel.

**Watch the memory.** The point of this project is that it is small. Two
regressions we already fixed, as a flavour of what to avoid: reading system
fallback fonts into `Vec<u8>` (56 MB of CJK for scripts most sessions never
draw — they are memory-mapped now), and synthesizing a whole demo track as
f32 (85 MB — it is windowed now). If a change adds tens of megabytes,
measure it and say so.

## Layout

| Path | What lives there |
|------|------------------|
| `src/api/` | The SoundCloud client: endpoints, models, pagination, 429 handling |
| `src/auth/` | OAuth 2.1 + PKCE, client credentials, the keyring |
| `src/auth/register.rs` | Registering the user's own app via device pairing |
| `src/ui/connect.rs` | The connection state machine, and the flow that drives it |
| `src/ui/login.rs` | The connect screen, shown until there is an application |
| `src/store.rs` | The cache every view reads from |
| `src/audio/` | HLS download, decode, EQ/limiter, cpal output |
| `src/player/` | Queue, transport, the decode loop, session persistence |
| `src/ui/theme.rs` | soundcloud.com's design tokens (`Palette`, `Metrics`, `Type`) |
| `src/ui/data.rs` | Where a view's rows come from — live or demo |
| `src/skin/layout/` | Where each control sits in each window, in skin pixels |
| `src/skin/mod.rs` | The `.wsz` format: sheets and the sprite rectangles in them |
| `src/skin/stock.rs` | The built-in skin, painted in code |
| `src/ui/winamp/` | The three windows, drawn from those: `mod.rs`, `eq.rs`, `pl.rs` |
| `src/vis.rs` | The Winamp spectrum analyser and oscilloscope |
| `src/desktop/` | Tray, media keys, hotkeys, single instance |

## Design changes

The colours, spacing, radii and type scale are not preferences — they are
transcribed from soundcloud.com's own CSS custom properties, and
`palettes_match_soundcloud_css` and `type_scale_matches_typography_tokens`
pin them. Use the tokens (`Type::H4.rich(…)`, `Metrics::SP_2`) rather than
literals; if a token is genuinely missing, add it with the declaration it
came from in the comment.

## Skins

The mini player implements Winamp 2's format from the published sprite
coordinates. Do not add Winamp art to the repository — the built-in skin is
generated in code (`src/skin/stock.rs`), and users supply their own `.wsz`.
Winamp 5 `.wal` skins are a different format and are rejected on purpose.

Those coordinates are the format, not a choice, so they live as data in two
places and nowhere else:

* **`skin::layout`** says *where* a control goes in the window. Every classic
  skin paints its background to match, so a control a few pixels off sits on
  the wrong part of somebody's artwork.
* **`skin::sprites`** says *what* fills it — the rectangle inside a sheet.

`sprites_and_layout_agree_on_sizes` checks the two against each other for the
main window, `the_eq_and_playlist_sprites_fit_their_slots` does the same for the
other two, and `every_control_in_the_window_has_art` checks the generated skin
actually has pixels for each one. Do not write a pixel position in
`src/ui/winamp/`: add it to `layout` and let the test catch the mismatch. That
rule is what found the time display sitting on the minus sign's cell, and what
later moved the playlist's five menu buttons out of `pl.rs` and into
`layout::pl::menu` — their labels are painted into `pledit.bmp` from the same
function, so the word and its hit area cannot drift apart.

`cargo test dump_the_skin -- --ignored --nocapture` prints the generated sheets
as text, which is how to check the built-in skin's geometry without a
screenshot.

Two sheets carry colour as *art* rather than as palette, and both are read once
when a skin is worn (`MiniPlayer::read_skin_colors`) rather than cropped per
frame: `eqmain.bmp`'s one-pixel column at (115, 294) is the equaliser curve's
colour per row, and the row at (0, 314) is the preamp line's.

## The three windows

Winamp had three top-level windows that docked together. This app has one (see
`ui::App::show_mini`), so they stack inside it: main, then equaliser, then
playlist, all 275 wide, each as tall as its own state.

`skin::layout::stack` owns that arithmetic and nothing else — which window is
open, which is rolled up, where each one's top edge is. `MiniPlayer::ui` walks
`Stack::open_windows`, offsets the frame to each window's top and clips to its
area, so a window's renderer works in its own coordinates and *cannot* paint on
its neighbours. Opening or rolling up a window changes the stack's height, which
is the OS window's height.

The equaliser needs `eqmain.bmp` and the playlist `pledit.bmp`; without them
those renderers return early rather than painting holes. The window is
transparent by request (`main.rs`), so a hole would show the desktop.

## The mini player's states

Each of the three windows closes and rolls up on its own; the main one also
minimizes:

* Closing the main window puts the app's own interface back
  (`winamp::Command::Close`); closing the other two just removes them from the
  stack.
* Minimizing goes through `ViewportCommand::Minimized`, and the tray's "Show
  Fastcloud" brings it back (`IpcMessage::Show`).
* Shade mode rolls a window up to its 14-pixel title bar. It is a *state*, not a
  fourth window: `Stack::shade` is a `bool` per window, and each renderer draws
  its own rolled-up bar.

All of it survives a restart (`winamp_window`, `winamp_shade`, `winamp_on_top`,
`winamp_eq_window`, `winamp_eq_shade`, `winamp_pl_window`, `winamp_pl_shade`,
`winamp_pl_rows` in settings), and `main.rs` builds the viewport for whichever
state it was left in rather than resizing on the first frame.
