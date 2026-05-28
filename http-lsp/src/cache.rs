// In-memory cache of the last response for each (uri, line) so "Show" can
// re-display a request's last response without re-sending it.

use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use httpyac::Exchange;

#[derive(Debug, Clone)]
pub struct Cached {
    pub exchange: Arc<Exchange>,
    pub at: chrono::DateTime<chrono::Local>,
    /// Path to the temp file we last wrote this response into, if any.
    pub temp_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Default)]
pub struct ResponseCache {
    inner: DashMap<(Url, u32), Cached>,
}

impl ResponseCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, uri: Url, line: u32, exchange: Exchange) -> Cached {
        let cached = Cached {
            exchange: Arc::new(exchange),
            at: chrono::Local::now(),
            temp_path: None,
        };
        self.inner.insert((uri, line), cached.clone());
        cached
    }

    pub fn attach_temp_path(&self, uri: &Url, line: u32, path: std::path::PathBuf) {
        if let Some(mut entry) = self.inner.get_mut(&(uri.clone(), line)) {
            entry.temp_path = Some(path);
        }
    }

    pub fn get(&self, uri: &Url, line: u32) -> Option<Cached> {
        self.inner.get(&(uri.clone(), line)).map(|r| r.clone())
    }
}
