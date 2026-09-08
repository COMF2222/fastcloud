#![allow(dead_code)]

use super::client::{API_ROOT, ApiClient};
use super::error::Result;
use super::models::Collection;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

/// The parameter that asks for a paginated collection.
///
/// Without it SoundCloud answers a **bare array** and no `next_href`, which is
/// the deprecated shape in their spec — and which [`Collection`] cannot be
/// deserialised from directly. Every list in the app broke on exactly this, so
/// it is applied here rather than left to each caller to remember.
pub const PARTITIONING: (&str, &str) = ("linked_partitioning", "true");

/// Lazy pager over a linked-partitioned collection.
///
/// The first call to [`Pager::next_page`] applies `query` to `path`; subsequent
/// calls follow the `next_href` returned by the API verbatim. `next_href`
/// already carries the query, so [`PARTITIONING`] is only added to the first
/// request.
pub struct Pager<T> {
    client: Arc<ApiClient>,
    next: Option<String>,
    pending_query: Option<Vec<(String, String)>>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned + Send + 'static> Pager<T> {
    pub fn new(client: Arc<ApiClient>, path: &str, query: Vec<(String, String)>) -> Self {
        Self {
            client,
            next: Some(format!("{}{}", API_ROOT, path)),
            pending_query: Some(with_partitioning(query)),
            _marker: PhantomData,
        }
    }

    /// Fetch the next page. Returns an empty vec when exhausted.
    pub async fn next_page(&mut self) -> Result<Vec<T>> {
        let Some(href) = self.next.clone() else {
            return Ok(Vec::new());
        };
        let query = self.pending_query.as_deref().unwrap_or_default();
        let pairs: Vec<(&str, String)> =
            query.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let coll: Collection<T> = self.client.get_url(&href, &pairs).await?;
        self.pending_query = None;
        self.next = coll.next_href;
        Ok(coll.collection)
    }

    /// Follow every `next_href`, preserving SoundCloud's server order.
    /// Library pages use this instead of silently stopping at the first 50.
    pub async fn collect_all(mut self) -> Result<Vec<T>> {
        let mut all = Vec::new();
        while self.next.is_some() {
            all.extend(self.next_page().await?);
        }
        Ok(all)
    }
}

/// Add [`PARTITIONING`] unless the caller set it, so a route that must not
/// paginate can still say so.
fn with_partitioning(mut query: Vec<(String, String)>) -> Vec<(String, String)> {
    let (key, value) = PARTITIONING;
    if !query.iter().any(|(k, _)| k == key) {
        query.push((key.to_owned(), value.to_owned()));
    }
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// The bug this module existed with for its whole life: without
    /// `linked_partitioning=true` the API answers a bare array, `Collection`
    /// fails to deserialise, and *every* list in the app reports an error.
    #[test]
    fn every_pager_asks_for_pagination() {
        let (key, value) = PARTITIONING;
        assert_eq!(key, "linked_partitioning");
        assert_eq!(value, "true");

        // An empty query still gets it.
        let applied = with_partitioning(Vec::new());
        assert_eq!(applied, q(&[("linked_partitioning", "true")]));

        // …and it is added alongside whatever the caller passed, not instead.
        let applied = with_partitioning(q(&[("q", "test"), ("access", "playable")]));
        assert_eq!(
            applied,
            q(&[
                ("q", "test"),
                ("access", "playable"),
                ("linked_partitioning", "true"),
            ])
        );
    }

    /// A caller that sets it explicitly keeps its own value: the only way to
    /// ask a route *not* to paginate.
    #[test]
    fn an_explicit_setting_is_not_overridden() {
        let applied = with_partitioning(q(&[("linked_partitioning", "false")]));
        assert_eq!(applied, q(&[("linked_partitioning", "false")]));
        // …and it is not duplicated, which would make the request ambiguous.
        let applied = with_partitioning(q(&[("linked_partitioning", "true"), ("q", "x")]));
        assert_eq!(
            applied
                .iter()
                .filter(|(k, _)| k == "linked_partitioning")
                .count(),
            1
        );
    }
}
