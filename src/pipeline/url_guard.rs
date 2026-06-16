use std::sync::Arc;

use moka::sync::Cache;

use crate::config::Config;
use crate::error::AppResult;
use crate::network_policy::url_blocked;
use crate::services::pihole::{parse_host, PiholeProbe};

#[derive(Clone)]
pub struct UrlGuard {
    pihole: Arc<PiholeProbe>,
    cache: Cache<String, bool>,
}

impl UrlGuard {
    pub fn new(cfg: &Config, pihole: Arc<PiholeProbe>) -> Arc<Self> {
        let cache = Cache::builder()
            .max_capacity(cfg.url_cache_capacity)
            .time_to_live(cfg.url_cache_ttl)
            .build();
        Arc::new(Self { pihole, cache })
    }

    pub async fn check_url(self: &Arc<Self>, url: &str) -> AppResult<bool> {
        if let Some(code) = url_blocked(url) {
            tracing::debug!(url, code, "url blocked by network policy");
            return Ok(false);
        }

        let host = parse_host(url)?;
        if let Some(allowed) = self.cache.get(&host) {
            return Ok(allowed);
        }

        let result = self.pihole.probe_host(&host).await?;
        let allowed = result.allowed;
        self.cache.insert(host, allowed);
        if !allowed {
            tracing::debug!(url, reason = ?result.reason, "url blocked by pi-hole");
        }
        Ok(allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_imds_url() {
        let cfg = Config::from_env();
        let pihole = PiholeProbe::new(&cfg).expect("pihole");
        let guard = UrlGuard::new(&cfg, pihole);
        let allowed = guard
            .check_url("http://169.254.169.254/latest/meta-data/")
            .await
            .expect("check");
        assert!(!allowed);
    }
}
