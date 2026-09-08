#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use anyhow::{Context, Result};
use clap::Parser;

mod api;
mod app_icon;
mod audio;
mod auth;
mod bidi;
mod cli;
mod config;
mod demo;
mod desktop;
mod fonts;
mod images;
mod link;
mod player;
mod playlists;
mod skin;
mod store;
mod system_fonts;
mod ui;
mod util;
mod vis;
mod waveforms;

use crate::player::Player;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = cli::Cli::parse();

    // CLI subcommand: forward to a running instance and exit.
    // Exit codes mirror fastpotify: 0 ok, 1 not running, 2 bad link.
    if let Some(cmd) = &args.command {
        match cli::run_remote_command(cmd.clone()) {
            Ok(cli::IpcReply::Ok) => return Ok(()),
            Ok(cli::IpcReply::Text(t)) => {
                print!("{t}");
                return Ok(());
            }
            Ok(cli::IpcReply::Err(e)) => {
                eprintln!("fastcloud: {e}");
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("fastcloud is not running.");
                std::process::exit(1);
            }
        }
    }

    // Positional link: hand to the running instance, or boot into it.
    let boot_link: Option<String> = match &args.link {
        Some(link) => match link::parse(link) {
            Ok(_) => {
                if desktop::single_instance::is_already_running() {
                    let msg = cli::IpcMessage::OpenLink(link.clone());
                    match desktop::single_instance::send_request(&msg) {
                        Ok(_) => return Ok(()),
                        Err(_) => {
                            eprintln!("fastcloud is not running.");
                            std::process::exit(1);
                        }
                    }
                }
                Some(link.clone())
            }
            Err(e) => {
                eprintln!("fastcloud: invalid link: {e}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    // Enforce a single GUI instance: the D-Bus name on Linux, the IPC port
    // everywhere. `_instance` must outlive the window, so it is kept here.
    let Some((ipc_server, _instance)) = desktop::single_instance::ensure_single_instance() else {
        std::process::exit(0);
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start tokio runtime")?;

    let mut app = build_app(&args, &rt, ipc_server)?;
    if let Some(link) = boot_link {
        app.pending_link = Some(link);
    }
    // The window comes back the way it was left: the mini player is a mode of
    // the one window, not a second one (see `ui::App::show_mini`), so the
    // viewport is built for it from the start rather than resized on frame one.
    let start_mini = app.settings.winamp_window;
    if start_mini {
        app.restore_mini();
    }
    // …at whatever height the windows it was left with add up to. Asking the
    // mini player rather than reading `winamp_shade` is what makes the
    // equaliser and the playlist come back too: the stack's height *is* the
    // window's, and a rolled-up window is a shorter one, not a different one.
    let scale = app.settings.winamp_scale.clamp(1, 4) as f32;
    let mini_height = app
        .mini
        .as_ref()
        .map(|mini| mini.lock().height())
        .unwrap_or(ui::winamp::HEIGHT);
    let mini_size = egui::vec2(ui::winamp::WIDTH * scale, mini_height * scale);
    // Read the interface face before the window exists: `install` runs inside
    // eframe's creation callback, which cannot fail gracefully.
    let interface_font = app
        .settings
        .interface_font
        .as_deref()
        .and_then(fonts::custom_face);
    let viewport = egui::ViewportBuilder::default()
        .with_title("Fastcloud")
        .with_app_id(config::APP_ID)
        // The same mark the tray and the .desktop file carry, so the taskbar
        // and Alt-Tab do not fall back to a blank square.
        .with_icon(window_icon());
    let viewport = if start_mini {
        // See-through, for skins that are not rectangles: the skin paints
        // every pixel that is the window.
        //
        // Always on top from the start too, or the window would flash behind
        // whatever has focus and only rise on the first frame.
        let level = if app.settings.winamp_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        viewport
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_window_level(level)
            .with_inner_size(mini_size)
            .with_min_inner_size(mini_size)
            .with_max_inner_size(mini_size)
    } else {
        viewport
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 560.0])
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let native = eframe::run_native(
        "Fastcloud",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            // Bounded artwork loader first, so http(s) artwork goes through
            // the RAM/disk budget instead of egui's default HTTP loader.
            cc.egui_ctx
                .add_bytes_loader(std::sync::Arc::new(app.art.clone()));
            fonts::install(&cc.egui_ctx, interface_font);
            // Windows: SMTC (souvlaki) requires the HWND of a live window.
            #[cfg(target_os = "windows")]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(handle) = cc.window_handle() {
                    if let RawWindowHandle::Win32(w) = handle.as_raw() {
                        app.attach_hwnd(w.hwnd.get() as *mut std::ffi::c_void);
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            let _ = &cc;
            Ok(Box::new(app))
        }),
    );
    match native {
        Ok(()) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}

/// The window icon, generated from the same shape the tray draws.
fn window_icon() -> std::sync::Arc<egui::IconData> {
    std::sync::Arc::new(egui::IconData {
        rgba: app_icon::rgba(app_icon::ICON_SIZE),
        width: app_icon::ICON_SIZE,
        height: app_icon::ICON_SIZE,
    })
}

fn build_app(
    args: &cli::Cli,
    rt: &tokio::runtime::Runtime,
    ipc_server: desktop::single_instance::IpcServer,
) -> Result<ui::App> {
    let settings = config::Settings::load().unwrap_or_default();

    // Credentials: the CLI/environment beats the keyring, where a registration
    // stores whatever app belongs to this user. Fastcloud ships none of its own
    // (see `auth`), so a fresh install has none — and rather than pretending
    // otherwise, `ui::App` shows the connect screen until it does.
    let creds = args
        .client_id
        .clone()
        .map(|id| {
            auth::AppCredentials::new(id, args.client_secret.clone().unwrap_or_default(), None)
        })
        .filter(|c| !c.client_secret.is_empty())
        .or_else(auth::AppCredentials::discover);
    // Demo mode is asked for, not fallen into: without credentials there is
    // nothing to browse *or* fake, and the connect screen says so. `--demo` is
    // still the way to explore the interface offline.
    let demo = args.demo;

    let api_client = std::sync::Arc::new(api::ApiClient::new(
        creds.as_ref().map(|c| c.client_id.clone()),
        demo,
    ));

    // Bring up a session when there is an application to do it with: resume the
    // signed-in one, else app-only so public browsing and playback work before
    // anyone signs in. With no credentials there is nothing to bring up, and
    // `ui::App` gates the interface on that.
    let session = if demo || creds.is_none() {
        None
    } else {
        rt.block_on(auth::start(api_client.clone()))
    };
    match &session {
        Some(session) => log::info!(
            "api ready ({})",
            match rt.block_on(session.grant()) {
                Some(auth::Grant::User) => "signed in",
                Some(auth::Grant::App) => "public only",
                None => "no token",
            }
        ),
        None if demo => log::info!("demo library (no network)"),
        None => log::info!("no application registered yet; showing the connect screen"),
    }

    let paths = config::app_paths()?;
    let art = images::ArtLoader::new(paths.cover_cache.clone(), rt.handle().clone());
    let audio_cache = std::sync::Arc::new(audio::cache::AudioCache::new(
        paths.audio_cache.clone(),
        512 * 1024 * 1024,
    )?);

    let output = audio::output::AudioOutput::open(settings.eq_gains_db, settings.volume)
        .context("open audio output")?;
    let player = std::sync::Arc::new(Player::new(
        std::sync::Arc::new(output),
        api_client.clone(),
        audio_cache.clone(),
        config::settings_path()?,
        rt.handle().clone(),
    ));
    player.attach();
    player.set_autoplay(settings.autoplay);
    // The volume reaches the output through `AudioOutput::open`; the balance
    // has no such argument, so it is applied here — otherwise a session left
    // panned came back centred.
    player.restore_controls(settings.volume, settings.balance, settings.mono);

    // Restore the persisted queue (paused; first Play resumes the stream).
    // Demo mode seeds its own synthesized queue below instead.
    if !demo && !settings.last_queue.is_empty() {
        player.restore_session(settings.last_queue.clone(), settings.last_queue_idx);
    }

    // Player engine loop.
    let player_loop = player.clone();
    rt.spawn(player_loop.run());

    // Cache quota loop.
    rt.spawn(audio::cache::quota_task(
        audio_cache.clone(),
        std::time::Duration::from_secs(300),
    ));

    // Demo mode: seed the queue with synthesized tracks (local PCM, no network).
    if demo {
        player.set_demo(true);
        let tracks = demo::demo_tracks();
        // Resume last session when the saved track still exists in the demo set.
        let (start_idx, resume_ms) = match (
            settings.last_track_urn.as_deref(),
            settings.last_position_ms,
        ) {
            (Some(urn), Some(ms)) => (tracks.iter().position(|t| t.urn() == urn).unwrap_or(0), ms),
            _ => (0, 0),
        };
        // Only the first window is synthesized; `Player::run` tops it up.
        let (pcm, rate) = demo::pcm_window(&tracks[start_idx], resume_ms);
        {
            let mut st = player.state.lock();
            st.queue = tracks;
            st.order = (0..st.queue.len()).collect();
            st.current = Some(start_idx);
            st.duration_ms = st.queue[start_idx].effective_duration_ms();
            st.position_ms = resume_ms;
            st.loading = false;
            st.is_playing = false;
        }
        player
            .output_handle()
            .start_track(pcm, rate, false, resume_ms);
    }

    // A new process always starts paused, even when an OS media session or a
    // persisted queue remembers that the previous process had been playing.
    // Autoplay still controls what happens after a user-started track ends.
    player.start_paused();

    // IPC from CLI.
    let (ipc_tx, ipc_rx) = crossbeam_channel::unbounded();
    ipc_server.spawn(ipc_tx, Some(player.clone()));

    let (tray, tray_rx) = match desktop::tray::Tray::spawn() {
        Ok((tray, rx)) => (Some(tray), Some(rx)),
        Err(e) => {
            log::warn!("tray unavailable: {e}");
            (None, None)
        }
    };
    let media = {
        #[cfg(target_os = "windows")]
        {
            // Windows SMTC needs the HWND; created in the eframe creation callback.
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            Some(desktop::media::MediaIntegration::new())
        }
    };
    let hotkeys = desktop::hotkeys::Hotkeys::register_defaults().ok();

    // The live catalogue: every list the views draw, fetched on demand and
    // cached (see `store`). Demo mode leaves it empty and draws `demo`.
    let store = store::Store::new(api_client.clone(), rt.handle().clone());
    if let Some(session) = &session {
        store.set_signed_in(rt.block_on(session.grant()) == Some(auth::Grant::User));
    }

    Ok(ui::App::new(
        player,
        settings,
        demo,
        media,
        hotkeys,
        Some(ipc_rx),
        tray,
        tray_rx,
        art,
        rt.handle().clone(),
        store,
        session,
    ))
}
