#![allow(dead_code)]

use crate::cli::IpcMessage;
use std::sync::mpsc;

/// Commands issued from the tray context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    PlayPause,
    Next,
    Prev,
    Quit,
}

impl TrayCommand {
    pub fn to_ipc(self) -> IpcMessage {
        match self {
            TrayCommand::Show => IpcMessage::Show,
            TrayCommand::PlayPause => IpcMessage::Toggle,
            TrayCommand::Next => IpcMessage::Next,
            TrayCommand::Prev => IpcMessage::Prev,
            TrayCommand::Quit => IpcMessage::Quit,
        }
    }
}

/// Tray icon + context menu. Non-Linux uses tray-icon; Linux relies on
/// MPRIS (souvlaki) + ksni tray service.
pub struct Tray {
    #[cfg(not(target_os = "linux"))]
    inner: Option<TrayInner>,
}

#[cfg(not(target_os = "linux"))]
struct TrayInner {
    _tray: tray_icon::TrayIcon,
    bindings: Vec<(tray_icon::menu::MenuId, TrayCommand)>,
}

impl Tray {
    #[cfg(not(target_os = "linux"))]
    pub fn spawn() -> anyhow::Result<(Self, mpsc::Receiver<TrayCommand>)> {
        use tray_icon::TrayIconBuilder;
        use tray_icon::menu::{Menu, MenuEvent, MenuItem};

        let menu = Menu::new();
        let show = MenuItem::new("Show Fastcloud", true, None);
        let toggle = MenuItem::new("Play/Pause", true, None);
        let next = MenuItem::new("Next", true, None);
        let prev = MenuItem::new("Previous", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let bindings = vec![
            (show.id().clone(), TrayCommand::Show),
            (toggle.id().clone(), TrayCommand::PlayPause),
            (next.id().clone(), TrayCommand::Next),
            (prev.id().clone(), TrayCommand::Prev),
            (quit.id().clone(), TrayCommand::Quit),
        ];
        menu.append_items(&[&show, &toggle, &next, &prev, &quit])?;

        let icon = tray_icon::Icon::from_rgba(app_icon_rgba(), 64, 64)?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Fastcloud")
            .with_icon(icon)
            .build()?;

        let (tx, rx) = mpsc::channel();
        let handler_bindings = bindings.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            for (id, cmd) in &handler_bindings {
                if *id == event.id {
                    let _ = tx.send(*cmd);
                }
            }
        }));

        Ok((
            Self {
                inner: Some(TrayInner {
                    _tray: tray,
                    bindings,
                }),
            },
            rx,
        ))
    }

    #[cfg(target_os = "linux")]
    pub fn spawn() -> anyhow::Result<(Self, mpsc::Receiver<TrayCommand>)> {
        let (tx, rx) = mpsc::channel();
        let service = ksni::TrayService::new(KsniTray { tx });
        service.spawn();
        Ok((Self {}, rx))
    }

    pub fn set_tooltip(&self, text: &str) {
        #[cfg(not(target_os = "linux"))]
        if let Some(inner) = &self.inner {
            let _ = inner._tray.set_tooltip(Some(text));
        }
        #[cfg(target_os = "linux")]
        let _ = text;
    }
}

#[cfg(target_os = "linux")]
struct KsniTray {
    tx: std::sync::mpsc::Sender<TrayCommand>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for KsniTray {
    fn id(&self) -> String {
        "fastcloud".into()
    }
    fn title(&self) -> String {
        "Fastcloud".into()
    }
    fn icon_name(&self) -> String {
        "fastcloud-tray".into()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Show Fastcloud".into(),
                activate: Box::new(|t: &Self| {
                    let _ = t.tx.send(TrayCommand::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Play/Pause".into(),
                activate: Box::new(|t: &Self| {
                    let _ = t.tx.send(TrayCommand::PlayPause);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                activate: Box::new(|t: &Self| {
                    let _ = t.tx.send(TrayCommand::Next);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                activate: Box::new(|t: &Self| {
                    let _ = t.tx.send(TrayCommand::Prev);
                }),
                ..Default::default()
            }
            .into(),
            MenuEntry::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &Self| {
                    let _ = t.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn app_icon_rgba() -> Vec<u8> {
    // 64x64 orange cloud, procedurally generated.
    let size = 64usize;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let cx = x as f32 / size as f32 - 0.5;
            let cy = y as f32 / size as f32 - 0.5;
            let inside = cloud_sdf(cx, cy) < 0.0;
            let px: [u8; 4] = if inside {
                [255, 85, 17, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&px);
        }
    }
    rgba
}

fn cloud_sdf(x: f32, y: f32) -> f32 {
    let circles = [
        (-0.18f32, 0.08f32, 0.20f32),
        (0.02, 0.14, 0.22),
        (0.20, 0.05, 0.17),
        (-0.05, -0.05, 0.18),
        (0.12, -0.05, 0.15),
    ];
    let mut d = f32::MAX;
    for (cx, cy, r) in circles {
        let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r;
        d = d.min(dist);
    }
    d.min(y - 0.14)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_rgba_size() {
        assert_eq!(app_icon_rgba().len(), 64 * 64 * 4);
    }

    #[test]
    fn cloud_covers_center() {
        assert!(cloud_sdf(0.0, 0.0) < 0.0);
        assert!(cloud_sdf(0.9, 0.9) > 0.0);
    }

    #[test]
    fn tray_commands_map() {
        assert!(matches!(
            TrayCommand::PlayPause.to_ipc(),
            IpcMessage::Toggle
        ));
    }
}
