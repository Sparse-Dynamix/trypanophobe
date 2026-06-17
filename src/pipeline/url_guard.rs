use std::sync::Arc;

use moka::sync::Cache;

use crate::config::Config;
use crate::error::AppResult;
use crate::network_policy::url_blocked;
use crate::services::pihole::{parse_host, PiholeProbe};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlCheckOutcome {
    Allowed,
    Blocked {
        reason: String,
        detail: Option<String>,
    },
}

impl UrlCheckOutcome {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[derive(Clone)]
pub struct UrlGuard {
    pihole: Arc<PiholeProbe>,
    cache: Cache<String, UrlCheckOutcome>,
}

impl UrlGuard {
    pub fn new(cfg: &Config, pihole: Arc<PiholeProbe>) -> Arc<Self> {
        let cache = Cache::builder()
            .max_capacity(cfg.url_cache_capacity)
            .time_to_live(cfg.url_cache_ttl)
            .build();
        Arc::new(Self { pihole, cache })
    }

    pub async fn check_url(self: &Arc<Self>, url: &str) -> AppResult<UrlCheckOutcome> {
        if let Some(code) = url_blocked(url) {
            tracing::debug!(url, code, "url blocked by network policy");
            return Ok(UrlCheckOutcome::Blocked {
                reason: "URL blocked by network policy".into(),
                detail: Some(code.into()),
            });
        }

        let host = parse_host(url)?;
        if let Some(cached) = self.cache.get(&host) {
            return Ok(cached);
        }

        let result = self.pihole.probe_host(&host).await?;
        let outcome = if result.allowed {
            UrlCheckOutcome::Allowed
        } else {
            tracing::debug!(url, reason = ?result.reason, "url blocked by pi-hole");
            UrlCheckOutcome::Blocked {
                reason: "URL blocked by DNS blocklist".into(),
                detail: result.reason,
            }
        };
        self.cache.insert(host, outcome.clone());
        Ok(outcome)
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
        let outcome = guard
            .check_url("http://169.254.169.254/latest/meta-data/")
            .await
            .expect("check");
        assert_eq!(
            outcome,
            UrlCheckOutcome::Blocked {
                reason: "URL blocked by network policy".into(),
                detail: Some("blocked_host".into()),
            }
        );
    }
}
