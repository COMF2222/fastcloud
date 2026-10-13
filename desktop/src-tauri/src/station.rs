use crate::{
    api::{self, models::Track},
    wave,
};
use std::{collections::HashSet, sync::Arc};

pub fn seeds(tracks: &[Track], variation: u32) -> Vec<Track> {
    let mut ids = HashSet::new();
    let eligible: Vec<_> = tracks
        .iter()
        .filter(|track| {
            track.id > 0 && !track.is_blocked() && !track.is_snippet() && ids.insert(track.id)
        })
        .collect();
    if eligible.is_empty() {
        return Vec::new();
    }
    let count = eligible.len().min(16);
    let offset = variation as usize % eligible.len();
    (0..count)
        .map(|index| eligible[(offset + index * eligible.len() / count) % eligible.len()].clone())
        .collect()
}

fn mix(
    seeds: &[Track],
    pages: &[Vec<Track>],
    artist_station: bool,
    avoid: &HashSet<u64>,
    identities: &[wave::TrackIdentity],
) -> Vec<Track> {
    let own: HashSet<_> = seeds.iter().map(|track| performer(track)).collect();
    let mut candidates = Vec::new();
    // Preserve related endpoint rank and give every seed a fair share.
    for index in 0..pages.iter().map(Vec::len).max().unwrap_or(0) {
        for page in pages {
            if let Some(track) = page.get(index) {
                candidates.push(track.clone());
            }
        }
    }
    let mut primary = seeds.to_vec();
    if artist_station {
        primary.extend(
            candidates
                .iter()
                .filter(|track| own.contains(&performer(track)))
                .cloned(),
        );
        candidates.retain(|track| !own.contains(&performer(track)));
    }
    let mut seen = avoid.clone();
    let mut recordings = identities.to_vec();
    let mut result: Vec<Track> = Vec::new();
    let mut counts = std::collections::HashMap::<String, usize>::new();
    while result.len() < 24 && (!candidates.is_empty() || !primary.is_empty()) {
        let use_primary = result.len() % 3 == 2 && !primary.is_empty() || candidates.is_empty();
        let pick = |pool: &[Track], avoid_last: bool| {
            pool.iter().position(|track| {
                if track.is_blocked() || track.is_snippet() || seen.contains(&track.id) {
                    return false;
                }
                let key = performer(track);
                if counts.get(&key).copied().unwrap_or(0)
                    >= if artist_station && own.contains(&key) {
                        8
                    } else {
                        2
                    }
                {
                    return false;
                }
                if avoid_last && result.last().is_some_and(|last| performer(last) == key) {
                    return false;
                }
                let identity = wave::track_identity(track);
                !recordings
                    .iter()
                    .any(|previous| wave::same_recording(previous, &identity))
            })
        };
        let chosen = [
            (use_primary, true),
            (!use_primary, true),
            (use_primary, false),
            (!use_primary, false),
        ]
        .into_iter()
        .find_map(|(own, avoid_last)| {
            pick(if own { &primary } else { &candidates }, avoid_last).map(|index| (own, index))
        });
        if let Some((own, index)) = chosen {
            let pool = if own { &mut primary } else { &mut candidates };
            let track = pool.remove(index);
            seen.insert(track.id);
            recordings.push(wave::track_identity(&track));
            *counts.entry(performer(&track)).or_default() += 1;
            result.push(track);
        } else {
            break;
        }
    }
    result
}

fn performer(track: &Track) -> String {
    track
        .artist()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn query_seeds(seeds: &[Track], frontier: &[Track], variation: u32) -> Vec<Track> {
    let mut ids: HashSet<_> = seeds.iter().map(|track| track.id).collect();
    let recent: Vec<_> = frontier.iter().rev()
        .filter(|track| !track.is_blocked() && !track.is_snippet() && ids.insert(track.id))
        .take(2).cloned().collect();
    let count = seeds.len().min(if recent.is_empty() { 4 } else { 2 });
    let mut queries: Vec<_> = (0..count)
        .map(|index| seeds[(variation as usize * count + index) % seeds.len()].clone())
        .collect();
    queries.extend(recent);
    queries
}

pub async fn generate(
    client: Arc<api::ApiClient>,
    engine: &wave::Engine,
    seeds: &[Track],
    frontier: &[Track],
    kind: &str,
    variation: u32,
    avoid: &HashSet<u64>,
    identities: &[wave::TrackIdentity],
) -> Result<Vec<Track>, String> {
    let seeds: Vec<_> = engine.with_dislike_filter(|filter| {
        seeds
            .iter()
            .filter(|track| !filter.rejects(track))
            .cloned()
            .collect()
    });
    if seeds.is_empty() {
        return Err("No playable radio seeds".into());
    }
    let queries = engine.with_dislike_filter(|filter| {
        let frontier: Vec<_> = frontier.iter().filter(|track| !filter.rejects(track)).cloned().collect();
        query_seeds(&seeds, &frontier, variation)
    });
    let mut searches = tokio::task::JoinSet::new();
    for (index, track) in queries.iter().enumerate() {
        let client = client.clone();
        let id = track.id;
        searches.spawn(async move {
            let mut pager = api::endpoints::related_tracks(&client, id).await;
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(9), pager.next_page()).await;
            (index, result.ok().and_then(Result::ok).unwrap_or_default())
        });
    }
    let mut pages = vec![Vec::new(); queries.len()];
    while let Some(result) = searches.join_next().await {
        if let Ok((index, tracks)) = result {
            pages[index] = tracks;
        }
    }
    let own: HashSet<_> = seeds.iter().map(performer).collect();
    let other_count = |pages: &[Vec<Track>]| {
        pages
            .iter()
            .flatten()
            .filter(|track| !track.is_blocked() && !track.is_snippet()
                && !avoid.contains(&track.id) && !own.contains(&performer(track))
                && !identities.iter().any(|old| wave::same_recording(old, &wave::track_identity(track))))
            .map(performer)
            .collect::<HashSet<_>>()
            .len()
    };
    if other_count(&pages) < 3 {
        if let Some(user) = seeds[0].user.as_ref().filter(|user| {
            user.username
                .to_lowercase()
                .chars()
                .filter(|ch| ch.is_alphanumeric())
                .collect::<String>()
                == performer(&seeds[0])
        }) {
            let mut profiles = api::endpoints::related_users(&client, user.id).await;
            let related =
                tokio::time::timeout(std::time::Duration::from_secs(6), profiles.next_page())
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or_default();
            let mut jobs = tokio::task::JoinSet::new();
            for index in 0..related.len().min(3) {
                let user = &related[(variation as usize * 3 + index) % related.len()];
                let id = user.id;
                let client = client.clone();
                jobs.spawn(async move {
                    let mut tracks = api::endpoints::user_tracks(&client, id).await;
                    tokio::time::timeout(std::time::Duration::from_secs(6), tracks.next_page())
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or_default()
                });
            }
            while let Some(result) = jobs.join_next().await {
                if let Ok(tracks) = result { pages.push(tracks); }
            }
        }
        if other_count(&pages) < 3
            && let Some(genre) = seeds[0]
                .genre
                .as_deref()
                .filter(|genre| !genre.trim().is_empty())
        {
            let mut search = api::paginated::Pager::<Track>::new(
                client,
                "/tracks",
                vec![
                    ("genres".into(), genre.into()),
                    ("access".into(), "playable".into()),
                    ("limit".into(), "50".into()),
                ],
            );
            if let Ok(Ok(tracks)) =
                tokio::time::timeout(std::time::Duration::from_secs(6), search.next_page()).await
            {
                pages.push(tracks);
            }
        }
    }
    Ok(engine.with_dislike_filter(|filter| {
        let pages: Vec<_> = pages
            .into_iter()
            .map(|page| {
                page.into_iter()
                    .filter(|track| !filter.rejects(track))
                    .collect()
            })
            .collect();
        mix(&seeds, &pages, kind == "artist", avoid, identities)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: u64, performer: &str) -> Track {
        let mut track = crate::demo::demo_tracks()[0].clone();
        track.id = id;
        track.title = format!("Song {id}");
        track.publisher_metadata = None;
        track.user.as_mut().unwrap().username = performer.into();
        track.access = Some("playable".into());
        track.policy = None;
        track
    }
    #[test]
    fn refill_explores_new_tracks_and_keeps_the_original_source() {
        let seeds = vec![track(1, "Source"), track(2, "Source")];
        let mut preview = track(9, "Other"); preview.access = Some("preview".into());
        let queries = query_seeds(&seeds, &[track(3, "Related"), track(4, "Related"), track(4, "Related"), preview], 1);
        assert_eq!(queries.iter().map(|track| track.id).collect::<Vec<_>>(), vec![1,2,4,3]);
        assert_eq!(query_seeds(&seeds, &[], 0).len(), 2);
        assert!(query_seeds(&[], &[], 0).is_empty());
    }
    #[test]
    fn a_seen_seed_does_not_block_fresh_songs_when_only_one_artist_is_available() {
        let seed = track(1, "Artist");
        let result = mix(
            &[seed.clone()],
            &[vec![track(2, "Artist"), track(3, "Artist")]],
            false,
            &HashSet::from([1]),
            &[wave::track_identity(&seed)],
        );
        assert_eq!(result.len(), 2);
    }
    #[test]
    fn collection_seeds_cover_the_whole_collection_and_rotate() {
        let tracks: Vec<_> = (1..=100).map(|id| track(id, "Artist")).collect();
        let chosen = seeds(&tracks, 0);
        assert_eq!(chosen.len(), 16);
        assert!(chosen.iter().any(|track| track.id > 90));
        assert_ne!(chosen[0].id, seeds(&tracks, 1)[0].id);
    }
    #[test]
    fn artist_station_interleaves_artist_and_related_performers() {
        let own: Vec<_> = (1..=12).map(|id| track(id, "Seed artist")).collect();
        let related: Vec<_> = (20..=43)
            .map(|id| track(id, &format!("Other {id}")))
            .collect();
        let result = mix(&own, &[related], true, &HashSet::new(), &[]);
        assert_eq!(result.len(), 24);
        assert_ne!(result[0].artist(), "Seed artist");
        assert!(
            result
                .iter()
                .filter(|track| track.artist() == "Seed artist")
                .count()
                >= 6
        );
        assert!(
            result
                .windows(2)
                .all(|pair| pair[0].artist() != pair[1].artist())
        );
    }
    #[test]
    fn station_rejects_previews_reuploads_and_seen_tracks() {
        let seed = track(1, "Seed");
        let mut duplicate = track(2, "Seed");
        duplicate.title = seed.title.clone();
        let mut preview = track(3, "Other");
        preview.access = Some("preview".into());
        let result = mix(
            &[seed.clone()],
            &[vec![
                duplicate,
                preview,
                track(4, "Other"),
                track(5, "Fresh"),
            ]],
            false,
            &HashSet::from([4]),
            &[wave::track_identity(&seed)],
        );
        assert_eq!(
            result.iter().map(|track| track.id).collect::<Vec<_>>(),
            vec![5]
        );
    }
}
