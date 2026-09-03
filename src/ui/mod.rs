pub mod player_bar;
pub mod route;
pub mod sidebar;
pub mod theme;
pub mod views;
pub mod widgets;

use crate::player::Player;
use eframe::egui;
use std::sync::Arc;

pub struct App {
    pub player: Arc<Player>,
    pub route: route::Route,
    pub theme: theme::Theme,
    pub theme_mode: crate::config::ThemeMode,
    pub accent: egui::Color32,
    #[allow(dead_code)]
    pub me: Option<crate::api::models::Me>,
    pub settings: crate::config::Settings,
    pub demo: bool,
    pub search_query: String,
    pub toasts: Vec<(String, f64)>,
    pub media: Option<crate::desktop::media::MediaIntegration>,
    pub hotkeys: Option<crate::desktop::hotkeys::Hotkeys>,
    pub ipc_rx: Option<crossbeam_channel::Receiver<crate::cli::IpcMessage>>,
    pub tray_rx: Option<std::sync::mpsc::Receiver<crate::desktop::tray::TrayCommand>>,
    pub quit_requested: bool,
    pub version_build: &'static str,
}

impl App {
    /// Build the UI shell around an initialized player.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: Arc<Player>,
        settings: crate::config::Settings,
        demo: bool,
        media: Option<crate::desktop::media::MediaIntegration>,
        hotkeys: Option<crate::desktop::hotkeys::Hotkeys>,
        ipc_rx: Option<crossbeam_channel::Receiver<crate::cli::IpcMessage>>,
        tray_rx: Option<std::sync::mpsc::Receiver<crate::desktop::tray::TrayCommand>>,
    ) -> Self {
        let theme_mode = settings.theme;
        let theme = theme::Theme::from_mode(theme_mode, theme::ORANGE);
        Self {
            player,
            route: route::Route::Home,
            theme,
            theme_mode,
            accent: theme::ORANGE,
            me: None,
            settings,
            demo,
            search_query: String::new(),
            toasts: Vec::new(),
            media,
            hotkeys,
            ipc_rx,
            tray_rx,
            quit_requested: false,
            version_build: concat!("v", env!("CARGO_PKG_VERSION")),
        }
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), 0.0));
    }

    /// Bind OS media controls to the live window (Windows requires the HWND).
    #[cfg(target_os = "windows")]
    pub fn attach_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        if self.media.is_none() {
            self.media = Some(crate::desktop::media::MediaIntegration::new(hwnd));
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn attach_hwnd(&mut self) {}
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_ipc_and_tray();
        self.handle_hotkeys();
        self.poll_player_state();
        // Repaint at ~5 Hz so IPC commands, hotkeys and the seek bar stay
        // responsive even when paused or hidden.
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_egui_hotkeys(&ctx);

        self.theme.apply(&ctx);

        egui::Panel::bottom(egui::Id::new("player_bar")).show(ui, |ui| {
            player_bar::show(self, ui);
        });
        egui::Panel::left(egui::Id::new("sidebar"))
            .default_size(220.0)
            .show(ui, |ui| {
                sidebar::show(self, ui);
            });
        egui::CentralPanel::default_margins().show(ui, |ui| {
            views::show(self, ui);
        });

        self.show_toasts(&ctx);

        if self.quit_requested {
            self.player.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl App {
    fn handle_ipc_and_tray(&mut self) {
        let mut inbox: Vec<crate::cli::IpcMessage> = Vec::new();
        if let Some(rx) = &self.ipc_rx {
            while let Ok(msg) = rx.try_recv() {
                inbox.push(msg);
            }
        }
        if let Some(rx) = &self.tray_rx {
            while let Ok(cmd) = rx.try_recv() {
                inbox.push(cmd.to_ipc());
            }
        }
        if let Some(media) = &mut self.media {
            while let Ok(msg) = media.rx.try_recv() {
                inbox.push(msg);
            }
        }
        for msg in inbox {
            self.apply_ipc(msg);
        }
    }

    fn apply_ipc(&mut self, msg: crate::cli::IpcMessage) {
        use crate::cli::IpcMessage;
        match msg {
            IpcMessage::Play => self.player.play(),
            IpcMessage::Pause => self.player.pause(),
            IpcMessage::Toggle => self.player.play_pause(),
            IpcMessage::Next => {
                self.player.next();
            }
            IpcMessage::Prev => {
                self.player.prev();
            }
            IpcMessage::Stop => self.player.stop(),
            IpcMessage::Volume(v) => self.player.set_volume(v as f32 / 100.0),
            IpcMessage::Show => {
                // Handled at viewport level when supported.
            }
            IpcMessage::Quit => self.quit_requested = true,
        }
    }

    fn handle_hotkeys(&mut self) {
        let Some(hk) = &self.hotkeys else { return };
        let mut actions = Vec::new();
        hk.poll(&mut actions);
        for action in actions {
            self.apply_ipc(action.to_ipc());
        }
    }

    fn handle_egui_hotkeys(&mut self, ctx: &egui::Context) {
        let next = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight));
        let prev = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft));
        let space = ctx.input(|i| i.key_pressed(egui::Key::Space));
        let search = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F));
        if space {
            self.player.play_pause();
        }
        if next {
            self.player.next();
        }
        if prev {
            self.player.prev();
        }
        if search {
            self.route = crate::ui::route::Route::Search(String::new());
        }
    }

    fn poll_player_state(&mut self) {
        let st = self.player.state.lock();
        let current = st.current.and_then(|i| st.queue.get(i).cloned());
        let pos = st.position_ms;
        let playing = st.is_playing;
        drop(st);
        if let Some(track) = current {
            if let Some(media) = &mut self.media {
                media.update_metadata(
                    &track.title,
                    track.artist(),
                    track.artwork.as_ref().and_then(|a| a.best()),
                    track.effective_duration_ms() as i64,
                );
                media.update_playback(playing, pos);
            }
        }
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|(_, born)| now - born < 4.0);
        // Stamp new toasts with the current time.
        for (_, born) in self.toasts.iter_mut() {
            if *born == 0.0 {
                *born = now;
            }
        }
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -96.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(self.theme.surface)
                    .show(ui, |ui| {
                        for (msg, _) in self.toasts.iter() {
                            ui.label(egui::RichText::new(msg).color(self.theme.text));
                        }
                    });
            });
    }
}
