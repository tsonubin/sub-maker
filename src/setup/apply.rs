use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::Utc;
use rand::RngCore;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};
use uuid::Uuid;

use crate::assets;
use crate::config::{self, CertStrategy, SetupConfig};
use crate::diagnostics;
use crate::generator;
use crate::setup::{
    copy_certbot_certs_to_etc, copy_certs_to_etc, download_file, download_sing_box,
    download_subconverter, ensure_acme, ensure_certbot, generate_self_signed_cert, home_dir,
    subconverter_dir, subconverter_installed,
};

/// Apply performs the full "setup on current server".
pub fn apply(cfg: &mut SetupConfig) -> anyhow::Result<()> {
    let base = config::etc_base();
    let etc_sub = PathBuf::from(&base).join("sub-maker");
    let etc_sb = PathBuf::from(&base).join("sing-box");
    let opt_sc = subconverter_dir();

    for d in [&etc_sub, &etc_sb, &opt_sc] {
        fs::create_dir_all(d)?;
    }

    info!("downloading sing-box...");
    if let Err(err) = download_sing_box("") {
        warn!(error = %err, "sing-box download failed (may already exist or network)");
    }
    info!("downloading subconverter...");
    if let Err(err) = download_subconverter("") {
        warn!(error = %err, "subconverter download failed");
    }

    ensure_certificates(cfg)?;
    ensure_runtime_credentials(cfg)?;
    render_sing_box_config(cfg, &etc_sb)
        .map_err(|e| anyhow::anyhow!("render sing-box config: {e}"))?;

    let nodes = generator::generate_selected(
        &cfg.server_addr,
        &cfg.domain,
        &cfg.enabled_protocols,
        &cfg.ports,
        &cfg.creds,
    );
    let nodes_path = etc_sub.join("nodes.txt");
    generator::write_nodes_file(&nodes_path, &nodes)
        .map_err(|e| anyhow::anyhow!("write nodes: {e}"))?;
    info!(count = nodes.len(), "wrote nodes.txt");

    if let Err(err) = setup_subconverter_files(&etc_sub, &opt_sc) {
        warn!(error = %err, "subconverter files");
    }

    write_systemd_units(cfg, &nodes_path)
        .map_err(|e| anyhow::anyhow!("write systemd units: {e}"))?;

    if cfg.link_passcode_hash.is_empty() || cfg.link_passcode_salt.is_empty() {
        cfg.set_link_passcode(&config::generate_link_passcode())
            .map_err(|e| anyhow::anyhow!("set SSH link passcode: {e}"))?;
    }
    cfg.updated_at = Some(Utc::now());
    if cfg.created_at.is_none() {
        cfg.created_at = cfg.updated_at;
    }
    let cfg_bytes = serde_yaml::to_string(cfg)?;
    let cfg_path = etc_sub.join("config.yaml");
    fs::write(&cfg_path, cfg_bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&cfg_path, fs::Permissions::from_mode(0o640));
    }

    if !config::demo_mode() {
        start_and_verify_services(cfg)?;
    }

    print_apply_summary(cfg, &nodes_path, &etc_sb, &opt_sc, &cfg_path);
    Ok(())
}

fn print_apply_summary(
    cfg: &SetupConfig,
    nodes_path: &Path,
    etc_sb: &Path,
    opt_sc: &Path,
    cfg_path: &Path,
) {
    info!(sub = cfg.sub_port, "firewall: ensure ports open");
    let links = diagnostics::build_links(cfg);
    println!("\n=== APPLY COMPLETE ===");
    println!("Nodes: {}", nodes_path.display());
    println!("sing-box config: {}/config.json", etc_sb.display());
    println!("subconverter: {}", opt_sc.display());
    println!("sub-maker config: {}", cfg_path.display());
    println!("\nSubscription:\n  {}", links.subscription);
    println!("Raw nodes:\n  {}", links.raw);
    if !cfg.domain.is_empty() && !cfg.server_addr.is_empty() {
        println!("IP fallback:\n  {}", links.ip_fallback);
    }
    if !cfg.link_passcode_plain.is_empty() {
        println!("SSH link passcode:\n  {}", cfg.link_passcode_plain);
    }
    println!("\nCommands:");
    println!("  sudo sub-maker status");
    println!("  sudo sub-maker link");
    println!("  sudo sub-maker links");
    println!("  sudo sub-maker doctor");
    println!("  sudo sub-maker restart");
    println!("Open firewall ports:");
    for rule in firewall_rules(cfg) {
        println!(
            "  ufw allow {}/{}  # {}",
            rule.port, rule.transport, rule.label
        );
    }
}

struct FirewallRule {
    port: u16,
    transport: &'static str,
    label: String,
}

fn firewall_rules(cfg: &SetupConfig) -> Vec<FirewallRule> {
    let mut rules = vec![FirewallRule {
        port: cfg.sub_port,
        transport: "tcp",
        label: "subscription".into(),
    }];
    for protocol in &cfg.enabled_protocols {
        let port = match cfg.ports.get(protocol) {
            Some(&p) if p > 0 => p,
            _ => continue,
        };
        match protocol.as_str() {
            "hysteria2" | "tuic" => rules.push(FirewallRule {
                port,
                transport: "udp",
                label: protocol.clone(),
            }),
            "ss2022" => {
                rules.push(FirewallRule {
                    port,
                    transport: "tcp",
                    label: protocol.clone(),
                });
                rules.push(FirewallRule {
                    port,
                    transport: "udp",
                    label: protocol.clone(),
                });
            }
            _ => rules.push(FirewallRule {
                port,
                transport: "tcp",
                label: protocol.clone(),
            }),
        }
    }
    rules
}

fn ensure_certificates(cfg: &mut SetupConfig) -> anyhow::Result<()> {
    if !diagnostics::protocols_require_local_cert(&cfg.enabled_protocols) {
        return Ok(());
    }
    if !cfg.cert_path.is_empty()
        && !cfg.key_path.is_empty()
        && cfg.cert_mode == CertStrategy::Existing
    {
        return diagnostics::validate_certificate_pair(
            &cfg.cert_path,
            &cfg.key_path,
            &cfg.domain,
        );
    }

    let certs_dir = PathBuf::from(config::etc_base())
        .join("sub-maker")
        .join("certs");
    let cert_path = certs_dir.join("fullchain.pem");
    let key_path = certs_dir.join("privkey.pem");
    if cfg.cert_path.is_empty() {
        cfg.cert_path = cert_path.to_string_lossy().into();
    }
    if cfg.key_path.is_empty() {
        cfg.key_path = key_path.to_string_lossy().into();
    }
    if diagnostics::validate_certificate_pair(&cfg.cert_path, &cfg.key_path, &cfg.domain).is_ok() {
        return Ok(());
    }

    match cfg.cert_mode {
        CertStrategy::CertbotHttp => {
            if !config::demo_mode() {
                ensure_certbot(&cfg.domain, &cfg.email)?;
                copy_certbot_certs_to_etc(&cfg.domain)?;
            } else {
                generate_self_signed_cert(&cfg.domain, &cfg.server_addr)?;
            }
        }
        CertStrategy::AcmeHttp | CertStrategy::AcmeDnsCf => {
            if !config::demo_mode() {
                ensure_acme(
                    &cfg.domain,
                    &cfg.email,
                    cfg.cert_mode,
                    &cfg.acme_cf_token,
                )?;
                copy_certs_to_etc(&cfg.domain)?;
            } else {
                generate_self_signed_cert(&cfg.domain, &cfg.server_addr)?;
            }
        }
        CertStrategy::SelfSigned => {
            generate_self_signed_cert(&cfg.domain, &cfg.server_addr)?;
        }
        CertStrategy::Existing => {
            return diagnostics::validate_certificate_pair(
                &cfg.cert_path,
                &cfg.key_path,
                &cfg.domain,
            );
        }
    }

    cfg.cert_path = cert_path.to_string_lossy().into();
    cfg.key_path = key_path.to_string_lossy().into();
    diagnostics::validate_certificate_pair(&cfg.cert_path, &cfg.key_path, &cfg.domain)
}

pub fn ensure_runtime_credentials(cfg: &mut SetupConfig) -> anyhow::Result<()> {
    for proto in cfg.enabled_protocols.clone() {
        let creds = cfg.creds.entry(proto.clone()).or_default();
        match proto.as_str() {
            "reality" => {
                if creds.get("uuid").map(|s| s.is_empty()).unwrap_or(true) {
                    creds.insert("uuid".into(), Uuid::new_v4().to_string());
                }
                if creds.get("short_id").map(|s| s.is_empty()).unwrap_or(true) {
                    creds.insert("short_id".into(), random_hex(4));
                }
                let need_keys = creds.get("private_key").map(|s| s.is_empty()).unwrap_or(true)
                    || creds.get("pbk").map(|s| s.is_empty()).unwrap_or(true);
                if need_keys {
                    match generate_reality_keypair() {
                        Ok((private_key, public_key)) => {
                            creds.insert("private_key".into(), private_key);
                            creds.insert("pbk".into(), public_key);
                        }
                        Err(err) if config::demo_mode() => {
                            warn!(error = %err, "demo mode: using stub Reality keys (sing-box binary may be for another OS)");
                            creds.insert(
                                "private_key".into(),
                                "demo-private-key-replace-on-linux".into(),
                            );
                            creds.insert(
                                "pbk".into(),
                                "demo-public-key-replace-on-linux".into(),
                            );
                        }
                        Err(err) => return Err(err),
                    }
                }
            }
            "hysteria2" | "tuic" | "anytls" => {
                if creds.get("pass").map(|s| s.is_empty()).unwrap_or(true) {
                    creds.insert("pass".into(), random_secret());
                }
                if proto == "tuic" && creds.get("uuid").map(|s| s.is_empty()).unwrap_or(true) {
                    creds.insert("uuid".into(), Uuid::new_v4().to_string());
                }
            }
            "ss2022" => {
                if creds.get("pass").map(|s| s.is_empty()).unwrap_or(true) {
                    creds.insert("pass".into(), random_ss2022_secret());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn random_secret() -> String {
    let mut b = [0u8; 18];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return Uuid::new_v4().to_string();
    }
    URL_SAFE_NO_PAD.encode(b)
}

fn random_ss2022_secret() -> String {
    let mut b = [0u8; 16];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return random_secret();
    }
    STANDARD.encode(b)
}

fn random_hex(size: usize) -> String {
    let mut b = vec![0u8; size];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return Uuid::new_v4().simple().to_string()[..size * 2].to_string();
    }
    hex::encode(b)
}

fn generate_reality_keypair() -> anyhow::Result<(String, String)> {
    let mut binary = PathBuf::from("/usr/local/bin/sing-box");
    if config::demo_mode() {
        binary = PathBuf::from(home_dir()).join(".local/bin/sing-box");
    }
    if !binary.exists() {
        if let Ok(found) = which::which("sing-box") {
            binary = found;
        }
    }
    let output = Command::new(&binary)
        .args(["generate", "reality-keypair"])
        .output()
        .map_err(|e| anyhow::anyhow!("generate reality keypair: {e}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "generate reality keypair: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    parse_reality_keypair_output(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_reality_keypair_output(output: &str) -> anyhow::Result<(String, String)> {
    let private_key = extract_key_line(
        output,
        r"(?i)private[\s_-]*key\s*:\s*([A-Za-z0-9_-]+)",
    );
    let public_key = extract_key_line(
        output,
        r"(?i)public[\s_-]*key\s*:\s*([A-Za-z0-9_-]+)",
    );
    if private_key.is_empty() || public_key.is_empty() {
        anyhow::bail!("could not parse sing-box reality keypair output: {output}");
    }
    Ok((private_key, public_key))
}

fn extract_key_line(output: &str, pattern: &str) -> String {
    let re = Regex::new(pattern).unwrap();
    re.captures(output)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

fn start_and_verify_services(cfg: &SetupConfig) -> anyhow::Result<()> {
    diagnostics::run_systemctl(&["daemon-reload"])?;
    let mut services = vec!["sing-box", "sub-maker-sub"];
    if subconverter_installed() {
        services.push("subconverter");
    }
    let mut args = vec!["enable", "--now"];
    args.extend(services);
    diagnostics::run_systemctl(&args)?;
    let local_url = format!(
        "http://127.0.0.1:{}/sub?token={}",
        cfg.sub_port, cfg.sub_token
    );
    diagnostics::verify_subscription_endpoint(&local_url)
}

pub fn protocol_server_name(
    proto: &str,
    cfg: &SetupConfig,
    creds: &HashMap<String, String>,
) -> String {
    if let Some(sni) = creds.get("sni").filter(|s| !s.is_empty()) {
        return sni.clone();
    }
    if proto == "reality" {
        return "www.apple.com".into();
    }
    cfg.domain.clone()
}

fn read_template(name: &str) -> anyhow::Result<String> {
    if let Ok(data) = fs::read_to_string(name) {
        return Ok(data);
    }
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    if let Ok(data) = fs::read_to_string(format!("templates/{base}")) {
        return Ok(data);
    }
    if let Some(data) = assets::template(base) {
        return Ok(data.to_string());
    }
    if name.contains("singbox_base") {
        return Ok(
            r#"{"log":{"level":"info"},"dns":{"servers":[{"address":"https://1.1.1.1/dns-query"}]},"inbounds":[{{.Inbounds}}],"outbounds":[{"type":"direct"}]}"#
                .into(),
        );
    }
    anyhow::bail!("template {name} not found")
}

fn inbound_template_data(
    proto: &str,
    cfg: &SetupConfig,
) -> (String, Vec<(&'static str, String)>) {
    let empty = HashMap::new();
    let creds = cfg.creds.get(proto).unwrap_or(&empty);
    let mut data: Vec<(&str, String)> = vec![
        (
            "Port",
            cfg.ports.get(proto).copied().unwrap_or(0).to_string(),
        ),
        ("ServerName", protocol_server_name(proto, cfg, creds)),
    ];
    if !cfg.cert_path.is_empty() {
        data.push(("CertPath", cfg.cert_path.clone()));
        data.push(("KeyPath", cfg.key_path.clone()));
    } else {
        data.push(("CertPath", "/etc/sub-maker/certs/fullchain.pem".into()));
        data.push(("KeyPath", "/etc/sub-maker/certs/privkey.pem".into()));
    }

    let tpl_name = match proto {
        "reality" => {
            data.push((
                "UUID",
                creds.get("uuid").cloned().unwrap_or_default(),
            ));
            data.push((
                "PrivateKey",
                creds.get("private_key").cloned().unwrap_or_default(),
            ));
            data.push((
                "ShortID",
                creds.get("short_id").cloned().unwrap_or_default(),
            ));
            "inbound_reality.json.tpl"
        }
        "hysteria2" => {
            data.push((
                "Password",
                creds.get("pass").cloned().unwrap_or_default(),
            ));
            "inbound_hysteria2.json.tpl"
        }
        "tuic" => {
            data.push((
                "UUID",
                creds.get("uuid").cloned().unwrap_or_default(),
            ));
            data.push((
                "Password",
                creds.get("pass").cloned().unwrap_or_default(),
            ));
            "inbound_tuic.json.tpl"
        }
        "anytls" => {
            data.push((
                "Password",
                creds.get("pass").cloned().unwrap_or_default(),
            ));
            "inbound_anytls.json.tpl"
        }
        "ss2022" => {
            data.push((
                "Password",
                creds.get("pass").cloned().unwrap_or_default(),
            ));
            data.push(("Method", "2022-blake3-aes-128-gcm".into()));
            "inbound_ss2022.json.tpl"
        }
        _ => "",
    };
    (tpl_name.into(), data)
}

pub fn render_sing_box_config(cfg: &SetupConfig, etc_sb: &Path) -> anyhow::Result<()> {
    let mut inbound_strs = Vec::new();
    for proto in &cfg.enabled_protocols {
        let (tpl_name, data) = inbound_template_data(proto, cfg);
        let tpl_content = read_template(&format!("templates/{tpl_name}")).unwrap_or_else(|_| {
            format!(
                r#"{{"type":"{proto}","listen":"::","listen_port":{}}}"#,
                cfg.ports.get(proto).copied().unwrap_or(0)
            )
        });
        inbound_strs.push(assets::render_simple(&tpl_content, &data));
    }

    // Assemble base JSON without relying on Go's range template.
    let inbounds_joined = inbound_strs.join(",\n    ");
    let base = format!(
        r#"{{
  "log": {{
    "level": "info",
    "timestamp": true
  }},
  "dns": {{
    "strategy": "ipv4_only",
    "servers": [
      {{
        "type": "https",
        "server": "1.1.1.1"
      }}
    ]
  }},
  "inbounds": [
    {inbounds_joined}
  ],
  "outbounds": [
    {{
      "type": "direct",
      "tag": "direct"
    }}
  ]
}}
"#
    );

    if let Err(err) = serde_json::from_str::<serde_json::Value>(&base) {
        warn!(error = %err, "sing-box config may not be perfect json");
    }
    fs::write(etc_sb.join("config.json"), base)?;
    Ok(())
}

fn setup_subconverter_files(etc_sub: &Path, opt_sc: &Path) -> anyhow::Result<()> {
    let pref = read_template("templates/subconverter_pref.ini.tpl")
        .unwrap_or_else(|_| "api_access_token=submaker123\n".into());
    fs::write(opt_sc.join("pref.ini"), pref)?;

    let acl_url =
        "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online_Full.ini";
    let acl_path = etc_sub.join("acl4ssr.ini");
    if let Err(err) = download_file(acl_url, &acl_path) {
        warn!(error = %err, "could not download ACL rules, using minimal");
        fs::write(
            &acl_path,
            "[custom]\n; add your rules here or use remote\n",
        )?;
    }
    Ok(())
}

pub fn write_systemd_units(cfg: &SetupConfig, nodes_path: &Path) -> anyhow::Result<()> {
    let unit_names = [
        "sing-box.service",
        "subconverter.service",
        "sub-maker-sub.service",
    ];
    for name in unit_names {
        let content = match read_template(&format!("templates/{name}.tpl")) {
            Ok(c) => c,
            Err(err) => {
                if name == "sub-maker-sub.service" {
                    return Err(err);
                }
                format!("[Unit]\nDescription={name}\n[Service]\nExecStart=/bin/true\n")
            }
        };
        let rendered = assets::render_simple(
            &content,
            &[
                ("Token", cfg.sub_token.clone()),
                ("Port", cfg.sub_port.to_string()),
                ("NodesPath", nodes_path.to_string_lossy().into()),
            ],
        );
        let path = if config::demo_mode() {
            PathBuf::from("/tmp").join(name)
        } else {
            PathBuf::from("/etc/systemd/system").join(name)
        };
        match fs::write(&path, &rendered) {
            Ok(()) => info!(path = %path.display(), "wrote unit"),
            Err(err) => warn!(path = %path.display(), error = %err, "write unit"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SetupConfig;

    #[test]
    fn parse_reality_keypair_output_accepts_singbox_labels() {
        let output = "PrivateKey: kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU\nPublicKey: ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs\n";
        let (private_key, public_key) = parse_reality_keypair_output(output).unwrap();
        assert_eq!(
            private_key,
            "kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU"
        );
        assert_eq!(public_key, "ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs");
    }

    #[test]
    fn parse_reality_keypair_output_accepts_separated_labels() {
        let output = "Private key: private_value\nPublic key: public_value\n";
        let (private_key, public_key) = parse_reality_keypair_output(output).unwrap();
        assert_eq!(private_key, "private_value");
        assert_eq!(public_key, "public_value");
    }

    #[test]
    fn protocol_server_name_uses_reality_camouflage_sni() {
        let cfg = SetupConfig {
            domain: "proxy.example.com".into(),
            ..SetupConfig::default()
        };
        let creds: HashMap<String, String> =
            [("sni".into(), "www.apple.com".into())].into_iter().collect();
        assert_eq!(protocol_server_name("reality", &cfg, &creds), "www.apple.com");
    }

    #[test]
    fn protocol_server_name_defaults_reality_without_override() {
        let cfg = SetupConfig {
            domain: "proxy.example.com".into(),
            ..SetupConfig::default()
        };
        assert_eq!(
            protocol_server_name("reality", &cfg, &HashMap::new()),
            "www.apple.com"
        );
    }

    #[test]
    fn protocol_server_name_uses_domain_for_tls_protocols() {
        let cfg = SetupConfig {
            domain: "proxy.example.com".into(),
            ..SetupConfig::default()
        };
        assert_eq!(
            protocol_server_name("hysteria2", &cfg, &HashMap::new()),
            "proxy.example.com"
        );
    }

    #[test]
    fn write_systemd_units_renders_sub_maker_subscription_unit() {
        std::env::set_var("SUB_MAKER_DEMO", "1");
        let unit_path = PathBuf::from("/tmp/sub-maker-sub.service");
        let _ = fs::remove_file(&unit_path);
        let cfg = SetupConfig {
            sub_token: "test-token".into(),
            sub_port: 1984,
            ..SetupConfig::default()
        };
        write_systemd_units(&cfg, Path::new("/etc/sub-maker/nodes.txt")).unwrap();
        let unit = fs::read_to_string(&unit_path).unwrap();
        for want in [
            "Environment=SUB_MAKER_TOKEN=test-token",
            "Environment=SUB_MAKER_PORT=1984",
            "Environment=SUB_MAKER_NODES_FILE=/etc/sub-maker/nodes.txt",
            "ExecStart=/usr/local/bin/sub-maker --serve",
        ] {
            assert!(unit.contains(want), "missing {want:?} in:\n{unit}");
        }
        let _ = fs::remove_file(&unit_path);
        std::env::remove_var("SUB_MAKER_DEMO");
    }

    #[test]
    fn render_sing_box_config_uses_base_template() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = SetupConfig {
            enabled_protocols: vec!["ss2022".into()],
            ports: [("ss2022".into(), 8388)].into_iter().collect(),
            creds: [(
                "ss2022".into(),
                [("pass".into(), "test-password".into())]
                    .into_iter()
                    .collect(),
            )]
            .into_iter()
            .collect(),
            ..SetupConfig::default()
        };
        render_sing_box_config(&cfg, dir.path()).unwrap();
        let data = fs::read_to_string(dir.path().join("config.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
        let inbounds = parsed["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1);
        assert_eq!(inbounds[0]["type"], "shadowsocks");
    }

    #[test]
    fn ensure_runtime_credentials_fills_all_enabled_protocols() {
        let mut cfg = SetupConfig::default_config();
        cfg.creds.insert(
            "reality".into(),
            [
                ("private_key".into(), "private".into()),
                ("pbk".into(), "public".into()),
            ]
            .into_iter()
            .collect(),
        );
        ensure_runtime_credentials(&mut cfg).unwrap();
        let required = [
            ("reality", &["uuid", "short_id", "private_key", "pbk"][..]),
            ("hysteria2", &["pass"]),
            ("tuic", &["uuid", "pass"]),
            ("anytls", &["pass"]),
            ("ss2022", &["pass"]),
        ];
        for (proto, keys) in required {
            for key in keys {
                assert!(
                    !cfg.creds[proto][*key].is_empty(),
                    "missing generated credential {proto}.{key}"
                );
            }
        }
    }
}
