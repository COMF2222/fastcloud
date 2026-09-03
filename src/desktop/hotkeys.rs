use crate::cli::IpcMessage;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// All player actions bindable to global hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    PlayPause,
    Next,
    Prev,
    Stop,
    VolumeUp,
    VolumeDown,
    Mute,
    Shuffle,
    Repeat,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 9] = [
        HotkeyAction::PlayPause,
        HotkeyAction::Next,
        HotkeyAction::Prev,
        HotkeyAction::Stop,
        HotkeyAction::VolumeUp,
        HotkeyAction::VolumeDown,
        HotkeyAction::Mute,
        HotkeyAction::Shuffle,
        HotkeyAction::Repeat,
    ];

    pub fn to_ipc(self) -> IpcMessage {
        match self {
            HotkeyAction::PlayPause => IpcMessage::Toggle,
            HotkeyAction::Next => IpcMessage::Next,
            HotkeyAction::Prev => IpcMessage::Prev,
            HotkeyAction::Stop => IpcMessage::Stop,
            HotkeyAction::VolumeUp => IpcMessage::Volume(100),
            HotkeyAction::VolumeDown | HotkeyAction::Mute => IpcMessage::Volume(0),
            HotkeyAction::Shuffle | HotkeyAction::Repeat => IpcMessage::Show,
        }
    }
}

/// Default binding: Ctrl+Alt+<key>.
pub fn default_binding(action: HotkeyAction) -> HotKey {
    let code = match action {
        HotkeyAction::PlayPause => Code::KeyP,
        HotkeyAction::Next => Code::ArrowRight,
        HotkeyAction::Prev => Code::ArrowLeft,
        HotkeyAction::Stop => Code::KeyS,
        HotkeyAction::VolumeUp => Code::ArrowUp,
        HotkeyAction::VolumeDown => Code::ArrowDown,
        HotkeyAction::Mute => Code::KeyM,
        HotkeyAction::Shuffle => Code::KeyH,
        HotkeyAction::Repeat => Code::KeyR,
    };
    HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), code)
}

pub struct Hotkeys {
    _manager: GlobalHotKeyManager,
    bindings: Vec<(u32, HotkeyAction)>,
}

impl Hotkeys {
    pub fn register_defaults() -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let mut bindings = Vec::new();
        for action in HotkeyAction::ALL {
            let hk = default_binding(action);
            match manager.register(hk) {
                Ok(()) => bindings.push((hk.id, action)),
                Err(global_hotkey::Error::AlreadyRegistered(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(Self {
            _manager: manager,
            bindings,
        })
    }

    /// Drain pending pressed hotkey events into actions.
    pub fn poll(&self, out: &mut Vec<HotkeyAction>) {
        out.clear();
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state() == HotKeyState::Pressed {
                if let Some((_, action)) = self.bindings.iter().find(|(id, _)| *id == event.id()) {
                    out.push(*action);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_actions_have_bindings() {
        for action in HotkeyAction::ALL {
            let hk = default_binding(action);
            assert_eq!(hk.mods, Modifiers::CONTROL | Modifiers::ALT);
        }
    }

    #[test]
    fn ipc_mapping() {
        assert!(matches!(
            HotkeyAction::PlayPause.to_ipc(),
            IpcMessage::Toggle
        ));
        assert!(matches!(HotkeyAction::Next.to_ipc(), IpcMessage::Next));
    }
}
