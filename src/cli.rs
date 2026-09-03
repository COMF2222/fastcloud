#![allow(dead_code)]

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "fastcloud",
    version,
    about = "Native SoundCloud client",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Start in demo mode with sample content (no credentials needed)
    #[arg(long)]
    pub demo: bool,

    /// Override client_id (takes precedence over saved settings)
    #[arg(long)]
    pub client_id: Option<String>,

    /// Override client_secret (takes precedence over saved settings)
    #[arg(long)]
    pub client_secret: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Send playback commands to a running instance
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    /// Set volume (0-100)
    Volume {
        volume: u8,
    },
    /// Bring the running window to front
    Show,
    /// Terminate running instance
    Quit,
}

/// Send a command to the running instance via IPC.
pub fn run_remote_command(cmd: Command) -> anyhow::Result<()> {
    use std::io::Write;
    let mut stream =
        std::net::TcpStream::connect(("127.0.0.1", crate::desktop::single_instance::IPC_PORT))?;
    let payload = serde_json::to_string(&IpcMessage::from(cmd))?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum IpcMessage {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Volume(u8),
    Show,
    Quit,
}

impl From<Command> for IpcMessage {
    fn from(cmd: Command) -> Self {
        match cmd {
            Command::Play => IpcMessage::Play,
            Command::Pause => IpcMessage::Pause,
            Command::Toggle => IpcMessage::Toggle,
            Command::Next => IpcMessage::Next,
            Command::Prev => IpcMessage::Prev,
            Command::Stop => IpcMessage::Stop,
            Command::Volume { volume } => IpcMessage::Volume(volume),
            Command::Show => IpcMessage::Show,
            Command::Quit => IpcMessage::Quit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_play() {
        let cli = Cli::try_parse_from(["fastcloud", "play"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Play)));
    }
}
