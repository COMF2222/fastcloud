//! Local fallback for discovery-by-feeling.
//!
//! The reference client performs vector search on its authenticated backend.
//! Fastcloud keeps the same interaction useful without sending listening data
//! to another service: several mood dimensions can match at once, their genre
//! windows are merged, then metadata relevance wins over raw popularity.

use std::collections::{HashMap, HashSet};

use crate::api::models::Track;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VibeProfile {
    pub label: String,
    pub genres: Vec<String>,
    pub exact_genre: Option<String>,
    terms: Vec<String>,
}

struct Preset {
    label: &'static str,
    keywords: &'static [&'static str],
    genres: &'static [&'static str],
}

const PRESETS: &[Preset] = &[
    Preset {
        label: "Night flow",
        keywords: &["night", "late", "midnight", "ноч", "полноч"],
        genres: &["ambient", "deep house", "synthwave"],
    },
    Preset {
        label: "Energy boost",
        keywords: &[
            "energy",
            "energetic",
            "workout",
            "running",
            "gym",
            "энерг",
            "трен",
            "бег",
            "качал",
        ],
        genres: &["drum & bass", "electronic", "rock"],
    },
    Preset {
        label: "Hyper rush",
        keywords: &["hyper", "speed", "rave", "гипер", "быстр", "рейв"],
        genres: &["hyperpop", "electronic", "drum & bass"],
    },
    Preset {
        label: "Calm focus",
        keywords: &[
            "calm",
            "chill",
            "focus",
            "relax",
            "study",
            "спокой",
            "чил",
            "фокус",
            "учеб",
            "работ",
        ],
        genres: &["ambient", "chillout", "downtempo"],
    },
    Preset {
        label: "After dark",
        keywords: &["dark", "industrial", "тём", "темн", "мрач"],
        genres: &["dark techno", "industrial", "ambient"],
    },
    Preset {
        label: "Melancholy",
        keywords: &[
            "sad",
            "melanch",
            "rain",
            "lonely",
            "груст",
            "меланх",
            "дожд",
            "одинок",
        ],
        genres: &["indie", "alternative", "ambient"],
    },
    Preset {
        label: "Good mood",
        keywords: &[
            "happy",
            "summer",
            "sunny",
            "fun",
            "весел",
            "летн",
            "солн",
            "радост",
        ],
        genres: &["pop", "dance", "house"],
    },
    Preset {
        label: "Party",
        keywords: &["party", "club", "dance", "вечерин", "клуб", "танц"],
        genres: &["house", "dance", "techno"],
    },
    Preset {
        label: "Slow romance",
        keywords: &[
            "love",
            "romance",
            "date",
            "romantic",
            "любов",
            "роман",
            "свидан",
        ],
        genres: &["r&b", "soul", "pop"],
    },
    Preset {
        label: "Sleep",
        keywords: &["sleep", "dream", "сон", "снов", "заснуть", "засып"],
        genres: &["ambient", "classical", "chillout"],
    },
    Preset {
        label: "Heavy pressure",
        keywords: &[
            "angry",
            "rage",
            "heavy",
            "aggressive",
            "злост",
            "злой",
            "ярост",
            "тяжел",
            "тяжёл",
        ],
        genres: &["metal", "hard rock", "hardcore"],
    },
];

pub fn profile(query: &str) -> VibeProfile {
    let normalized = query.trim().to_lowercase();
    let exact_genre = if is_genre_query(&normalized) {
        Some(normalized.clone())
    } else {
        None
    };
    let mut genre_scores: HashMap<&'static str, u32> = HashMap::new();
    let mut labels = Vec::new();

    for preset in PRESETS {
        if exact_genre.is_some() { break; }
        let matches = preset
            .keywords
            .iter()
            .filter(|keyword| normalized.contains(**keyword))
            .count() as u32;
        if matches == 0 {
            continue;
        }
        labels.push(preset.label);
        for (position, genre) in preset.genres.iter().enumerate() {
            *genre_scores.entry(genre).or_default() += matches * 10 + (3 - position as u32);
        }
    }

    let mut weighted_genres: Vec<_> = genre_scores.into_iter().collect();
    weighted_genres.sort_by(|(left_genre, left_score), (right_genre, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_genre.cmp(right_genre))
    });
    let mut genres: Vec<String> = weighted_genres
        .into_iter()
        .take(6)
        .map(|(genre, _)| genre.to_owned())
        .collect();
    if let Some(genre) = &exact_genre {
        genres.clear();
        genres.push(genre.clone());
    } else if genres.is_empty() {
        genres.push(normalized.clone());
    }

    let mut terms: Vec<String> = normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_owned)
        .collect();
    terms.extend(genres.iter().flat_map(|genre| {
        genre
            .split(|character: char| !character.is_alphanumeric())
            .filter(|term| term.chars().count() >= 3)
            .map(str::to_owned)
    }));
    terms.sort();
    terms.dedup();

    let label = match labels.as_slice() {
        [] => format!("{query} mix"),
        [one] => (*one).to_owned(),
        _ => labels.into_iter().take(2).collect::<Vec<_>>().join(" × "),
    };

    VibeProfile {
        label,
        genres,
        exact_genre,
        terms,
    }
}

fn genre_key(value: &str) -> String {
    let key = value.to_lowercase().replace('&', " and ");
    let words = key.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty() && *word != "and")
        .collect::<Vec<_>>().join(" ");
    match words.as_str() {
        "kpop" | "k pop" | "korean pop" => "k pop".into(),
        "hyper pop" | "hyperpop" => "hyperpop".into(),
        "dnb" | "drum bass" => "drum bass".into(),
        "r b" | "rhythm blues" => "r b".into(),
        "hiphop" | "hip hop" | "hip hop rap" => "hip hop rap".into(),
        "lofi" | "lo fi" => "lo fi".into(),
        _ => words,
    }
}

fn is_genre_query(query: &str) -> bool {
    const GENRES: &[&str] = &[
        "k-pop", "hyperpop", "electronic", "hip-hop & rap", "ambient", "alternative rock",
        "lo-fi", "house", "phonk", "r&b", "trap", "jazz", "techno", "indie", "soul",
        "drum & bass", "pop", "rock", "metal", "classical", "downtempo", "shoegaze",
        "brazilian funk", "funk", "deep house", "synthwave", "hardcore", "dance",
    ];
    GENRES.iter().any(|genre| genre_key(genre) == genre_key(query))
}

fn track_matches_genre(track: &Track, query: &str) -> bool {
    let wanted = genre_key(query);
    track.genre.as_deref().is_some_and(|genre| genre.split([',', '/', '|', ';']).any(|part| genre_key(part) == wanted))
        || track.genre.as_deref().unwrap_or_default().trim().is_empty()
            && track.tag_list.as_deref().is_some_and(|tags| {
                let detected: HashSet<_> = tags.split('"').enumerate().flat_map(|(index, tag)| {
                    if index % 2 == 1 { vec![tag] } else { tag.split_whitespace().collect() }
                }).filter(|tag| is_genre_query(tag)).map(genre_key).collect();
                detected.len() == 1 && detected.contains(&wanted)
            })
}

pub fn rank(profile: &VibeProfile, tracks: impl IntoIterator<Item = Track>) -> Vec<Track> {
    let mut seen = HashSet::new();
    let mut ranked: Vec<_> = tracks
        .into_iter()
        .filter(|track| seen.insert(track.id))
        .filter(|track| profile.exact_genre.as_deref().is_none_or(|genre| track_matches_genre(track, genre)))
        .map(|track| {
            let score = track_score(profile, &track);
            (track, score)
        })
        .filter(|(_, score)| *score > 0)
        .collect();
    ranked.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| {
                right
                    .likes_count
                    .or(right.favoritings_count)
                    .unwrap_or_default()
                    .cmp(
                        &left
                            .likes_count
                            .or(left.favoritings_count)
                            .unwrap_or_default(),
                    )
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().map(|(track, _)| track).collect()
}

fn track_score(profile: &VibeProfile, track: &Track) -> u32 {
    if profile.exact_genre.is_some() { return 100; }
    let genre = track.genre.as_deref().unwrap_or_default().to_lowercase();
    let title = track.title.to_lowercase();
    let artist = track.artist().to_lowercase();
    let description = track
        .description
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let mut score = profile
        .genres
        .iter()
        .enumerate()
        .filter(|(_, candidate)| genre.contains(candidate.as_str()))
        .map(|(position, _)| 18_u32.saturating_sub(position as u32 * 2))
        .sum();
    for term in &profile.terms {
        if genre.contains(term) {
            score += 7;
        }
        if title.contains(term) {
            score += 4;
        }
        if artist.contains(term) {
            score += 2;
        }
        if description.contains(term) {
            score += 1;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u64, title: &str, artist: &str, genre: &str, likes: u64) -> Track {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": title,
            "metadata_artist": artist,
            "genre": genre,
            "likes_count": likes
        }))
        .unwrap()
    }

    #[test]
    fn profile_recognizes_a_russian_night_vibe() {
        let result = profile("спокойная ночная музыка");

        assert_eq!(result.genres.first().map(String::as_str), Some("ambient"));
        assert!(result.genres.iter().any(|genre| genre == "deep house"));
        assert!(result.genres.iter().any(|genre| genre == "chillout"));
    }

    #[test]
    fn profile_combines_several_mood_dimensions() {
        let result = profile("dark energetic workout");

        assert!(result.genres.iter().any(|genre| genre == "drum & bass"));
        assert!(result.genres.iter().any(|genre| genre == "dark techno"));
        assert!(result.label.contains('×'));
    }

    #[test]
    fn profile_uses_the_query_as_a_genre_when_no_mood_matches() {
        let result = profile("shoegaze");

        assert_eq!(result.genres, ["shoegaze"]);
    }

    #[test]
    fn profile_maps_hyper_to_a_real_soundcloud_genre_window() {
        let result = profile("hyper");

        assert!(result.genres.iter().any(|genre| genre == "hyperpop"));
        assert!(result.genres.iter().any(|genre| genre == "electronic"));
    }

    #[test]
    fn rank_deduplicates_and_excludes_unrelated_music() {
        let profile = profile("energetic workout");
        let matching = track(1, "Run Faster", "Alice", "drum & bass", 10);
        let popular = track(2, "Still", "Bob", "ambient", 10_000);

        let result = rank(&profile, [popular, matching.clone(), matching.clone()]);

        assert_eq!(
            result.iter().map(|track| track.id).collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn exact_genres_do_not_bleed_into_neighbouring_styles() {
        for (query, matching) in [("Hyperpop", "hyper pop"), ("K-pop", "Kpop")] {
            let profile = profile(query);
            assert_eq!(profile.genres.len(), 1);
            let tracks = [
                track(1, "Funk hit", "A", "Brazilian funk", 1_000_000),
                track(2, "Right genre", "B", matching, 2),
            ];
            assert_eq!(rank(&profile, tracks).iter().map(|track| track.id).collect::<Vec<_>>(), [2]);
        }
    }

    #[test]
    fn tags_only_count_when_they_name_one_unambiguous_genre() {
        let matching: Track = serde_json::from_value(serde_json::json!({
            "id": 3, "title": "Song", "tag_list": "\"K-pop\" cover"
        })).unwrap();
        let ambiguous: Track = serde_json::from_value(serde_json::json!({
            "id": 4, "title": "Song", "tag_list": "\"K-pop\" \"Brazilian funk\""
        })).unwrap();
        assert_eq!(rank(&profile("K-pop"), [ambiguous, matching]).iter().map(|track| track.id).collect::<Vec<_>>(), [3]);
    }
}
