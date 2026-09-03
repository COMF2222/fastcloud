#![allow(dead_code)]

use crate::cli::{Cli, Command, IpcMessage};
use crossbeam_channel::Sender;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

/// Port for single-instance enforcement + CLI IPC (auth callback uses 41317).
pub const IPC_PORT: u16 = 41318;

/// Send the CLI intent to a running instance. Ok(true) = delivered.
pub fn send_command(cmd: &Command) -> std::io::Result<bool> {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", IPC_PORT)) else {
        return Ok(false);
    };
    let payload =
        serde_json::to_string(&IpcMessage::from(cmd.clone())).map_err(std::io::Error::other)?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut ack = [0u8; 32];
    let _ = stream.read(&mut ack)?;
    Ok(true)
}

/// If a CLI subcommand was given, forward it; returns Some(()) when handled.
pub fn try_command_existing(cli: &Cli) -> Option<()> {
    let cmd = cli.command.clone()?;
    match send_command(&cmd) {
        Ok(true) => Some(()),
        Ok(false) => None,
        Err(e) => {
            log::warn!("ipc send: {e}");
            None
        }
    }
}

/// Serve IPC on the singleton port; forward messages to the app channel.
pub struct IpcServer {
    listener: TcpListener,
}

impl IpcServer {
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", IPC_PORT))?;
        Ok(Self { listener })
    }

    pub fn spawn(self, tx: Sender<IpcMessage>) {
        std::thread::spawn(move || {
            for stream in self.listener.incoming().flatten() {
                if let Ok(msg) = recv(stream) {
                    let _ = tx.send(msg);
                }
            }
        });
    }
}

fn recv(stream: TcpStream) -> std::io::Result<IpcMessage> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str(line.trim())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad ipc message"))
}

/// Whether another fastcloud instance holds the IPC port.
pub fn is_already_running() -> bool {
    TcpStream::connect(("127.0.0.1", IPC_PORT)).is_ok()
}

/// Ensure only one GUI instance runs: bind or exit.
pub fn ensure_single_instance() -> Option<IpcServer> {
    match IpcServer::bind() {
        Ok(server) => Some(server),
        Err(_) => {
            eprintln!("fastcloud is already running (IPC port {IPC_PORT} in use).");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_bind_roundtrip_and_exclusive() -> anyhow::Result<()> {
        // Both checks in one test: cargo test runs tests in parallel threads,
        // and the IPC port is a singleton.
        let server = IpcServer::bind()?;
        let (tx, rx) = crossbeam_channel::bounded(1);
        server.spawn(tx);

        let mut stream = TcpStream::connect(("127.0.0.1", IPC_PORT))?;
        let payload = serde_json::to_string(&IpcMessage::Next)?;
        stream.write_all(payload.as_bytes())?;
        stream.write_all(b"\n")?;
        let msg = rx.recv_timeout(std::time::Duration::from_secs(2))?;
        assert!(matches!(msg, IpcMessage::Next));

        // While the server holds the port a second bind must fail.
        assert!(IpcServer::bind().is_err());
        Ok(())
    }
}
