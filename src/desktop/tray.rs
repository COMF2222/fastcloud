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
    #[cfg(target_os = "linux")]
    _inner: ksni::blocking::Handle<KsniTray>,
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

        let icon = tray_icon::Icon::from_rgba(
            crate::app_icon::rgba(crate::app_icon::ICON_SIZE),
            crate::app_icon::ICON_SIZE,
            crate::app_icon::ICON_SIZE,
        )?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .with_tooltip("Fastcloud")
            .with_icon(icon)
            .build()?;

        let (tx, rx) = mpsc::channel();
        let click_tx = tx.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                tray_icon::TrayIconEvent::DoubleClick {
                    button: tray_icon::MouseButton::Left,
                    ..
                }
            ) {
                let _ = click_tx.send(TrayCommand::Show);
            }
        }));
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
        use ksni::blocking::TrayMethods;

        let (tx, rx) = mpsc::channel();
        let handle = KsniTray { tx }.spawn()?;
        Ok((Self { _inner: handle }, rx))
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
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayCommand::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Play/Pause".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayCommand::PlayPause);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayCommand::Next);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayCommand::Prev);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_commands_map() {
        assert!(matches!(
            TrayCommand::PlayPause.to_ipc(),
            IpcMessage::Toggle
        ));
    }
}
