use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use subtle::ConstantTimeEq;
use tracing::info;
use url::Url;

use crate::config;

/// Cap on concurrently handled connections. Auth happens only after the
/// request is read, so without a cap a connection flood from anyone who can
/// reach the port spawns unbounded threads.
const MAX_CONNECTIONS: usize = 64;

/// Holds one slot of the connection budget; released on drop.
struct ConnSlot(Arc<AtomicUsize>);

impl ConnSlot {
    fn try_acquire(active: &Arc<AtomicUsize>) -> Option<Self> {
        if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(ConnSlot(Arc::clone(active)))
    }
}

impl Drop for ConnSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn run() -> anyhow::Result<()> {
    let port = std::env::var("SUB_MAKER_PORT").unwrap_or_else(|_| "8964".into());
    let token = std::env::var("SUB_MAKER_TOKEN").unwrap_or_default();
    if token.is_empty() || token == "changeme-in-config" {
        if config::demo_mode() {
            tracing::warn!("serving with a default/empty token in demo mode");
        } else {
            anyhow::bail!(
                "SUB_MAKER_TOKEN is unset or the insecure default; \
                 set it (the generated systemd unit does) or use SUB_MAKER_DEMO=1 for smoke tests"
            );
        }
    }
    let nodes_path = std::env::var("SUB_MAKER_NODES_FILE").unwrap_or_else(|_| {
        PathBuf::from(config::etc_base())
            .join("sub-maker")
            .join("nodes.txt")
            .to_string_lossy()
            .into()
    });

    let state = Arc::new(ServerState {
        token,
        nodes_path,
        port: port.clone(),
    });

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)?;
    info!(
        addr = %addr,
        nodes = %state.nodes_path,
        "starting sub-maker subscription server"
    );

    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "accept failed");
                continue;
            }
        };
        let Some(slot) = ConnSlot::try_acquire(&active) else {
            tracing::warn!("connection limit reached; dropping connection");
            continue; // dropping `stream` closes it
        };
        // A client that stalls must not wedge the whole server.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            let _slot = slot;
            let mut stream = stream;
            if let Err(e) = handle_connection(&mut stream, &state) {
                tracing::debug!(error = %e, "request handling error");
            }
        });
    }
    Ok(())
}

struct ServerState {
    token: String,
    nodes_path: String,
    port: String,
}

/// Constant-time token comparison over fixed-length digests.
fn token_matches(expected: &str, got: Option<&str>) -> bool {
    let mut h1 = Sha256::new();
    h1.update(expected.as_bytes());
    let mut h2 = Sha256::new();
    h2.update(got.unwrap_or_default().as_bytes());
    bool::from(h1.finalize().ct_eq(&h2.finalize()))
}

fn handle_connection(stream: &mut (impl Read + Write), state: &ServerState) -> anyhow::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let _method = parts.next().unwrap_or("GET");
    let path_and_query = parts.next().unwrap_or("/");

    let url = Url::parse(&format!("http://localhost{path_and_query}"))
        .unwrap_or_else(|_| Url::parse("http://localhost/").unwrap());
    let path = url.path().to_string();
    let query_token = url
        .query_pairs()
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.to_string());

    match path.as_str() {
        "/" => {
            let body = format!(
                "sub-maker on {}\nGET /sub?token=XXX  (Clash subscription with common rulesets)\nNodes source: {}\nAlso: /raw  (original node URIs)\n",
                state.port, state.nodes_path
            );
            write_response(stream, 200, "text/plain; charset=utf-8", &body, &[])?;
        }
        "/raw" => {
            if !token_matches(&state.token, query_token.as_deref()) {
                write_response(stream, 401, "text/plain", "unauthorized\n", &[])?;
                return Ok(());
            }
            let nodes = load_nodes(&state.nodes_path);
            if nodes.is_empty() {
                write_response(stream, 503, "text/plain", "no nodes configured\n", &[])?;
                return Ok(());
            }
            let body = format!("{}\n", nodes.join("\n"));
            write_response(stream, 200, "text/plain", &body, &[])?;
        }
        "/sub" => {
            if !token_matches(&state.token, query_token.as_deref()) {
                write_response(stream, 401, "text/plain", "unauthorized\n", &[])?;
                return Ok(());
            }
            let nodes = load_nodes(&state.nodes_path);
            if nodes.is_empty() {
                write_response(stream, 503, "text/plain", "no nodes configured\n", &[])?;
                return Ok(());
            }
            let (proxies_yaml, proxy_names) = nodes_to_clash_proxies_with_names(&nodes);
            let group_proxies = if proxy_names.is_empty() {
                "DIRECT".to_string()
            } else {
                proxy_names.join("\n      - ")
            };
            let body = format!(
                r#"mixed-port: 7890
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:9090

dns:
  enable: true
  nameserver:
    - 223.5.5.5

proxies:
{proxies_yaml}
proxy-groups:
  - name: "🔰 节点选择"
    type: select
    proxies:
      - {group_proxies}
      - DIRECT
  - name: "🎯 全球直连"
    type: select
    proxies:
      - DIRECT
      - "🔰 节点选择"

rules:
  # Common rulesets (via subconverter + ACL4SSR in full setups)
  - DOMAIN-SUFFIX,local,🎯 全球直连
  - GEOIP,CN,🎯 全球直连
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/LocalAreaNetwork.list,🎯 全球直连
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Google.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Netflix.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/YouTube.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Telegram.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/OpenAI.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/BanAD.list,REJECT
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/ChinaDomain.list,🎯 全球直连
  - MATCH,🔰 节点选择
"#
            );
            write_response(
                stream,
                200,
                "text/yaml; charset=utf-8",
                &body,
                &[
                    ("Profile-Update-Interval", "6"),
                    (
                        "Subscription-Userinfo",
                        "upload=0; download=0; total=0; expire=0",
                    ),
                ],
            )?;
        }
        _ => write_response(stream, 404, "text/plain", "not found\n", &[])?,
    }
    Ok(())
}

fn write_response(
    stream: &mut impl Write,
    status: u16,
    content_type: &str,
    body: &str,
    extra_headers: &[(&str, &str)],
) -> anyhow::Result<()> {
    // Note: Write alone is fine without parentheses.
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let mut headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        headers.push_str(&format!("{k}: {v}\r\n"));
    }
    headers.push_str("\r\n");
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(())
}

pub fn load_nodes(path: &str) -> Vec<String> {
    load_nodes_with_demo(path, config::demo_mode())
}

fn load_nodes_with_demo(path: &str, demo: bool) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(data) => {
            let out: Vec<String> = data
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|s| s.to_string())
                .collect();
            if out.is_empty() && demo {
                return demo_nodes();
            }
            out
        }
        Err(_) => {
            if demo {
                demo_nodes()
            } else {
                vec![]
            }
        }
    }
}

fn demo_nodes() -> Vec<String> {
    vec![
        "hysteria2://demo-pass@127.0.0.1:8443/?sni=example.com#Hy2".into(),
        "vless://demo-uuid@127.0.0.1:443?security=reality&fp=chrome&sni=www.apple.com&sid=0123abc&pbk=demo-pbk#Reality".into(),
    ]
}

/// Renders a YAML double-quoted scalar, escaping the values that would
/// otherwise break (or inject into) the generated Clash config.
fn yaml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn nodes_to_clash_proxies_with_names(nodes: &[String]) -> (String, Vec<String>) {
    let mut buf = String::new();
    let mut names = Vec::new();
    for n in nodes {
        let Ok(u) = Url::parse(n) else {
            continue;
        };
        let scheme = u.scheme();
        let host = u.host_str().unwrap_or("");
        let port = u.port().map(|p| p.to_string()).unwrap_or_else(|| "443".into());
        let remark = {
            let frag = u.fragment().unwrap_or("");
            if frag.is_empty() {
                scheme.to_string()
            } else {
                frag.to_string()
            }
        };
        let username = u.username();
        let password = u.password().unwrap_or("");
        let query = |key: &str| -> String {
            u.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.to_string())
                .unwrap_or_default()
        };

        let remark_q = yaml_quote(&remark);
        let host_q = yaml_quote(host);
        let sni_q = yaml_quote(&query("sni"));

        let line = match scheme {
            "hysteria2" => format!(
                "  - {{name: {remark_q}, type: hysteria2, server: {host_q}, port: {port}, password: {}, sni: {sni_q}, alpn: [h3], udp: true, skip-cert-verify: false}}\n",
                yaml_quote(username)
            ),
            "vless" => {
                let sec = query("security");
                if sec == "reality" {
                    format!(
                        "  - {{name: {remark_q}, type: vless, server: {host_q}, port: {port}, uuid: {}, network: tcp, tls: true, udp: true, flow: xtls-rprx-vision, servername: {sni_q}, packet-encoding: xudp, reality-opts: {{public-key: {}, short-id: {}}}, client-fingerprint: chrome}}\n",
                        yaml_quote(username),
                        yaml_quote(&query("pbk")),
                        yaml_quote(&query("sid"))
                    )
                } else {
                    format!(
                        "  - {{name: {remark_q}, type: vless, server: {host_q}, port: {port}, uuid: {}, tls: true, udp: true}}\n",
                        yaml_quote(username)
                    )
                }
            }
            "tuic" => {
                let (uuid, pass) = if password.is_empty() {
                    // user may be "uuid:pass" if password not split by Url parser
                    let parts: Vec<&str> = username.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (username.to_string(), String::new())
                    }
                } else {
                    (username.to_string(), password.to_string())
                };
                format!(
                    "  - {{name: {remark_q}, type: tuic, server: {host_q}, port: {port}, uuid: {}, password: {}, sni: {sni_q}, alpn: [h3], udp: true, udp-relay-mode: native, congestion-controller: bbr}}\n",
                    yaml_quote(&uuid),
                    yaml_quote(&pass)
                )
            }
            "anytls" => format!(
                "  - {{name: {remark_q}, type: anytls, server: {host_q}, port: {port}, password: {}, sni: {sni_q}, client-fingerprint: chrome, udp: true, alpn: [h2, http/1.1]}}\n",
                yaml_quote(username)
            ),
            "ss" => {
                let (cipher, password) = decode_ss_user_info(username);
                format!(
                    "  - {{name: {remark_q}, type: ss, server: {host_q}, port: {port}, cipher: {}, password: {}, udp: true}}\n",
                    yaml_quote(&cipher),
                    yaml_quote(&password)
                )
            }
            _ => format!(
                "  - {{name: {remark_q}, type: ss, server: {host_q}, port: {port}, cipher: \"aes-128-gcm\", password: \"demo\"}}\n"
            ),
        };
        if !line.is_empty() {
            buf.push_str(&line);
            names.push(remark_q);
        }
    }
    (buf, names)
}

fn decode_ss_user_info(encoded: &str) -> (String, String) {
    for engine in [URL_SAFE_NO_PAD, URL_SAFE, STANDARD_NO_PAD, STANDARD] {
        if let Ok(decoded) = engine.decode(encoded) {
            if let Ok(s) = String::from_utf8(decoded) {
                if let Some((cipher, password)) = s.split_once(':') {
                    if !cipher.is_empty() {
                        return (cipher.to_string(), password.to_string());
                    }
                }
            }
        }
    }
    ("2022-blake3-aes-128-gcm".into(), encoded.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conn_slots_are_bounded_and_released() {
        let active = Arc::new(AtomicUsize::new(0));
        let slots: Vec<_> = (0..MAX_CONNECTIONS)
            .map(|_| ConnSlot::try_acquire(&active).expect("slot under limit"))
            .collect();
        assert!(ConnSlot::try_acquire(&active).is_none());
        drop(slots);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(ConnSlot::try_acquire(&active).is_some());
    }

    #[test]
    fn token_matches_only_the_exact_token() {
        assert!(token_matches("s3cret", Some("s3cret")));
        assert!(!token_matches("s3cret", Some("s3cre")));
        assert!(!token_matches("s3cret", Some("")));
        assert!(!token_matches("s3cret", None));
        assert!(token_matches("", Some("")));
    }

    #[test]
    fn nodes_to_clash_proxies_uses_protocol_accurate_fields() {
        let nodes = vec![
            "vless://uuid@1.2.3.4:443?security=reality&flow=xtls-rprx-vision&fp=chrome&sni=www.apple.com&sid=0123abcd&pbk=pubkey#Reality".into(),
            "hysteria2://hy-pass@1.2.3.4:8443/?sni=example.com#Hy2".into(),
            "tuic://tuic-uuid:tuic-pass@1.2.3.4:9443/?sni=example.com&alpn=h3#TUIC".into(),
            "anytls://any-pass@1.2.3.4:7443?security=tls&sni=example.com#AnyTLS".into(),
            "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206MDEyMzQ1Njc4OWFiY2RlZg@1.2.3.4:8388#SS2022".into(),
        ];
        let (yaml, names) = nodes_to_clash_proxies_with_names(&nodes);
        assert_eq!(names.len(), nodes.len());
        for check in [
            "type: vless",
            "flow: xtls-rprx-vision",
            "packet-encoding: xudp",
            "servername: \"www.apple.com\"",
            "type: hysteria2",
            "alpn: [h3]",
            "type: tuic",
            "udp-relay-mode: native",
            "congestion-controller: bbr",
            "type: anytls",
            "cipher: \"2022-blake3-aes-128-gcm\"",
            "password: \"0123456789abcdef\"",
            "udp: true",
        ] {
            assert!(yaml.contains(check), "expected YAML to contain {check:?}:\n{yaml}");
        }
        assert!(!yaml.contains("type: trojan"));
    }

    #[test]
    fn nodes_to_clash_escapes_yaml_specials_and_ipv6_hosts() {
        let nodes = vec![
            // query_pairs percent-decodes: sni becomes a"b\c
            "hysteria2://p@1.2.3.4:8443/?sni=a%22b%5Cc#node1".into(),
            "ss://YWVzLTEyOC1nY206cA@[::1]:8388#v6".into(),
        ];
        let (yaml, names) = nodes_to_clash_proxies_with_names(&nodes);
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&format!("proxies:\n{yaml}")).expect("generated YAML must parse");
        assert_eq!(doc["proxies"][0]["sni"].as_str().unwrap(), "a\"b\\c");
        assert_eq!(doc["proxies"][1]["server"].as_str().unwrap(), "[::1]");
        // Group-list names must parse back to the same scalars as proxy names.
        let group_name: serde_yaml::Value = serde_yaml::from_str(&names[0]).unwrap();
        assert_eq!(group_name.as_str().unwrap(), "node1");
    }

    #[test]
    fn load_nodes_demo_mode_and_normal_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nodes.txt");
        let path = path.to_str().unwrap();
        assert!(load_nodes_with_demo(path, false).is_empty());
        assert!(!load_nodes_with_demo(path, true).is_empty());

        std::fs::write(path, "# no nodes yet\n\n").unwrap();
        assert!(load_nodes_with_demo(path, false).is_empty());
        assert!(!load_nodes_with_demo(path, true).is_empty());

        std::fs::write(path, "# configured node\n  example-node  \n").unwrap();
        for demo in [false, true] {
            assert_eq!(load_nodes_with_demo(path, demo), vec!["example-node"]);
        }
    }
}
