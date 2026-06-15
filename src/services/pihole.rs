use std::net::SocketAddr;
use std::sync::Arc;

use hickory_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;
use url::Url;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::network_policy::{address_blocked, host_blocked, ip_blocked};

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub host: String,
    pub allowed: bool,
    pub reason: Option<String>,
}

pub struct PiholeProbe {
    resolver: TokioAsyncResolver,
}

impl PiholeProbe {
    pub fn new(cfg: &Config) -> AppResult<Arc<Self>> {
        let addr: SocketAddr = cfg
            .pihole_dns
            .parse()
            .map_err(|e| AppError::Internal(format!("PIHOLE_DNS: {e}")))?;
        let ns = NameServerConfig {
            socket_addr: addr,
            protocol: Protocol::Udp,
            tls_dns_name: None,
            trust_negative_responses: true,
            bind_addr: None,
        };
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_name_server(ns);
        let resolver = TokioAsyncResolver::tokio(resolver_config, ResolverOpts::default());
        Ok(Arc::new(Self { resolver }))
    }

    pub async fn probe_host(self: &Arc<Self>, host: &str) -> AppResult<ProbeResult> {
        if host_blocked(host) {
            return Ok(blocked(host, "blocked_host"));
        }
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if ip_blocked(ip) {
                return Ok(blocked(host, "blocked_address"));
            }
        }

        let response = match self.resolver.lookup_ip(host).await {
            Ok(r) => r,
            Err(_) => return Ok(blocked(host, "dns_blocked")),
        };

        let addrs: Vec<String> = response.iter().map(|ip| ip.to_string()).collect();
        if addrs.is_empty() {
            return Ok(blocked(host, "dns_blocked"));
        }

        if addrs.iter().all(|a| !address_blocked(a)) {
            Ok(ProbeResult {
                host: host.to_string(),
                allowed: true,
                reason: None,
            })
        } else {
            Ok(blocked(host, "blocked_address"))
        }
    }

    pub async fn check_ready(cfg: &Config) -> bool {
        let Ok(probe) = Self::new(cfg) else {
            return false;
        };
        probe
            .probe_host(&cfg.pihole_ready_probe_host)
            .await
            .is_ok_and(|r| r.allowed)
    }
}

pub fn parse_host(url_str: &str) -> AppResult<String> {
    let url = Url::parse(url_str).map_err(|e| AppError::BadRequest(format!("invalid url: {e}")))?;
    url.host_str()
        .map(|h| h.to_string())
        .ok_or_else(|| AppError::BadRequest("url has no host".into()))
}

fn blocked(host: &str, reason: &str) -> ProbeResult {
    ProbeResult {
        host: host.to_string(),
        allowed: false,
        reason: Some(reason.into()),
    }
}
