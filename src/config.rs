use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const ALL_PROTOCOLS: &[&str] = &["reality", "hysteria2", "tuic", "anytls", "ss2022"];

pub fn default_ports() -> std::collections::HashMap<String, u16> {
    [
        ("reality", 443),
        ("hysteria2", 8443),
        ("tuic", 9443),
        ("anytls", 7443),
        ("ss2022", 8388),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub fn demo_mode() -> bool {
    std::env::var("SUB_MAKER_DEMO").map(|v| !v.is_empty()).unwrap_or(false)
}

pub fn demo_base() -> &'static str {
    "/tmp/sub-maker-demo-etc"
}

pub fn etc_base() -> String {
    if demo_mode() {
        demo_base().to_string()
    } else {
        "/etc".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SetupMode {
    #[default]
    Production,
    #[serde(rename = "ip-only")]
    IpOnly,
}

impl SetupMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::IpOnly => "ip-only",
        }
    }
}

impl std::fmt::Display for SetupMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SetupMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "production" => Ok(Self::Production),
            "ip-only" => Ok(Self::IpOnly),
            other => anyhow::bail!("unknown setup mode: {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CertStrategy {
    #[default]
    #[serde(rename = "certbot-http")]
    CertbotHttp,
    #[serde(rename = "acme-http")]
    AcmeHttp,
    #[serde(rename = "acme-dns-cf")]
    AcmeDnsCf,
    Existing,
    #[serde(rename = "self-signed")]
    SelfSigned,
}

impl CertStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertbotHttp => "certbot-http",
            Self::AcmeHttp => "acme-http",
            Self::AcmeDnsCf => "acme-dns-cf",
            Self::Existing => "existing",
            Self::SelfSigned => "self-signed",
        }
    }
}

impl std::fmt::Display for CertStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CertStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "certbot-http" => Ok(Self::CertbotHttp),
            "acme-http" => Ok(Self::AcmeHttp),
            "acme-dns-cf" => Ok(Self::AcmeDnsCf),
            "existing" => Ok(Self::Existing),
            "self-signed" => Ok(Self::SelfSigned),
            other => anyhow::bail!("unknown cert strategy: {other}"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsCheckResult {
    pub domain: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expected_ipv4: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expected_ipv6: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_a: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_aaaa: Vec<String>,
    pub ipv4_matches: bool,
    pub ipv6_matches: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_checked_at: String,
}

/// SetupConfig holds all user choices from the TUI wizard. Persisted to /etc/sub-maker/config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupConfig {
    #[serde(default)]
    pub server_addr: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub sub_token: String,
    #[serde(default = "default_sub_port")]
    pub sub_port: u16,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub setup_mode: SetupMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_check: Option<DnsCheckResult>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub link_passcode_salt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub link_passcode_hash: String,
    #[serde(skip)]
    pub link_passcode_plain: String,

    #[serde(default)]
    pub enabled_protocols: Vec<String>,
    #[serde(default)]
    pub ports: std::collections::HashMap<String, u16>,
    #[serde(default)]
    pub creds: std::collections::HashMap<String, std::collections::HashMap<String, String>>,

    #[serde(default)]
    pub cert_mode: CertStrategy,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub acme_cf_token: String,
    #[serde(default)]
    pub cert_path: String,
    #[serde(default)]
    pub key_path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

fn default_sub_port() -> u16 {
    8964
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            server_addr: String::new(),
            domain: String::new(),
            sub_token: String::new(),
            sub_port: 8964,
            email: String::new(),
            setup_mode: SetupMode::Production,
            dns_check: None,
            link_passcode_salt: String::new(),
            link_passcode_hash: String::new(),
            link_passcode_plain: String::new(),
            enabled_protocols: ALL_PROTOCOLS.iter().map(|s| s.to_string()).collect(),
            ports: default_ports(),
            creds: std::collections::HashMap::new(),
            cert_mode: CertStrategy::CertbotHttp,
            acme_cf_token: String::new(),
            cert_path: String::new(),
            key_path: String::new(),
            created_at: None,
            updated_at: None,
        }
    }
}

impl SetupConfig {
    pub fn default_config() -> Self {
        Self::default()
    }

    pub fn set_link_passcode(&mut self, passcode: &str) -> anyhow::Result<()> {
        if passcode.is_empty() {
            anyhow::bail!("passcode is required");
        }
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let salt_text = URL_SAFE_NO_PAD.encode(salt);
        self.link_passcode_salt = salt_text.clone();
        self.link_passcode_hash = hash_link_passcode(&salt_text, passcode);
        self.link_passcode_plain = passcode.to_string();
        Ok(())
    }

    pub fn verify_link_passcode(&self, passcode: &str) -> bool {
        if self.link_passcode_salt.is_empty()
            || self.link_passcode_hash.is_empty()
            || passcode.is_empty()
        {
            return false;
        }
        let expected = hash_link_passcode(&self.link_passcode_salt, passcode);
        expected
            .as_bytes()
            .ct_eq(self.link_passcode_hash.as_bytes())
            .into()
    }
}

/// Subscription tokens end up in URLs and in a systemd unit file; restrict
/// them to a URL-safe charset that is also inert in unit files (no
/// whitespace, quotes, or systemd `%` specifiers).
pub fn validate_sub_token(token: &str) -> anyhow::Result<()> {
    if token.is_empty() {
        anyhow::bail!("subscription token must not be empty");
    }
    if token.len() > 128 {
        anyhow::bail!("subscription token too long (max 128 characters)");
    }
    if !token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
    {
        anyhow::bail!(
            "subscription token may only contain letters, digits, '.', '_', '~', '-'"
        );
    }
    Ok(())
}

pub fn generate_link_passcode() -> String {
    const ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
    let mut b = [0u8; 10];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return "change-me".to_string();
    }
    b.iter()
        .map(|v| ALPHABET[(*v as usize) % ALPHABET.len()] as char)
        .collect()
}

fn hash_link_passcode(salt: &str, passcode: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0u8]);
    hasher.update(passcode.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_passcode_hash_verifies_only_correct_passcode() {
        let mut cfg = SetupConfig::default_config();
        cfg.set_link_passcode("123456").unwrap();
        assert!(!cfg.link_passcode_hash.is_empty());
        assert!(!cfg.link_passcode_salt.is_empty());
        assert_ne!(cfg.link_passcode_hash, "123456");
        assert!(cfg.verify_link_passcode("123456"));
        assert!(!cfg.verify_link_passcode("654321"));
    }

    #[test]
    fn generate_link_passcode_returns_human_sized_code() {
        let passcode = generate_link_passcode();
        assert_eq!(passcode.len(), 10);
    }

    #[test]
    fn link_passcode_plain_is_not_serialized() {
        let mut cfg = SetupConfig::default_config();
        cfg.set_link_passcode("do-not-store").unwrap();
        let data = serde_yaml::to_string(&cfg).unwrap();
        assert!(!data.contains("do-not-store"));
    }
}
