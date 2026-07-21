use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use rand::RngCore;
use std::collections::HashMap;
use std::path::Path;
use url::form_urlencoded;
use uuid::Uuid;

use crate::config;

/// Node represents one generated inbound share link (URI format subconverter understands).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Node {
    pub protocol: String,
    pub uri: String,
    pub remark: String,
}

fn random_short_id() -> String {
    let mut b = [0u8; 4];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return Uuid::new_v4().simple().to_string()[..8].to_string();
    }
    hex::encode(b)
}

fn random_pass() -> String {
    let mut b = [0u8; 16];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return Uuid::new_v4().to_string();
    }
    URL_SAFE_NO_PAD.encode(b)
}

fn random_ss2022_pass() -> String {
    let mut b = [0u8; 16];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        let s = Uuid::new_v4().to_string();
        return STANDARD.encode(&s.as_bytes()[..16]);
    }
    STANDARD.encode(b)
}

/// Generate a VLESS + Reality-Vision link.
pub fn generate_vless_reality(
    server_addr: &str,
    port: u16,
    domain_for_sni: &str,
    uuid_str: &str,
    public_key: &str,
    short_id: &str,
    remark: &str,
) -> String {
    let uuid_str = if uuid_str.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        uuid_str.to_string()
    };
    let short_id = if short_id.is_empty() {
        random_short_id()
    } else {
        short_id.to_string()
    };
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("type", "tcp")
        .append_pair("security", "reality")
        .append_pair("flow", "xtls-rprx-vision")
        .append_pair("pbk", public_key)
        .append_pair("fp", "chrome")
        .append_pair("sni", domain_for_sni)
        .append_pair("sid", &short_id)
        .append_pair("packetEncoding", "xudp")
        .finish();
    format!(
        "vless://{uuid_str}@{server_addr}:{port}?{query}#{}",
        urlencoding_path(remark)
    )
}

pub fn generate_hysteria2(
    server_addr: &str,
    port: u16,
    password: &str,
    sni: &str,
    remark: &str,
    insecure: bool,
) -> String {
    let password = if password.is_empty() {
        random_pass()
    } else {
        password.to_string()
    };
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("sni", sni)
        .append_pair("insecure", if insecure { "1" } else { "0" })
        .finish();
    format!(
        "hysteria2://{password}@{server_addr}:{port}/?{query}#{}",
        urlencoding_path(remark)
    )
}

pub fn generate_tuic(
    server_addr: &str,
    port: u16,
    uuid_str: &str,
    password: &str,
    sni: &str,
    alpn: &str,
    remark: &str,
) -> String {
    let uuid_str = if uuid_str.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        uuid_str.to_string()
    };
    let password = if password.is_empty() {
        random_pass()
    } else {
        password.to_string()
    };
    let alpn = if alpn.is_empty() { "h3" } else { alpn };
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("sni", sni)
        .append_pair("alpn", alpn)
        .append_pair("congestion_control", "bbr")
        .append_pair("udp_relay_mode", "native")
        .append_pair("allow_insecure", "0")
        .finish();
    format!(
        "tuic://{uuid_str}:{password}@{server_addr}:{port}/?{query}#{}",
        urlencoding_path(remark)
    )
}

pub fn generate_anytls(
    server_addr: &str,
    port: u16,
    password: &str,
    sni: &str,
    remark: &str,
) -> String {
    let password = if password.is_empty() {
        random_pass()
    } else {
        password.to_string()
    };
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("security", "tls")
        .append_pair("sni", sni)
        .append_pair("fp", "chrome")
        .finish();
    format!(
        "anytls://{password}@{server_addr}:{port}?{query}#{}",
        urlencoding_path(remark)
    )
}

pub fn generate_ss2022(
    server_addr: &str,
    port: u16,
    password: &str,
    method: &str,
    remark: &str,
) -> String {
    let password = if password.is_empty() {
        random_ss2022_pass()
    } else {
        password.to_string()
    };
    let method = if method.is_empty() {
        "2022-blake3-aes-128-gcm"
    } else {
        method
    };
    let userinfo = format!("{method}:{password}");
    let encoded = URL_SAFE_NO_PAD.encode(userinfo.as_bytes());
    format!(
        "ss://{encoded}@{server_addr}:{port}#{}",
        urlencoding_path(remark)
    )
}

fn urlencoding_path(s: &str) -> String {
    // Match Go's url.PathEscape for fragment remarks.
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

struct NodeSpec {
    proto: &'static str,
    node_protocol: &'static str,
    default_remark: &'static str,
}

const NODE_SPECS: &[NodeSpec] = &[
    NodeSpec {
        proto: "reality",
        node_protocol: "vless-reality",
        default_remark: "VLESS-Reality",
    },
    NodeSpec {
        proto: "hysteria2",
        node_protocol: "hysteria2",
        default_remark: "Hysteria2",
    },
    NodeSpec {
        proto: "tuic",
        node_protocol: "tuic",
        default_remark: "TUICv5",
    },
    NodeSpec {
        proto: "anytls",
        node_protocol: "anytls",
        default_remark: "AnyTLS",
    },
    NodeSpec {
        proto: "ss2022",
        node_protocol: "ss2022",
        default_remark: "SS2022",
    },
];

fn generate_node(
    spec: &NodeSpec,
    server_addr: &str,
    base_domain: &str,
    port: u16,
    cred: &HashMap<String, String>,
) -> Node {
    let remark = cred
        .get("remark")
        .filter(|s| !s.is_empty())
        .map(|s| s.as_str())
        .unwrap_or(spec.default_remark);
    let mut sni = cred
        .get("sni")
        .filter(|s| !s.is_empty())
        .map(|s| s.as_str())
        .unwrap_or(base_domain)
        .to_string();

    let uri = match spec.proto {
        "reality" => {
            if sni.is_empty() {
                sni = "www.microsoft.com".to_string();
            }
            let pbk = cred
                .get("pbk")
                .filter(|s| !s.is_empty())
                .map(|s| s.as_str())
                .unwrap_or("stub-public-key-replace-after-reality-gen");
            generate_vless_reality(
                server_addr,
                port,
                &sni,
                cred.get("uuid").map(|s| s.as_str()).unwrap_or(""),
                pbk,
                cred.get("short_id").map(|s| s.as_str()).unwrap_or(""),
                remark,
            )
        }
        "hysteria2" => generate_hysteria2(
            server_addr,
            port,
            cred.get("pass").map(|s| s.as_str()).unwrap_or(""),
            &sni,
            remark,
            false,
        ),
        "tuic" => generate_tuic(
            server_addr,
            port,
            cred.get("uuid").map(|s| s.as_str()).unwrap_or(""),
            cred.get("pass").map(|s| s.as_str()).unwrap_or(""),
            &sni,
            "h3",
            remark,
        ),
        "anytls" => generate_anytls(
            server_addr,
            port,
            cred.get("pass").map(|s| s.as_str()).unwrap_or(""),
            &sni,
            remark,
        ),
        "ss2022" => {
            let method = cred
                .get("method")
                .filter(|s| !s.is_empty())
                .map(|s| s.as_str())
                .unwrap_or("2022-blake3-aes-128-gcm");
            generate_ss2022(
                server_addr,
                port,
                cred.get("pass").map(|s| s.as_str()).unwrap_or(""),
                method,
                remark,
            )
        }
        _ => String::new(),
    };

    Node {
        protocol: spec.node_protocol.to_string(),
        remark: remark.to_string(),
        uri,
    }
}

/// Generate all top-5 nodes.
pub fn generate_all(
    server_addr: &str,
    base_domain: &str,
    ports: &HashMap<String, u16>,
    creds: &HashMap<String, HashMap<String, String>>,
) -> Vec<Node> {
    let server_addr = if server_addr.is_empty() {
        "127.0.0.1"
    } else {
        server_addr
    };
    let defaults = config::default_ports();
    NODE_SPECS
        .iter()
        .map(|spec| {
            let port = ports
                .get(spec.proto)
                .copied()
                .filter(|p| *p != 0)
                .or_else(|| defaults.get(spec.proto).copied())
                .unwrap_or(443);
            let empty = HashMap::new();
            let cred = creds.get(spec.proto).unwrap_or(&empty);
            generate_node(spec, server_addr, base_domain, port, cred)
        })
        .collect()
}

pub fn generate_selected(
    server_addr: &str,
    base_domain: &str,
    selected: &[String],
    ports: &HashMap<String, u16>,
    creds: &HashMap<String, HashMap<String, String>>,
) -> Vec<Node> {
    if selected.is_empty() {
        return generate_all(server_addr, base_domain, ports, creds);
    }
    let mut allowed = std::collections::HashSet::new();
    for protocol in selected {
        if protocol == "reality" {
            allowed.insert("vless-reality".to_string());
        } else {
            allowed.insert(protocol.clone());
        }
    }
    generate_all(server_addr, base_domain, ports, creds)
        .into_iter()
        .filter(|n| allowed.contains(&n.protocol))
        .collect()
}

pub fn write_nodes_file(path: &Path, nodes: &[Node]) -> anyhow::Result<()> {
    let mut content = String::new();
    for n in nodes {
        if !n.uri.is_empty() {
            content.push_str(&n.uri);
            content.push('\n');
        }
    }
    std::fs::write(path, content)?;
    // Match Go 0640 best-effort on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o640));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn load_nodes_from_file(path: &Path) -> anyhow::Result<Vec<String>> {
    let data = std::fs::read_to_string(path)?;
    let content = data.trim();
    if content.is_empty() {
        return Ok(vec![]);
    }
    Ok(content.lines().map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn generate_all_top5_formats() {
        let ports: HashMap<String, u16> = [
            ("reality", 443),
            ("hysteria2", 8443),
            ("tuic", 9443),
            ("anytls", 7443),
            ("ss2022", 8388),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let mut creds = HashMap::new();
        creds.insert(
            "reality".into(),
            [
                ("sni", "www.apple.com"),
                ("pbk", "testpbk1234567890abcdef"),
                ("short_id", "01234567"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        );
        for proto in ["hysteria2", "tuic", "anytls"] {
            creds.insert(
                proto.into(),
                [("sni".into(), "example.com".into())]
                    .into_iter()
                    .collect(),
            );
        }
        creds.insert("ss2022".into(), HashMap::new());

        let nodes = generate_all("1.2.3.4", "example.com", &ports, &creds);
        assert_eq!(nodes.len(), 5);

        for n in &nodes {
            match n.protocol.as_str() {
                "vless-reality" => {
                    assert!(n.uri.contains("security=reality"));
                    assert!(n.uri.contains("flow=xtls-rprx-vision"));
                    assert!(n.uri.contains("sni=www.apple.com"));
                }
                "hysteria2" => {
                    assert!(n.uri.starts_with("hysteria2://"));
                    assert!(n.uri.contains("sni=example.com"));
                }
                "tuic" => {
                    assert!(n.uri.starts_with("tuic://"));
                    assert!(n.uri.contains("sni=example.com"));
                }
                "anytls" => {
                    assert!(n.uri.starts_with("anytls://"));
                    assert!(n.uri.contains("security=tls"));
                }
                "ss2022" => assert!(n.uri.starts_with("ss://")),
                other => panic!("unexpected protocol {other}"),
            }
        }
    }

    #[test]
    fn write_nodes_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nodes.txt");
        let nodes = vec![
            Node {
                protocol: "hysteria2".into(),
                uri: "hysteria2://p@1.2.3.4:8443/?sni=ex.com#Hy2".into(),
                remark: "Hy2".into(),
            },
            Node {
                protocol: "vless-reality".into(),
                uri: "vless://u@1.2.3.4:443?security=reality#R".into(),
                remark: "R".into(),
            },
        ];
        write_nodes_file(&path, &nodes).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("hysteria2://"));
        assert!(content.contains("vless://"));
        let lines = load_nodes_from_file(&path).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn generate_selected_filters_protocols() {
        let ports: HashMap<String, u16> = [
            ("reality", 443),
            ("hysteria2", 8443),
            ("tuic", 9443),
            ("anytls", 7443),
            ("ss2022", 8388),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let mut creds = HashMap::new();
        creds.insert(
            "reality".into(),
            [
                ("sni", "www.apple.com"),
                ("pbk", "testpbk"),
                ("short_id", "01234567"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        );
        let nodes = generate_selected(
            "1.2.3.4",
            "example.com",
            &["reality".into(), "ss2022".into()],
            &ports,
            &creds,
        );
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].protocol, "vless-reality");
        assert_eq!(nodes[1].protocol, "ss2022");
    }
}
