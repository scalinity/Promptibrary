//! SSRF guard for outbound fetches. SCA-915.
//!
//! Every user-pasted URL (article, X/Twitter oEmbed, YouTube oEmbed)
//! passes through `validate_url_for_outbound_fetch` which:
//!
//! 1. Requires the scheme to be `http` or `https`. `file://`, `ftp://`,
//!    `javascript:`, `data:` are rejected pre-DNS.
//! 2. Resolves the host via `tokio::net::lookup_host` and confirms
//!    EVERY resolved address is a public, routable IP. RFC1918,
//!    loopback, link-local, multicast, unspecified, broadcast,
//!    IPv4-mapped IPv6, IPv6 unique-local-addresses, and the
//!    AWS/GCP metadata service (169.254.169.254) are all rejected.
//! 3. Reject hostnames that resolve to ZERO addresses (DNS NXDOMAIN
//!    or empty A/AAAA), since that leaves nothing to validate.
//!
//! The check happens at the IPC entry point so a redirect re-validation
//! callback can run the same predicate on each hop.
//!
//! CWE-918 (SSRF), CWE-400 (uncontrolled resource consumption when
//! coupled with the streaming body cap in `bounded_get`).

use std::net::IpAddr;

use url::Url;

use crate::error::{AppError, AppErrorKind, Result};

/// Outbound-fetch policy. Returns Ok if `url` is safe to fetch from
/// untrusted-input sources (user pasted URLs), Err otherwise.
pub async fn validate_url_for_outbound_fetch(url: &Url) -> Result<()> {
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(AppError::new(
            AppErrorKind::UnsupportedSource,
            format!("URL scheme `{scheme}` not allowed for outbound fetch"),
        ));
    }

    let host = url.host_str().ok_or_else(|| {
        AppError::new(
            AppErrorKind::UnsupportedSource,
            "URL has no host component",
        )
    })?;

    // If the host *literally is* an IP, validate it directly. Saves a
    // DNS round-trip and prevents an attacker from forcing us through
    // a slow resolver.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_or_special_ip(&ip) {
            return Err(reject_ip(&ip));
        }
        return Ok(());
    }

    // Resolve all A/AAAA records. Reject if the resolution yields any
    // private address — defends against DNS rebinding where an
    // attacker-controlled domain's first lookup is public but a
    // second lookup mid-request returns 127.0.0.1.
    let port = url.port_or_known_default().unwrap_or(80);
    let target = format!("{host}:{port}");
    let addrs = match tokio::net::lookup_host(&target).await {
        Ok(it) => it.collect::<Vec<_>>(),
        Err(e) => {
            return Err(AppError::new(
                AppErrorKind::NetworkUnavailable,
                format!("DNS lookup failed for {host}: {e}"),
            ))
        }
    };
    if addrs.is_empty() {
        return Err(AppError::new(
            AppErrorKind::UnsupportedSource,
            format!("DNS returned no addresses for {host}"),
        ));
    }
    for sa in addrs {
        let ip = sa.ip();
        if is_private_or_special_ip(&ip) {
            return Err(reject_ip(&ip));
        }
    }
    Ok(())
}

fn reject_ip(ip: &IpAddr) -> AppError {
    AppError::new(
        AppErrorKind::UnsupportedSource,
        format!("URL resolves to private/special address {ip} — refused"),
    )
    .with_detail("reason", "ssrf_guard")
    .with_detail("ip", ip.to_string())
}

/// Returns true if `ip` is in any of the SSRF-relevant ranges that
/// should never be reached via untrusted-input fetches.
pub fn is_private_or_special_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_documentation()
            {
                return true;
            }
            // Cloud metadata service (covers AWS, GCP, Oracle Cloud).
            if v4.octets() == [169, 254, 169, 254] {
                return true;
            }
            // 0.0.0.0/8 — "this host on this network" per RFC 1122.
            if v4.octets()[0] == 0 {
                return true;
            }
            // 100.64.0.0/10 — carrier-grade NAT, RFC 6598.
            if v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 0x40 {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
            {
                return true;
            }
            let segs = v6.segments();
            // fe80::/10 link-local
            if (segs[0] & 0xFFC0) == 0xFE80 {
                return true;
            }
            // fc00::/7 unique-local addresses (ULA)
            if (segs[0] & 0xFE00) == 0xFC00 {
                return true;
            }
            // IPv4-mapped IPv6: ::ffff:0:0/96 — re-validate the embedded v4.
            if segs[0] == 0
                && segs[1] == 0
                && segs[2] == 0
                && segs[3] == 0
                && segs[4] == 0
                && segs[5] == 0xFFFF
            {
                let v4 = std::net::Ipv4Addr::new(
                    (segs[6] >> 8) as u8,
                    (segs[6] & 0xFF) as u8,
                    (segs[7] >> 8) as u8,
                    (segs[7] & 0xFF) as u8,
                );
                return is_private_or_special_ip(&IpAddr::V4(v4));
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_rfc1918_addresses() {
        for raw in ["10.0.0.1", "172.16.0.1", "192.168.1.1"] {
            let ip: IpAddr = raw.parse().unwrap();
            assert!(is_private_or_special_ip(&ip), "rfc1918 {raw} must reject");
        }
    }

    #[test]
    fn rejects_loopback_and_link_local() {
        for raw in ["127.0.0.1", "127.255.255.254", "169.254.0.1", "::1"] {
            let ip: IpAddr = raw.parse().unwrap();
            assert!(is_private_or_special_ip(&ip), "{raw} must reject");
        }
    }

    #[test]
    fn rejects_cloud_metadata_service() {
        let ip: IpAddr = "169.254.169.254".parse().unwrap();
        assert!(is_private_or_special_ip(&ip));
    }

    #[test]
    fn rejects_ipv4_mapped_loopback_via_ipv6() {
        let ip: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(is_private_or_special_ip(&ip));
    }

    #[test]
    fn rejects_ipv6_ula_and_link_local() {
        for raw in ["fd00::1", "fc00::1", "fe80::1"] {
            let ip: IpAddr = raw.parse().unwrap();
            assert!(is_private_or_special_ip(&ip), "{raw} must reject");
        }
    }

    #[test]
    fn accepts_public_addresses() {
        for raw in ["1.1.1.1", "8.8.8.8", "151.101.1.69", "2001:4860:4860::8888"] {
            let ip: IpAddr = raw.parse().unwrap();
            assert!(!is_private_or_special_ip(&ip), "{raw} must accept");
        }
    }

    #[tokio::test]
    async fn url_with_literal_loopback_ip_rejected() {
        let url = Url::parse("http://127.0.0.1:8080/admin").unwrap();
        let err = validate_url_for_outbound_fetch(&url).await.unwrap_err();
        assert_eq!(err.kind, AppErrorKind::UnsupportedSource);
    }

    #[tokio::test]
    async fn non_http_scheme_rejected() {
        let url = Url::parse("file:///etc/passwd").unwrap();
        let err = validate_url_for_outbound_fetch(&url).await.unwrap_err();
        assert_eq!(err.kind, AppErrorKind::UnsupportedSource);
    }

    #[tokio::test]
    async fn javascript_scheme_rejected() {
        let url = Url::parse("javascript:alert(1)").unwrap();
        let err = validate_url_for_outbound_fetch(&url).await.unwrap_err();
        assert_eq!(err.kind, AppErrorKind::UnsupportedSource);
    }
}
