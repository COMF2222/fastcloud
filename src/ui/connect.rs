//! Connecting an account, as a state machine.
//!
//! SoundCloud needs an application to issue tokens, and an application needs a
//! browser sign-in to create (see [`crate::auth::register`]). That is a
//! multi-second, multi-step, failable thing, and the interface has to say what
//! is happening at each point — so it lives here as one enum rather than as
//! several booleans on `App`.
//!
//! Nothing in this module blocks: the work runs on the tokio runtime and
//! reports back through a channel, which `App::poll_connect` drains.

/// Where the app stands with SoundCloud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    /// Looking for stored credentials, before the first frame has an answer.
    Starting,
    /// No application registered, so there is nothing to call. The way out is
    /// [`Connection::Pairing`].
    Disconnected,
    /// A pairing code is out: the user has to open the page and sign in.
    Pairing {
        /// The six-character code, shown in case the browser did not open.
        code: String,
        /// The page the user signs in on.
        url: String,
    },
    /// The user signed in; registering the application and minting a token.
    Registering,
    /// Credentials in hand and a token minted. `username` is `None` on an
    /// app-only session, which can still search and play public tracks.
    Connected { username: Option<String> },
    /// The account cannot register an application: SoundCloud requires an
    /// Artist Pro subscription for API access.
    NeedsArtistPro(String),
    /// Something else went wrong, in SoundCloud's own words where possible.
    Failed(String),
}

impl Connection {
    /// Whether a connection attempt is in flight, so the button can say so and
    /// a second press cannot start a second one.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Pairing { .. } | Self::Registering
        )
    }

    /// Whether the app has credentials and a token.
    pub fn connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// What went wrong, for the screen that has to show it.
    ///
    /// Used by the tests below to state the invariant, and by the connect
    /// screen through the pattern match it does anyway.
    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::NeedsArtistPro(message) | Self::Failed(message) => Some(message),
            _ => None,
        }
    }
}

/// A step of the connect flow, reported back to the interface thread.
///
/// `Registered` carries the credentials rather than storing them itself: the
/// keyring write and the session rebuild happen on the interface thread, in one
/// place, so a half-applied connection cannot exist.
pub enum ConnectStep {
    /// The code is out; show it and wait.
    Paired { code: String, url: String },
    /// The user signed in; the registration call is running.
    Registering,
    /// Credentials obtained. `existing` when the account already had an app.
    Registered {
        credentials: Box<crate::auth::AppCredentials>,
        existing: bool,
    },
    /// The account has no Artist Pro subscription.
    NeedsArtistPro(String),
    /// Anything else, already worded for a human.
    Failed(String),
    /// The user cancelled, so nothing should be said at all.
    Cancelled,
}

/// Run the whole flow, reporting each step through `tx`.
///
/// Split out of `App` so it can be a plain async function: it borrows nothing
/// from the interface and therefore cannot be tempted to touch it.
pub async fn connect(
    http: reqwest::Client,
    cancel: tokio::sync::watch::Receiver<bool>,
    tx: crossbeam_channel::Sender<ConnectStep>,
) {
    use crate::auth::register::{self, RegisterError};

    let pairing = match register::begin(&http).await {
        Ok(pairing) => pairing,
        Err(e) => {
            let _ = tx.send(step_of(e));
            return;
        }
    };
    // Open the browser after the code exists, so a failure to get one is not
    // reported *after* a tab has already opened.
    let url = pairing.url().to_owned();
    let _ = tx.send(ConnectStep::Paired {
        code: pairing.code.clone(),
        url: url.clone(),
    });
    let opened = url.clone();
    tokio::task::spawn_blocking(move || {
        if webbrowser::open(&opened).is_err() {
            log::warn!("could not open a browser; visit this URL to continue:\n{opened}");
        }
    });

    match register::finish(&http, &pairing, cancel).await {
        Ok(registered) => {
            let _ = tx.send(ConnectStep::Registering);
            let _ = tx.send(ConnectStep::Registered {
                credentials: Box::new(registered.credentials),
                existing: registered.existing,
            });
        }
        Err(RegisterError::Cancelled) => {
            let _ = tx.send(ConnectStep::Cancelled);
        }
        Err(e) => {
            let _ = tx.send(step_of(e));
        }
    }
}

/// Which step a registration error is. `NeedsArtistPro` is its own so the
/// screen can offer a subscription rather than a retry.
fn step_of(error: crate::auth::register::RegisterError) -> ConnectStep {
    use crate::auth::register::RegisterError;
    match error {
        RegisterError::NeedsArtistPro(message) => ConnectStep::NeedsArtistPro(message),
        RegisterError::Cancelled => ConnectStep::Cancelled,
        other => ConnectStep::Failed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three states that mean "wait" have to be exactly the three, or the
    /// connect button either double-fires or never re-enables.
    #[test]
    fn only_the_in_flight_states_are_busy() {
        assert!(Connection::Starting.busy());
        assert!(
            Connection::Pairing {
                code: "ABC123".into(),
                url: "https://secure.soundcloud.com/activate/ABC123".into(),
            }
            .busy()
        );
        assert!(Connection::Registering.busy());
        for settled in [
            Connection::Disconnected,
            Connection::Connected { username: None },
            Connection::NeedsArtistPro("no pro".into()),
            Connection::Failed("boom".into()),
        ] {
            assert!(!settled.busy(), "{settled:?} should not be busy");
        }
    }

    /// Only `Connected` is connected — in particular `Registering` is not, or
    /// the app would draw its library before it can fetch one.
    #[test]
    fn connected_is_only_the_connected_state() {
        assert!(
            Connection::Connected {
                username: Some("dj".into())
            }
            .connected()
        );
        assert!(Connection::Connected { username: None }.connected());
        for other in [
            Connection::Starting,
            Connection::Disconnected,
            Connection::Registering,
            Connection::Failed("x".into()),
        ] {
            assert!(!other.connected(), "{other:?} should not be connected");
        }
    }

    /// The two failure states carry a message to show; the rest carry none, so
    /// the screen cannot print an empty error box.
    #[test]
    fn only_failures_carry_a_message() {
        assert_eq!(
            Connection::NeedsArtistPro("needs pro".into()).error(),
            Some("needs pro")
        );
        assert_eq!(
            Connection::Failed("network".into()).error(),
            Some("network")
        );
        assert_eq!(Connection::Starting.error(), None);
        assert_eq!(Connection::Disconnected.error(), None);
        assert_eq!(Connection::Connected { username: None }.error(), None);
    }

    /// "You need Artist Pro" must stay its own step: as a plain failure the
    /// screen would offer a retry, and retrying cannot help.
    #[test]
    fn artist_pro_stays_distinct_from_other_failures() {
        use crate::auth::register::RegisterError;
        assert!(matches!(
            step_of(RegisterError::NeedsArtistPro("pro".into())),
            ConnectStep::NeedsArtistPro(message) if message == "pro"
        ));
        assert!(matches!(
            step_of(RegisterError::Cancelled),
            ConnectStep::Cancelled
        ));
        // Everything else keeps its own wording, whatever kind it was.
        for (error, expected) in [
            (RegisterError::CodeExpired, "expired"),
            (RegisterError::TimedOut, "timed out"),
            (RegisterError::Refused("nope".into()), "nope"),
        ] {
            match step_of(error) {
                ConnectStep::Failed(message) => {
                    assert!(
                        message.contains(expected),
                        "{message:?} does not mention {expected:?}"
                    );
                }
                other => panic!("wrong step for {expected}: {:?}", StepName(&other)),
            }
        }
    }

    /// `ConnectStep` has no `Debug` (it carries credentials), so tests name it
    /// this way rather than printing one by accident.
    struct StepName<'a>(&'a ConnectStep);

    impl std::fmt::Debug for StepName<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self.0 {
                ConnectStep::Paired { .. } => "Paired",
                ConnectStep::Registering => "Registering",
                ConnectStep::Registered { .. } => "Registered",
                ConnectStep::NeedsArtistPro(_) => "NeedsArtistPro",
                ConnectStep::Failed(_) => "Failed",
                ConnectStep::Cancelled => "Cancelled",
            })
        }
    }
}
