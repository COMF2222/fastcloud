//! Optimistic playlist edits.
//!
//! SoundCloud's API has no incremental "add track" call and no snapshot id
//! (checked against the published OpenAPI spec): `PUT /playlists/{urn}`
//! **replaces** the whole ordered track list. Two things follow.
//!
//! *The screen cannot wait for the network.* An edit updates a local list
//! immediately, so a dropped song appears at once; the write goes out behind
//! it, and a failure puts the list back the way the server last confirmed it
//! and says so.
//!
//! *Writes must coalesce.* Dropping three songs in a row must not send three
//! full lists — the last one would win anyway, and a reply arriving out of
//! order could resurrect a removed track. At most one write per playlist is
//! in flight; whatever the user has done meanwhile is sent when it returns.
//!
//! [`Edits`] owns that bookkeeping. The pure decisions ([`plan`],
//! [`Entry::confirm`], [`Entry::roll_back`]) are separate from the async
//! plumbing so they can be tested without a server.

use crate::api::{ApiClient, endpoints};
use std::collections::HashMap;
use std::sync::Arc;

/// One playlist's edit state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What the interface shows: every edit applied.
    pub desired: Vec<u64>,
    /// The last list the server confirmed. A failed write returns here.
    pub confirmed: Vec<u64>,
    /// The list a write is carrying right now, if one is in flight.
    pub sending: Option<Vec<u64>>,
}

impl Entry {
    fn new(confirmed: Vec<u64>) -> Self {
        Self {
            desired: confirmed.clone(),
            confirmed,
            sending: None,
        }
    }

    /// Nothing pending and nothing in flight: the entry can be dropped.
    pub fn settled(&self) -> bool {
        self.sending.is_none() && self.desired == self.confirmed
    }

    /// A write succeeded. The sent list is now the server's truth.
    pub fn confirm(&mut self) {
        if let Some(sent) = self.sending.take() {
            self.confirmed = sent;
        }
    }

    /// A write failed: show what the server last confirmed, and drop the
    /// edits that were riding on it — keeping them would re-send a list the
    /// server already rejected.
    pub fn roll_back(&mut self) {
        self.sending = None;
        self.desired = self.confirmed.clone();
    }
}

/// The list to write now, or `None` while a write is in flight or there is
/// nothing new to say.
pub fn plan(entry: &Entry) -> Option<Vec<u64>> {
    if entry.sending.is_some() || entry.desired == entry.confirmed {
        return None;
    }
    Some(entry.desired.clone())
}

/// A finished write, handed back to the interface thread.
pub enum Done {
    Ok { playlist_id: u64 },
    Failed { playlist_id: u64, error: String },
}

/// Optimistic edits for every playlist touched this session.
pub struct Edits {
    lists: HashMap<u64, Entry>,
    tx: crossbeam_channel::Sender<Done>,
    rx: crossbeam_channel::Receiver<Done>,
}

impl Default for Edits {
    fn default() -> Self {
        Self::new()
    }
}

impl Edits {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            lists: HashMap::new(),
            tx,
            rx,
        }
    }

    /// The list to draw for `playlist_id`: the optimistic one when an edit is
    /// outstanding, else `None` so the caller uses whatever it fetched.
    pub fn view(&self, playlist_id: u64) -> Option<&[u64]> {
        self.lists.get(&playlist_id).map(|e| e.desired.as_slice())
    }

    /// Append a track, unless it is already there. `current` is the list as
    /// last read from the server, used the first time a playlist is touched.
    /// Returns `false` when the track is already in the list.
    pub fn add(&mut self, playlist_id: u64, current: &[u64], track_id: u64) -> bool {
        let entry = self
            .lists
            .entry(playlist_id)
            .or_insert_with(|| Entry::new(current.to_vec()));
        if entry.desired.contains(&track_id) {
            return false;
        }
        entry.desired.push(track_id);
        true
    }

    /// Drop a track. Returns `false` when it was not in the list.
    pub fn remove(&mut self, playlist_id: u64, current: &[u64], track_id: u64) -> bool {
        let entry = self
            .lists
            .entry(playlist_id)
            .or_insert_with(|| Entry::new(current.to_vec()));
        let before = entry.desired.len();
        entry.desired.retain(|id| *id != track_id);
        before != entry.desired.len()
    }

    /// Move a track from `from` to `to` (both indices into the shown list).
    pub fn reorder(&mut self, playlist_id: u64, current: &[u64], from: usize, to: usize) -> bool {
        let entry = self
            .lists
            .entry(playlist_id)
            .or_insert_with(|| Entry::new(current.to_vec()));
        if from >= entry.desired.len() || to > entry.desired.len() || from == to {
            return false;
        }
        let id = entry.desired.remove(from);
        let to = to.min(entry.desired.len());
        entry.desired.insert(to, id);
        true
    }

    /// Send whatever is outstanding. Call once a frame: it is cheap when
    /// there is nothing to do, and it re-sends after an in-flight write
    /// returns so late edits are not lost.
    pub fn flush(&mut self, client: &Arc<ApiClient>, rt: &tokio::runtime::Handle) {
        for (&playlist_id, entry) in self.lists.iter_mut() {
            let Some(tracks) = plan(entry) else {
                continue;
            };
            entry.sending = Some(tracks.clone());
            let client = client.clone();
            let tx = self.tx.clone();
            rt.spawn(async move {
                let done =
                    match endpoints::update_playlist(&client, playlist_id, None, None, &tracks)
                        .await
                    {
                        Ok(_) => Done::Ok { playlist_id },
                        Err(e) => Done::Failed {
                            playlist_id,
                            error: e.to_string(),
                        },
                    };
                let _ = tx.send(done);
            });
        }
    }

    /// Apply finished writes. Returns updated playlist ids (whose server
    /// cache must be invalidated) and one message per failure.
    pub fn poll(&mut self) -> (Vec<u64>, Vec<String>) {
        let mut updated = Vec::new();
        let mut errors = Vec::new();
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Done::Ok { playlist_id } => {
                    if let Some(entry) = self.lists.get_mut(&playlist_id) {
                        entry.confirm();
                    }
                    updated.push(playlist_id);
                }
                Done::Failed { playlist_id, error } => {
                    if let Some(entry) = self.lists.get_mut(&playlist_id) {
                        entry.roll_back();
                    }
                    errors.push(error);
                }
            }
        }
        self.lists.retain(|_, entry| !entry.settled());
        (updated, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(confirmed: &[u64]) -> Entry {
        Entry::new(confirmed.to_vec())
    }

    #[test]
    fn an_untouched_playlist_has_nothing_to_write() {
        let e = entry(&[1, 2, 3]);
        assert!(e.settled());
        assert_eq!(plan(&e), None);
    }

    #[test]
    fn an_edit_is_shown_before_it_is_written() {
        let mut edits = Edits::new();
        assert_eq!(edits.view(7), None, "untouched playlists draw themselves");
        assert!(edits.add(7, &[1, 2], 3));
        assert_eq!(edits.view(7), Some([1, 2, 3].as_slice()));
        // The same track twice is not an edit.
        assert!(!edits.add(7, &[1, 2], 3));
        assert_eq!(edits.view(7), Some([1, 2, 3].as_slice()));
    }

    #[test]
    fn removing_and_reordering_change_the_shown_list() {
        let mut edits = Edits::new();
        assert!(edits.remove(7, &[1, 2, 3], 2));
        assert_eq!(edits.view(7), Some([1, 3].as_slice()));
        assert!(!edits.remove(7, &[1, 2, 3], 99), "not in the list");

        let mut edits = Edits::new();
        assert!(edits.reorder(7, &[1, 2, 3], 0, 2));
        assert_eq!(edits.view(7), Some([2, 3, 1].as_slice()));
        assert!(!edits.reorder(7, &[1, 2, 3], 1, 1), "a no-op move");
        assert!(!edits.reorder(7, &[1, 2, 3], 9, 0), "out of range");
    }

    /// One write at a time: a second edit waits for the first to return,
    /// then goes out as one list. Three drops must not become three PUTs.
    #[test]
    fn writes_coalesce_instead_of_racing() {
        let mut e = entry(&[1]);
        e.desired = vec![1, 2];
        let first = plan(&e).expect("a write is due");
        assert_eq!(first, vec![1, 2]);
        e.sending = Some(first);

        // More edits while that write is on the wire.
        e.desired = vec![1, 2, 3];
        assert_eq!(plan(&e), None, "no second write while one is in flight");

        e.confirm();
        assert_eq!(e.confirmed, vec![1, 2]);
        let second = plan(&e).expect("the late edit still goes out");
        assert_eq!(second, vec![1, 2, 3]);
        e.sending = Some(second);
        e.confirm();
        assert!(e.settled());
        assert_eq!(plan(&e), None);
    }

    /// A rejected write puts the list back and forgets the edits that rode
    /// on it: re-sending them would only be rejected again.
    #[test]
    fn a_failed_write_rolls_back_to_the_server_list() {
        let mut e = entry(&[1, 2]);
        e.desired = vec![1, 2, 3];
        e.sending = plan(&e);
        e.roll_back();
        assert_eq!(e.desired, vec![1, 2]);
        assert!(e.settled());
        assert_eq!(plan(&e), None, "nothing is retried");
    }

    #[test]
    fn settled_entries_are_dropped_on_poll() {
        let mut edits = Edits::new();
        edits.add(7, &[1], 2);
        // Simulate the write finishing.
        let entry = edits.lists.get_mut(&7).unwrap();
        entry.sending = plan(entry);
        edits.tx.send(Done::Ok { playlist_id: 7 }).unwrap();
        let (updated, errors) = edits.poll();
        assert_eq!(updated, vec![7]);
        assert!(errors.is_empty());
        assert_eq!(edits.view(7), None, "the entry is gone once settled");
    }

    #[test]
    fn a_failure_is_reported_once() {
        let mut edits = Edits::new();
        edits.add(7, &[1], 2);
        let entry = edits.lists.get_mut(&7).unwrap();
        entry.sending = plan(entry);
        edits
            .tx
            .send(Done::Failed {
                playlist_id: 7,
                error: "403".into(),
            })
            .unwrap();
        let (updated, errors) = edits.poll();
        assert!(updated.is_empty());
        assert_eq!(errors, vec!["403".to_owned()]);
        assert_eq!(edits.view(7), None, "rolled back and dropped");
        let (updated, errors) = edits.poll();
        assert!(updated.is_empty());
        assert!(errors.is_empty());
    }
}
