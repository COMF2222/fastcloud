#![allow(dead_code)]

use super::client::{API_ROOT, ApiClient};
use super::error::Result;
use super::models::Collection;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

/// Lazy pager over a linked-partitioned collection.
///
/// The first call to [`next_page`] applies `query` to `path`; subsequent calls
/// follow the `next_href` returned by the API verbatim.
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
            pending_query: Some(query),
            _marker: PhantomData,
        }
    }

    /// Fetch the next page. Returns an empty vec when exhausted.
    pub async fn next_page(&mut self) -> Result<Vec<T>> {
        let Some(href) = self.next.clone() else {
            return Ok(Vec::new());
        };
        let query = self.pending_query.take().unwrap_or_default();
        let pairs: Vec<(&str, String)> =
            query.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let coll: Collection<T> = self.client.get_url(&href, &pairs).await?;
        self.next = coll.next_href;
        Ok(coll.collection)
    }
}
