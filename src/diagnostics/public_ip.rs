use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

#[derive(Clone)]
pub struct PublicIpEndpoint {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

fn default_endpoints() -> Vec<PublicIpEndpoint> {
    vec![
        PublicIpEndpoint {
            url: "https://api.ipify.org".into(),
            headers: vec![],
        },
        PublicIpEndpoint {
            url: "https://ifconfig.me/ip".into(),
            headers: vec![],
        },
        PublicIpEndpoint {
            url: "https://icanhazip.com".into(),
            headers: vec![],
        },
        PublicIpEndpoint {
            url: "http://169.254.169.254/latest/meta-data/public-ipv4".into(),
            headers: vec![],
        },
        PublicIpEndpoint {
            url: "http://169.254.169.254/metadata/v1/interfaces/public/0/ipv4/address".into(),
            headers: vec![],
        },
        PublicIpEndpoint {
            url: "http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip".into(),
            headers: vec![("Metadata-Flavor".into(), "Google".into())],
        },
        PublicIpEndpoint {
            url: "http://169.254.169.254/metadata/instance/network/interface/0/ipv4/ipAddress/0/publicIpAddress?api-version=2021-02-01&format=text".into(),
            headers: vec![("Metadata".into(), "true".into())],
        },
    ]
}

pub fn detect_public_ip() -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()?;
    detect_public_ip_with(&client, &default_endpoints())
}

pub fn detect_public_ip_with(
    client: &reqwest::blocking::Client,
    endpoints: &[PublicIpEndpoint],
) -> anyhow::Result<String> {
    let mut last_err: Option<anyhow::Error> = None;
    for endpoint in endpoints {
        match fetch_public_ip(client, endpoint) {
            Ok(ip) => return Ok(ip),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no public IP endpoints configured")))
}

fn fetch_public_ip(
    client: &reqwest::blocking::Client,
    endpoint: &PublicIpEndpoint,
) -> anyhow::Result<String> {
    let mut req = client.get(&endpoint.url);
    for (k, v) in &endpoint.headers {
        req = req.header(k, v);
    }
    let resp = req.send()?;
    if !resp.status().is_success() {
        anyhow::bail!("{} returned {}", endpoint.url, resp.status());
    }
    let body = resp.text()?;
    let ip = body.trim().to_string();
    if !is_public_ip(&ip) {
        anyhow::bail!("{} returned non-public IP {ip:?}", endpoint.url);
    }
    Ok(ip)
}

pub fn is_public_ip(value: &str) -> bool {
    let Ok(addr) = value.parse::<IpAddr>() else {
        return false;
    };
    match addr {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

fn is_public_ipv4(addr: Ipv4Addr) -> bool {
    let raw = u32::from(addr);
    !addr.is_private()
        && !addr.is_loopback()
        && !addr.is_link_local()
        && !addr.is_unspecified()
        && !addr.is_broadcast()
        && !addr.is_documentation()
        // 0.0.0.0/8, RFC 6890.
        && raw & 0xff00_0000 != 0
        // 100.64.0.0/10, shared address space (RFC 6598).
        && raw & 0xffc0_0000 != 0x6440_0000
        // 192.0.0.0/24, IETF protocol assignments (RFC 6890).
        && raw & 0xffff_ff00 != 0xc000_0000
        // 198.18.0.0/15, benchmarking (RFC 2544).
        && raw & 0xffff_0000 != 0xc612_0000
        // 224.0.0.0/4, multicast; 240.0.0.0/4, reserved (RFC 6890).
        && raw & 0xe000_0000 != 0xe000_0000
}

fn is_public_ipv6(addr: Ipv6Addr) -> bool {
    if let Some(v4) = addr.to_ipv4_mapped() {
        return is_public_ipv4(v4);
    }
    let segments = addr.segments();
    !addr.is_loopback()
        && !addr.is_unspecified()
        && !addr.is_unique_local()
        && !addr.is_unicast_link_local()
        // 2001:db8::/32, documentation (RFC 3849).
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        // ff00::/8, multicast (RFC 4291).
        && segments[0] & 0xff00 != 0xff00
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_public_ip_rejects_special_use_ranges_and_accepts_public_addresses() {
        for value in [
            "",
            "not-an-ip",
            "0.1.2.3",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "192.0.0.1",
            "192.168.1.1",
            "198.18.0.1",
            "224.0.0.1",
            "240.0.0.1",
            "::",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "ff02::1",
        ] {
            assert!(!is_public_ip(value), "is_public_ip({value:?}) = true");
        }

        for value in ["1.1.1.1", "8.8.8.8", "2001:4860:4860::8888"] {
            assert!(is_public_ip(value), "is_public_ip({value:?}) = false");
        }
    }
}
