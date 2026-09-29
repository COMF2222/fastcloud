//! Local taste summary derived from the genres in a listener's liked tracks.

use std::collections::HashMap;

use crate::api::models::Track;

#[derive(Debug, Clone, PartialEq)]
pub struct GenreShare {
    pub genre: String,
    pub tracks: usize,
    pub ratio: f32,
}

pub fn summarize(tracks: &[Track], limit: usize) -> Vec<GenreShare> {
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for genre in tracks
        .iter()
        .filter_map(|track| track.genre.as_deref())
        .map(str::trim)
        .filter(|genre| !genre.is_empty())
    {
        let key = genre.to_lowercase();
        let entry = counts.entry(key).or_insert_with(|| (genre.to_owned(), 0));
        entry.1 += 1;
    }
    let total: usize = counts.values().map(|(_, count)| count).sum();
    let mut shares: Vec<_> = counts
        .into_values()
        .map(|(genre, tracks)| GenreShare {
            genre,
            tracks,
            ratio: if total == 0 {
                0.0
            } else {
                tracks as f32 / total as f32
            },
        })
        .collect();
    shares.sort_by(|a, b| {
        b.tracks
            .cmp(&a.tracks)
            .then_with(|| a.genre.to_lowercase().cmp(&b.genre.to_lowercase()))
    });
    shares.truncate(limit);
    shares
}

/// Convert the visible shares into exact lane widths without forcing tiny
/// genres to steal space from the final segment.
pub fn lane_widths(shares: &[GenreShare], lane_width: f32) -> Vec<f32> {
    if shares.is_empty() || lane_width <= 0.0 {
        return Vec::new();
    }
    let visible_total: f32 = shares.iter().map(|share| share.ratio.max(0.0)).sum();
    if visible_total <= f32::EPSILON {
        return vec![0.0; shares.len()];
    }
    let mut used = 0.0;
    shares
        .iter()
        .enumerate()
        .map(|(index, share)| {
            let width = if index + 1 == shares.len() {
                (lane_width - used).max(0.0)
            } else {
                lane_width * share.ratio.max(0.0) / visible_total
            };
            used += width;
            width
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u64, genre: &str) -> Track {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": format!("Track {id}"),
            "genre": genre
        }))
        .unwrap()
    }

    #[test]
    fn summarize_orders_genres_by_the_number_of_likes() {
        let tracks = [track(1, "House"), track(2, "Techno"), track(3, "house")];

        let result = summarize(&tracks, 7);

        assert_eq!(result[0].genre, "House");
        assert_eq!(result[0].tracks, 2);
    }

    #[test]
    fn summarize_ignores_tracks_without_a_genre() {
        let tracks = [track(1, ""), track(2, "Ambient")];

        let result = summarize(&tracks, 7);

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn tiny_genres_do_not_overrun_the_soundprint_lane() {
        let shares = vec![
            GenreShare {
                genre: "Large".into(),
                tracks: 51,
                ratio: 0.51,
            },
            GenreShare {
                genre: "Medium".into(),
                tracks: 37,
                ratio: 0.37,
            },
            GenreShare {
                genre: "Small".into(),
                tracks: 8,
                ratio: 0.08,
            },
            GenreShare {
                genre: "Tiny".into(),
                tracks: 1,
                ratio: 0.01,
            },
        ];

        let widths = lane_widths(&shares, 1_000.0);

        assert!((widths.iter().sum::<f32>() - 1_000.0).abs() < 0.01);
        assert!(widths.iter().all(|width| *width >= 0.0));
        assert!(widths[3] < widths[2]);
    }

    #[test]
    fn truncated_shares_are_normalized_to_fill_the_lane() {
        let shares = vec![
            GenreShare {
                genre: "A".into(),
                tracks: 6,
                ratio: 0.6,
            },
            GenreShare {
                genre: "B".into(),
                tracks: 2,
                ratio: 0.2,
            },
        ];

        assert_eq!(lane_widths(&shares, 800.0), [600.0, 200.0]);
    }
}
