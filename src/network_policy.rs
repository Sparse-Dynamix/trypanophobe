//! Block loopback, link-local, cloud metadata, and other non-public targets (SSRF).

use std::net::IpAddr;
use std::str::FromStr;

use url::Url;

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "metadata",
    "metadata.google.internal",
    "instance-data",
    "instance-data.ec2.internal",
    "169.254.169.254",
    "169.254.170.2",
    "fd00:ec2::254",
];

pub fn url_blocked(url_str: &str) -> Option<&'static str> {
    let url = Url::parse(url_str).ok()?;
    let host = url.host_str()?;
    if host_blocked(host) {
        return Some("blocked_host");
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip_blocked(ip) {
            return Some("blocked_address");
        }
    }
    None
}

pub fn host_blocked(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    BLOCKED_HOSTS.iter().any(|&b| host == b)
        || host.ends_with(".localhost")
        || host.ends_with(".local")
}

pub fn ip_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_unspecified() || v4.is_loopback() {
                return true;
            }
            let [a, b, _, _] = v4.octets();
            if a == 127 {
                return true;
            }
            if a == 169 && b == 254 {
                return true;
            }
            if a == 10 {
                return true;
            }
            if a == 172 && (16..=31).contains(&b) {
                return true;
            }
            if a == 192 && b == 168 {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            if v6.is_unspecified() || v6.is_loopback() {
                return true;
            }
            if (v6.segments()[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            let o = v6.octets();
            if o[0] == 0xfd && o[1] == 0x00 && o[2] == 0x0e && o[3] == 0xc2 {
                return true;
            }
            false
        }
    }
}

pub fn address_blocked(addr: &str) -> bool {
    if let Ok(ip) = IpAddr::from_str(addr) {
        return ip_blocked(ip);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_imds_urls() {
        assert_eq!(
            url_blocked("http://169.254.169.254/latest/meta-data/"),
            Some("blocked_host")
        );
    }
}
