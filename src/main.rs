use anyhow::{Context, Result};
use clap::Parser;

mod api;
mod audio;
mod auth;
mod cli;
mod config;
mod demo;
mod desktop;
mod player;
mod ui;
mod util;

use crate::player::Player;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = cli::Cli::parse();

    // CLI subcommand: forward to a running instance and exit.
    if let Some(cmd) = &args.command {
        if desktop::single_instance::send_command(cmd).unwrap_or(false) {
            return Ok(());
        }
        // No running instance; a bare subcommand without one is a no-op exit.
        return Ok(());
    }

    // Enforce single GUI instance (binds the IPC port).
    let Some(ipc_server) = desktop::single_instance::ensure_single_instance() else {
        std::process::exit(0);
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("start tokio runtime")?;

    let mut app = build_app(&args, &rt, ipc_server)?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 560.0]),
        ..Default::default()
    };
    let native = eframe::run_native(
        "Fastcloud",
        options,
        Box::new(move |cc| {
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

fn build_app(
    args: &cli::Cli,
    rt: &tokio::runtime::Runtime,
    ipc_server: desktop::single_instance::IpcServer,
) -> Result<ui::App> {
    let settings = config::Settings::load().unwrap_or_default();
    let demo = args.demo || (settings.client_id.is_none() && !has_credentials_env());

    // Credentials priority: CLI/env > keyring (Settings dialog writes to keyring).
    let client_id = args
        .client_id
        .clone()
        .or_else(|| std::env::var("FASTCLOUD_CLIENT_ID").ok())
        .or_else(|| auth::AppCredentials::from_keyring().map(|c| c.client_id));

    let api_client = std::sync::Arc::new(api::ApiClient::new(client_id.clone(), demo));

    // Auth: ensure token when credentials exist (skip in demo).
    if !demo {
        if let Some(creds) =
            auth::AppCredentials::from_env().or_else(auth::AppCredentials::from_keyring)
        {
            match rt.block_on(auth::ensure_tokens(&api_client, &creds)) {
                Ok(_) => log::info!("authorized"),
                Err(e) => log::warn!("auth: {e}; continuing unauthenticated"),
            }
        }
    }

    let paths = config::app_paths()?;
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
    ));
    player.attach();

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
        let first_pcm = demo::demo_pcm_for(&tracks[start_idx]);
        let first_rate = 44_100;
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
        // Seek the preloaded PCM to the resumed position.
        let skip_frames = (resume_ms * first_rate as u64 / 1000) as usize * 2;
        let pcm = if skip_frames > 0 && skip_frames < first_pcm.len() {
            first_pcm[skip_frames..].to_vec()
        } else {
            first_pcm
        };
        player.output_handle().start_track(pcm, first_rate, false);
    }

    // IPC from CLI.
    let (ipc_tx, ipc_rx) = crossbeam_channel::unbounded();
    ipc_server.spawn(ipc_tx);

    let (tray, tray_rx) = match desktop::tray::Tray::spawn() {
        Ok((tray, rx)) => (Some(tray), Some(rx)),
        Err(e) => {
            log::warn!("tray unavailable: {e}");
            (None, None)
        }
    };
    let _ = tray;

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

    Ok(ui::App::new(
        player,
        settings,
        demo,
        media,
        hotkeys,
        Some(ipc_rx),
        tray_rx,
    ))
}

fn has_credentials_env() -> bool {
    std::env::var("FASTCLOUD_CLIENT_ID").is_ok() && std::env::var("FASTCLOUD_CLIENT_SECRET").is_ok()
}
