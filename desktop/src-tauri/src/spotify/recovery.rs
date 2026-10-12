use super::*;
use std::{io::Write, path::PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Part { pub id: u64, pub tracks: Vec<u64> }

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub collection: usize, pub part: usize, pub name: String,
    pub tracks: Vec<u64>, pub id: Option<u64>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Saved {
    pub owner: u64,
    pub source: String,
    pub collections: Vec<Collection>,
    pub matches: HashMap<String, Option<u64>>,
    pub parts: Vec<Vec<Part>>,
    pub pending: Option<Intent>,
    pub yandex_ids: Vec<String>,
    #[serde(default)]
    pub yandex_started: bool,
    pub done: bool,
    #[serde(default)]
    pub reports: Vec<Report>,
}

impl Import {
    pub fn new(root: PathBuf) -> Self { Self { root: Some(root), ..Self::default() } }

    pub(super) fn save_recovery(&self) -> Result<(), String> {
        let saved = self.saved.lock();
        let (Some(root), Some(saved)) = (&self.root, saved.as_ref()) else { return Ok(()) };
        let path = root.join(format!("{}.json", saved.owner));
        let temporary = path.with_extension("tmp");
        let backup = path.with_extension("bak");
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(root)?;
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&serde_json::to_vec(saved)?)?;
            file.sync_all()?;
            drop(file);
            if path.exists() {
                if backup.exists() { std::fs::remove_file(&backup)?; }
                std::fs::rename(&path, &backup)?;
            }
            if let Err(error) = std::fs::rename(&temporary, &path) {
                if backup.exists() { let _ = std::fs::rename(&backup, &path); }
                return Err(error);
            }
            Ok(())
        };
        write().map_err(|error| format!("Could not save import progress: {error}"))
    }

    pub(super) fn restore(&self, owner: u64) -> Result<(), String> {
        if owner == 0 { return Err("Wait for account synchronization".into()); }
        let mut view = self.view.lock();
        if view.running || self.loaded_owner.load(Ordering::Acquire) == owner { return Ok(()); }
        let mut restored = None;
        if let Some(root) = &self.root {
            for extension in ["json", "bak"] {
                let path = root.join(format!("{owner}.{extension}"));
                if !path.exists() { continue; }
                let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
                if bytes.len() > TOTAL_LIMIT as usize { continue; }
                if let Ok(value) = serde_json::from_slice::<Saved>(&bytes)
                    && value.owner == owner && (validate_plan(&value.collections).is_ok() || (value.source == "yandex" && value.collections.len() == 1 && value.collections[0].songs.is_empty()))
                    && value.parts.len() == value.collections.len()
                    && value.parts.iter().flatten().all(|part| part.tracks.len() <= 500)
                    && value.yandex_ids.len() <= TRACK_LIMIT
                    && value.pending.as_ref().is_none_or(|pending| pending.collection < value.collections.len() && pending.part <= value.parts[pending.collection].len() && pending.tracks.len() <= 500 && !pending.tracks.contains(&0))
                    && value.parts.iter().flatten().all(|part| part.id > 0)
                    && value.matches.values().all(|id| id.is_none_or(|id| id > 0)) {
                    restored = Some(value); break;
                }
            }
        }
        *self.saved.lock() = restored;
        *self.plan.lock() = None;
        *view = View::default();
        self.loaded_owner.store(owner, Ordering::Release);
        drop(view);
        self.refresh_recovery_view();
        Ok(())
    }

    pub(super) fn refresh_recovery_view(&self) {
        let saved = self.saved.lock().clone();
        let Some(saved) = saved.as_ref() else { return };
        let mut view = self.view.lock();
        if view.ready { return; }
        view.resumable = !saved.done;
        view.retry_missing = saved.matches.values().any(Option::is_none);
        view.needs_token = saved.source == "yandex" && (!saved.yandex_started || !saved.yandex_ids.is_empty());
        view.source = saved.source.clone();
        view.total = saved.collections.iter().map(|collection| collection.songs.len()).sum();
        view.collections = saved.collections.iter().enumerate().map(|(id, collection)| Summary {
            id, name: collection.name.clone(), kind: collection.kind,
            tracks: collection.songs.len(), skipped: collection.skipped,
        }).collect();
        if !view.running {
            view.current = 0; view.matched = 0; view.reports.clear();
            for (index, collection) in saved.collections.iter().enumerate() {
                let mut report = Report { name: collection.name.clone(), completed: saved.done, ..Report::default() };
                for song in &collection.songs {
                    if let Some(found) = saved.matches.get(&song.key()) {
                        view.current += 1;
                        if found.is_some() { view.matched += 1; report.matched += 1; }
                        else { report.not_found += 1; if report.missing.len() < 200 { report.missing.push(song.label()); } }
                    }
                }
                report.playlist_ids = saved.parts[index].iter().map(|part| part.id).collect();
                report.added = saved.parts[index].iter().map(|part| part.tracks.len()).sum();
                if collection.kind == Kind::Likes && let Some(previous) = saved.reports.get(index) { report.added = previous.added; report.already_liked = previous.already_liked; }
                view.reports.push(report);
            }
        }
    }
}

async fn tracks(context: &Context, id: u64) -> Result<Vec<u64>, String> {
    context.check()?;
    let owner = context.job.saved.lock().as_ref().map_or(0, |saved| saved.owner);
    let playlist = api::endpoints::playlist(&context.client, id).await.map_err(|error| error.to_string())?;
    if owner != 0 && playlist.user.as_ref().map(|user| user.id) != Some(owner) {
        return Err("Imported playlist belongs to another account".into());
    }
    let mut pager = api::endpoints::playlist_tracks(&context.client, id).await;
    let mut ids = Vec::new();
    while !pager.is_exhausted() {
        context.check()?;
        ids.extend(pager.next_page().await.map_err(|error| error.to_string())?.iter().map(|track| track.id));
        if ids.len() > 500 { return Err("Imported playlist was edited; its tracks have been kept".into()); }
    }
    Ok(ids)
}

pub(super) async fn reconcile(context: &Context) -> Result<(), String> {
    let pending = context.job.saved.lock().as_ref().and_then(|saved| saved.pending.clone());
    let Some(mut pending) = pending else { return Ok(()) };
    if pending.id.is_none() {
        let mut pager = api::endpoints::my_playlists(&context.client).await;
        let mut matches = Vec::new();
        while !pager.is_exhausted() {
            context.check()?;
            for playlist in pager.next_page().await.map_err(|error| error.to_string())? {
                if playlist.title == pending.name && tracks(context, playlist.id).await? == pending.tracks {
                    matches.push(playlist.id);
                }
            }
        }
        if matches.len() != 1 {
            return Err("Could not confirm the saved playlist. Check your SoundCloud playlists before starting a new import.".into());
        }
        pending.id = matches.first().copied();
        context.job.saved.lock().as_mut().unwrap().pending = Some(pending.clone());
        context.job.save_recovery()?;
    }
    let id = pending.id.unwrap();
    let remote = tracks(context, id).await?;
    if remote != pending.tracks {
        let previous = context.job.saved.lock().as_ref().unwrap().parts[pending.collection].get(pending.part).cloned();
        if previous.as_ref().map_or_else(|| !remote.iter().all(|id| pending.tracks.contains(id)), |part| part.tracks != remote) {
            return Err("Imported playlist was edited or saved incompletely; its tracks have been kept".into());
        }
        api::endpoints::update_playlist(&context.client, id, None, None, &pending.tracks).await.map_err(|error| error.to_string())?;
        if tracks(context, id).await? != pending.tracks { return Err("SoundCloud did not save all playlist tracks. Resume to check again.".into()); }
    }
    context.check()?;
    {
        let mut saved = context.job.saved.lock();
        let saved = saved.as_mut().unwrap();
        let part = Part { id, tracks: pending.tracks };
        if saved.parts[pending.collection].len() == pending.part { saved.parts[pending.collection].push(part); }
        else { saved.parts[pending.collection][pending.part] = part; }
        saved.pending = None;
    }
    context.job.save_recovery()?;
    context.store.invalidate(&Key::MyPlaylists);
    let parts = context.job.saved.lock().as_ref().unwrap().parts[pending.collection].clone();
    if let Some(report) = context.job.view.lock().reports.get_mut(pending.collection) {
        report.playlist_ids = parts.iter().map(|part| part.id).collect();
        report.added = parts.iter().map(|part| part.tracks.len()).sum();
    }
    Ok(())
}

pub(super) async fn save_parts(context: &Context, index: usize, name: &str, ids: &[u64], split: bool) -> Result<(), String> {
    reconcile(context).await?;
    for (part, chunk) in ids.chunks(500).enumerate() {
        context.check()?;
        let previous = context.job.saved.lock().as_ref().unwrap().parts[index].get(part).cloned();
        if let Some(previous) = &previous {
            let remote = tracks(context, previous.id).await?;
            if remote != previous.tracks { return Err("Imported playlist was edited; its tracks have been kept".into()); }
            if remote == chunk { continue; }
        }
        let title = if split { format!("{name} - part {}", part + 1) } else { name.into() };
        let pending = Intent { collection: index, part, name: title.clone(), tracks: chunk.to_vec(), id: previous.as_ref().map(|part| part.id) };
        context.job.saved.lock().as_mut().unwrap().pending = Some(pending);
        context.job.save_recovery()?;
        if previous.is_none() {
            let playlist = api::endpoints::create_playlist(&context.client, &title, false, chunk).await.map_err(|error| format!("Could not create playlist '{title}': {error}"))?;
            context.job.saved.lock().as_mut().unwrap().pending.as_mut().unwrap().id = Some(playlist.id);
            context.job.save_recovery()?;
        }
        reconcile(context).await?;
    }
    Ok(())
}
