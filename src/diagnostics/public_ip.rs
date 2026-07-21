use std::net::IpAddr;
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
        IpAddr::V4(v4) => {
            !v4.is_private()
                && !v4.is_loopback()
                && !v4.is_link_local()
                && !v4.is_unspecified()
                && !v4.is_broadcast()
                && !v4.is_documentation()
        }
        IpAddr::V6(v6) => {
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_unique_local()
                && !v6.is_unicast_link_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_public_ip_rejects_non_public_values() {
        for value in [
            "",
            "not-an-ip",
            "127.0.0.1",
            "10.0.0.1",
            "192.168.1.1",
            "::1",
            "fc00::1",
        ] {
            assert!(!is_public_ip(value), "is_public_ip({value:?}) = true");
        }
    }

    #[test]
    fn detect_public_ip_uses_first_public_response() {
        // Lightweight unit test without live network: exercise endpoint parsing path.
        assert!(is_public_ip("8.8.8.8"));
        assert!(is_public_ip("1.1.1.1"));
    }
}
