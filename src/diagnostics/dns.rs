use std::net::{IpAddr, ToSocketAddrs};

use crate::config::DnsCheckResult;

pub fn check_domain_dns(
    domain: &str,
    expected_ipv4: &str,
    expected_ipv6: &str,
) -> (DnsCheckResult, Option<anyhow::Error>) {
    check_domain_dns_with(domain, expected_ipv4, expected_ipv6, lookup_ips)
}

fn lookup_ips(domain: &str) -> Result<Vec<IpAddr>, anyhow::Error> {
    // System resolver (its own timeout applies).
    let addrs = format!("{domain}:0")
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!(e))?
        .map(|a| a.ip())
        .collect::<Vec<_>>();
    Ok(addrs)
}

pub(crate) fn check_domain_dns_with<F>(
    domain: &str,
    expected_ipv4: &str,
    expected_ipv6: &str,
    resolve: F,
) -> (DnsCheckResult, Option<anyhow::Error>)
where
    F: FnOnce(&str) -> Result<Vec<IpAddr>, anyhow::Error>,
{
    let mut result = DnsCheckResult {
        domain: domain.to_string(),
        expected_ipv4: expected_ipv4.to_string(),
        expected_ipv6: expected_ipv6.to_string(),
        last_checked_at: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    if domain.is_empty() {
        return (result, Some(anyhow::anyhow!("domain is required")));
    }

    match resolve(domain) {
        Ok(addrs) => {
            for addr in addrs {
                match addr {
                    IpAddr::V4(v4) => result.resolved_a.push(v4.to_string()),
                    IpAddr::V6(v6) => {
                        // Unmap IPv4-mapped IPv6 if present conceptually.
                        if let Some(v4) = v6.to_ipv4_mapped() {
                            result.resolved_a.push(v4.to_string());
                        } else {
                            result.resolved_aaaa.push(v6.to_string());
                        }
                    }
                }
            }
            result.resolved_a.sort();
            result.resolved_aaaa.sort();
            result.ipv4_matches =
                expected_ipv4.is_empty() || contains_ip(&result.resolved_a, expected_ipv4);
            result.ipv6_matches =
                expected_ipv6.is_empty() || contains_ip(&result.resolved_aaaa, expected_ipv6);
            (result, None)
        }
        Err(err) => {
            // A failed lookup never proves that the expected address matches.
            result.ipv4_matches = false;
            result.ipv6_matches = false;
            (result, Some(err))
        }
    }
}

pub fn dns_matches(result: &DnsCheckResult) -> bool {
    result.ipv4_matches && result.ipv6_matches
}

pub fn split_expected_ips(value: &str) -> (String, String) {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => (v4.to_string(), String::new()),
        Ok(IpAddr::V6(v6)) => (String::new(), v6.to_string()),
        Err(_) => (String::new(), String::new()),
    }
}

fn contains_ip(values: &[String], expected: &str) -> bool {
    if expected.is_empty() {
        return true;
    }
    let Ok(expected_addr) = expected.parse::<IpAddr>() else {
        return false;
    };
    let expected_s = expected_addr.to_string();
    values.iter().any(|v| {
        v.parse::<IpAddr>()
            .map(|a| a.to_string() == expected_s)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn check_domain_dns_matches_ipv4() {
        let (result, err) = check_domain_dns_with(
            "example.com",
            "43.245.198.178",
            "",
            |_| Ok(vec![IpAddr::V4(Ipv4Addr::new(43, 245, 198, 178))]),
        );
        assert!(err.is_none());
        assert!(result.ipv4_matches);
        assert!(dns_matches(&result));
    }

    #[test]
    fn check_domain_dns_detects_mismatch() {
        let (result, err) = check_domain_dns_with(
            "example.com",
            "43.245.198.178",
            "",
            |_| Ok(vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))]),
        );
        assert!(err.is_none());
        assert!(!result.ipv4_matches);
        assert!(!dns_matches(&result));
    }

    #[test]
    fn check_domain_dns_marks_lookup_errors_as_unmatched() {
        let (result, err) = check_domain_dns_with("example.com", "", "", |_| {
            Err(anyhow::anyhow!("no such host"))
        });
        assert!(err.is_some());
        assert_eq!(result.domain, "example.com");
        assert!(!dns_matches(&result));
    }
}
