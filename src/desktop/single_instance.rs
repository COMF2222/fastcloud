#![allow(dead_code)]

//! One instance, and the channel the CLI talks to it over.
//!
//! On Linux the singleton is a **D-Bus name** (`rocks.fastcloud.Instance`),
//! not a socket: the session bus is the platform's own answer to "is it
//! already running", it needs no port, and a second launch that only wants to
//! raise the window or open a link does not have to reach a listener we own.
//! The name is requested with `DoNotQueue`, so losing the race means another
//! instance holds it. Transport verbs still ride the local socket below,
//! which every platform shares.
//!
//! Everywhere else the socket alone decides: binding
//! `127.0.0.1:41318` succeeds for exactly one process.

use crate::cli::{DeviceInfo, IpcMessage, IpcReply, NowPlayingInfo, repeat_name};
use crossbeam_channel::Sender;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// Port for single-instance enforcement + CLI IPC (auth callback uses 41317).
pub const IPC_PORT: u16 = 41318;
/// The D-Bus name that marks the running instance on Linux.
pub const INSTANCE_NAME: &str = "rocks.fastcloud.Instance";
const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Holds the platform's "I am the one instance" claim for as long as it lives.
///
/// On Linux that is the D-Bus name (dropped with the connection); elsewhere
/// the bound socket in [`IpcServer`] is the claim, and this is empty.
pub struct InstanceGuard {
    #[cfg(target_os = "linux")]
    _connection: Option<zbus::blocking::Connection>,
}

/// What the bus said about our claim on the instance name.
///
/// Mirrors `zbus::fdo::RequestNameReply` (which only exists on Linux) so the
/// decision below can be written and tested on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameClaim {
    /// We hold the name: this process is the one instance.
    PrimaryOwner,
    /// Someone else holds it.
    Taken,
    /// We already held it — a double claim, still ours.
    AlreadyOwner,
    /// The bus is unreachable or refused for a reason of its own. The socket
    /// decides instead; never lock the user out over a bus problem.
    Unavailable,
}

/// Whether a claim means this process may run the window.
pub fn owns_instance(claim: NameClaim) -> bool {
    match claim {
        NameClaim::PrimaryOwner | NameClaim::AlreadyOwner | NameClaim::Unavailable => true,
        NameClaim::Taken => false,
    }
}

/// Claim the D-Bus name. `None` means another instance already holds it.
#[cfg(target_os = "linux")]
fn claim_bus_name() -> Option<InstanceGuard> {
    use zbus::fdo::{RequestNameFlags, RequestNameReply};

    let connection = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(e) => {
            // No session bus (a bare TTY, a container): the socket decides.
            log::warn!("no session bus, falling back to the socket: {e}");
            return Some(InstanceGuard { _connection: None });
        }
    };
    // `DoNotQueue`: we want to know now whether we are the one instance, not
    // to be handed the name when the other one exits.
    let (claim, connection) = match connection
        .request_name_with_flags(INSTANCE_NAME, RequestNameFlags::DoNotQueue.into())
    {
        Ok(RequestNameReply::PrimaryOwner) => (NameClaim::PrimaryOwner, Some(connection)),
        Ok(RequestNameReply::AlreadyOwner) => (NameClaim::AlreadyOwner, Some(connection)),
        Ok(_) => (NameClaim::Taken, None),
        Err(e) => {
            log::warn!("cannot claim {INSTANCE_NAME}: {e}");
            (NameClaim::Unavailable, None)
        }
    };
    owns_instance(claim).then_some(InstanceGuard {
        _connection: connection,
    })
}

#[cfg(not(target_os = "linux"))]
fn claim_bus_name() -> Option<InstanceGuard> {
    Some(InstanceGuard {})
}

/// Send one message and wait for the reply. `Err` = no running instance.
pub fn send_request(msg: &IpcMessage) -> anyhow::Result<IpcReply> {
    send_request_to_port(msg, IPC_PORT)
}

fn send_request_to_port(msg: &IpcMessage, port: u16) -> anyhow::Result<IpcReply> {
    let stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    let mut stream = stream;
    let payload = serde_json::to_string(msg)?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(serde_json::from_str(line.trim())?)
}

/// Serve IPC on the singleton port. Transport verbs are forwarded to the
/// app channel; queries (`NowPlaying`/`Devices`) are answered here from the
/// player snapshot so the CLI never waits for a UI frame.
pub struct IpcServer {
    listener: TcpListener,
}

impl IpcServer {
    pub fn bind() -> std::io::Result<Self> {
        Self::bind_port(IPC_PORT)
    }

    fn bind_port(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        Ok(Self { listener })
    }

    pub fn spawn(self, tx: Sender<IpcMessage>, player: Option<Arc<crate::player::Player>>) {
        std::thread::spawn(move || {
            for stream in self.listener.incoming().flatten() {
                if let Ok(msg) = recv(&stream) {
                    let reply = match &msg {
                        IpcMessage::NowPlaying { raw } => {
                            let info = now_playing(player.as_deref());
                            let text = if *raw { info.raw() } else { info.human() };
                            IpcReply::Text(text)
                        }
                        IpcMessage::Devices { raw } => {
                            let devices = output_devices();
                            let text = if *raw {
                                serde_json::to_string(&devices).unwrap_or_else(|_| "[]".into())
                            } else {
                                crate::cli::format_devices_human(&devices)
                            };
                            IpcReply::Text(text)
                        }
                        _ => {
                            let _ = tx.send(msg);
                            IpcReply::Ok
                        }
                    };
                    let _ = write_reply(stream, &reply);
                }
            }
        });
    }
}

fn recv(stream: &TcpStream) -> std::io::Result<IpcMessage> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str(line.trim())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad ipc message"))
}

fn write_reply(stream: TcpStream, reply: &IpcReply) -> std::io::Result<()> {
    let mut stream = stream;
    let payload = serde_json::to_string(reply).map_err(|e| std::io::Error::other(e.to_string()))?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn now_playing(player: Option<&crate::player::Player>) -> NowPlayingInfo {
    let empty = NowPlayingInfo {
        state: "stopped".into(),
        title: String::new(),
        artist: String::new(),
        position_ms: 0,
        duration_ms: 0,
        volume: 0,
        shuffle: "off".into(),
        repeat: "off".into(),
        art_url: String::new(),
        liked: "unknown".into(),
        device: "local".into(),
    };
    let Some(player) = player else {
        return empty;
    };
    let st = player.state.lock();
    let Some(idx) = st.current else {
        return empty;
    };
    let Some(track) = st.queue.get(idx).cloned() else {
        return empty;
    };
    let state = if st.loading {
        "loading"
    } else if st.is_playing {
        "playing"
    } else {
        "paused"
    }
    .to_string();
    let liked = liked_status(&track);
    NowPlayingInfo {
        state,
        title: track.title.clone(),
        artist: track.artist().to_string(),
        position_ms: st.position_ms,
        duration_ms: st.duration_ms,
        volume: (st.volume.clamp(0.0, 1.0) * 100.0).round() as u8,
        shuffle: if st.shuffle { "on" } else { "off" }.into(),
        repeat: repeat_name(st.repeat).into(),
        art_url: track
            .artwork
            .as_ref()
            .and_then(|a| a.best())
            .unwrap_or("")
            .to_string(),
        liked,
        device: "local".into(),
    }
}

fn liked_status(track: &crate::api::models::Track) -> String {
    // The settings file is the source of truth for likes; small enough to
    // read per query without a cache.
    let path = crate::config::settings_path()
        .ok()
        .and_then(|p| crate::config::Settings::load_from(&p).ok());
    match path {
        Some(s) if s.liked_ids.contains(&track.id) => "yes".into(),
        Some(_) => "no".into(),
        None => "unknown".into(),
    }
}

fn output_devices() -> Vec<DeviceInfo> {
    // cpal 0.18 exposes no stable per-device names, and the engine always
    // follows the system default output — so report exactly that.
    // (If output selection lands, enumerate real edges here.)
    vec![DeviceInfo {
        id: "local".into(),
        name: "Default output".into(),
        kind: "local".into(),
        active: true,
    }]
}

/// Whether another fastcloud instance holds the IPC port.
pub fn is_already_running() -> bool {
    TcpStream::connect(("127.0.0.1", IPC_PORT)).is_ok()
}

/// Ensure only one GUI instance runs.
///
/// On Linux the D-Bus name is claimed first (the platform's own singleton),
/// then the socket the CLI talks over; elsewhere the socket is both. Either
/// claim failing means another instance is up, and this returns `None`.
/// The returned guard must be kept alive for the app's lifetime.
pub fn ensure_single_instance() -> Option<(IpcServer, InstanceGuard)> {
    let guard = claim_bus_name()?;
    match IpcServer::bind() {
        Ok(server) => Some((server, guard)),
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
        // Use an OS-assigned test port so this remains valid while the real
        // desktop app owns its production singleton port.
        let server = IpcServer::bind_port(0)?;
        let port = server.listener.local_addr()?.port();
        let (tx, rx) = crossbeam_channel::bounded(1);
        server.spawn(tx, None);

        // Transport verbs are forwarded…
        let reply = send_request_to_port(&IpcMessage::Next, port)?;
        assert!(matches!(reply, IpcReply::Ok));
        let msg = rx.recv_timeout(Duration::from_secs(2))?;
        assert!(matches!(msg, IpcMessage::Next));

        // …queries are answered without a player (empty state)…
        let reply = send_request_to_port(&IpcMessage::NowPlaying { raw: true }, port)?;
        match reply {
            IpcReply::Text(t) => assert!(t.starts_with("stopped\t")),
            other => panic!("unexpected reply: {other:?}"),
        }
        // …and never touch the app channel.
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

        // While the server holds the port a second bind must fail.
        assert!(IpcServer::bind_port(port).is_err());
        Ok(())
    }

    /// The window runs unless the bus says someone else holds the name; a bus
    /// that is missing or unhappy must never lock the user out.
    #[test]
    fn only_a_taken_name_stops_this_instance() {
        assert!(owns_instance(NameClaim::PrimaryOwner));
        assert!(owns_instance(NameClaim::AlreadyOwner));
        assert!(owns_instance(NameClaim::Unavailable));
        assert!(!owns_instance(NameClaim::Taken));
    }
}
