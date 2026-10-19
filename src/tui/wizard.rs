//! Multi-step setup wizard — single ratatui app with sidebar navigation.

use std::collections::HashMap;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use tracing::info;
use uuid::Uuid;

use crate::config::{
    self, CertStrategy, DnsCheckResult, SetupConfig, SetupMode,
};
use crate::diagnostics;
use crate::tui::terminal::{cancel_err, next_input, Input, Tui};
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    draw_fields, draw_footer, draw_header, draw_option_list, draw_review, draw_sidebar, kv_line,
    root_chunks, StepMeta, StepStatus,
};
use crate::tui::{
    capitalize_first, edit_text, insert_text, random_hex, random_secret, random_ss2022_secret,
    random_token, Edit,
};

const POLL: Duration = Duration::from_millis(80);

const PROTO_OPTIONS: &[(&str, &str)] = &[
    ("reality", "VLESS + Reality-Vision — stealth"),
    ("hysteria2", "Hysteria2 — lossy / high speed"),
    ("tuic", "TUIC v5 — QUIC alternative"),
    ("anytls", "AnyTLS — padded TLS camo"),
    ("ss2022", "Shadowsocks 2022 — simple & fast"),
];

const CERT_OPTIONS: &[&str] = &[
    "Certbot HTTP-01 (recommended · domain DNS + port 80)",
    "acme.sh HTTP-01 (domain DNS + port 80)",
    "ACME DNS-01 Cloudflare (API token)",
    "Existing certificate files",
    "Self-signed (limited client compatibility)",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Mode = 0,
    Identity = 1,
    Protocols = 2,
    Certs = 3,
    Details = 4,
    Review = 5,
}

impl Step {
    fn all() -> &'static [Step] {
        &[
            Step::Mode,
            Step::Identity,
            Step::Protocols,
            Step::Certs,
            Step::Details,
            Step::Review,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Step::Mode => "Setup mode",
            Step::Identity => "Server & domain",
            Step::Protocols => "Protocols",
            Step::Certs => "Certificates",
            Step::Details => "Node details",
            Step::Review => "Review",
        }
    }

}

struct Field {
    label: &'static str,
    help: &'static str,
    value: String,
    password: bool,
    pos: usize,
}

struct Wizard {
    step: Step,
    /// Highest step index the user has reached (for sidebar "done" markers).
    max_reached: usize,

    // Mode
    mode_cursor: usize, // 0 production, 1 ip-only
    detected_ip: String,

    // Identity fields
    identity: Vec<Field>,
    identity_focus: usize,

    // Protocols
    proto_selected: [bool; 5],
    proto_cursor: usize,

    // Certs
    cert_cursor: usize,
    cert_extra: Vec<Field>, // CF token or cert/key paths
    cert_extra_focus: usize,
    show_cert_extra: bool,

    // Per-protocol details
    detail_protos: Vec<String>,
    detail_idx: usize,
    detail_fields: Vec<Field>,
    detail_focus: usize,
    /// Accumulated creds keyed by protocol.
    creds: HashMap<String, HashMap<String, String>>,
    ports: HashMap<String, u16>,

    // DNS warning gate
    dns_warning: Option<String>,
    dns_cursor: usize, // 0 abort, 1 continue

    // Review
    error: Option<String>,
    finished: bool,
}

impl Wizard {
    fn new() -> Self {
        let mut detected_ip = String::new();
        let mut server_default = String::new();
        match diagnostics::detect_public_ip() {
            Ok(ip) => {
                info!(ip = %ip, "detected server public IP");
                detected_ip = ip.clone();
                server_default = ip;
            }
            Err(err) => {
                info!(error = %err, "could not auto-detect server public IP");
            }
        }

        let mut identity = vec![
            Field {
                label: "Server address",
                help: "Public IP or domain used in client share links",
                value: server_default,
                password: false,
                pos: 0,
            },
            Field {
                label: "Domain",
                help: "DNS name for certs & subscription (required in production)",
                value: String::new(),
                password: false,
                pos: 0,
            },
            Field {
                label: "ACME email",
                help: "Contact email for Let's Encrypt / acme.sh",
                value: String::new(),
                password: false,
                pos: 0,
            },
            Field {
                label: "Subscription token",
                help: "Leave blank to auto-generate",
                value: String::new(),
                password: false,
                pos: 0,
            },
            Field {
                label: "SSH link passcode",
                help: "Protects `sub-maker link` · blank = auto-generate",
                value: String::new(),
                password: true,
                pos: 0,
            },
            Field {
                label: "Sub port",
                help: "Subscription HTTP listen port",
                value: "8964".into(),
                password: false,
                pos: 0,
            },
        ];
        for f in &mut identity {
            f.pos = f.value.len();
        }

        Self {
            step: Step::Mode,
            max_reached: 0,
            mode_cursor: 0,
            detected_ip,
            identity,
            identity_focus: 0,
            proto_selected: [true; 5],
            proto_cursor: 0,
            cert_cursor: 0,
            cert_extra: Vec::new(),
            cert_extra_focus: 0,
            show_cert_extra: false,
            detail_protos: Vec::new(),
            detail_idx: 0,
            detail_fields: Vec::new(),
            detail_focus: 0,
            creds: HashMap::new(),
            ports: config::default_ports(),
            dns_warning: None,
            dns_cursor: 0,
            error: None,
            finished: false,
        }
    }

    fn setup_mode(&self) -> SetupMode {
        if self.mode_cursor == 0 {
            SetupMode::Production
        } else {
            SetupMode::IpOnly
        }
    }

    fn enabled_protocols(&self) -> Vec<String> {
        PROTO_OPTIONS
            .iter()
            .enumerate()
            .filter(|(i, _)| self.proto_selected[*i])
            .map(|(_, (id, _))| (*id).to_string())
            .collect()
    }

    fn needs_certs(&self) -> bool {
        diagnostics::protocols_require_local_cert(&self.enabled_protocols())
    }

    fn cert_strategy(&self) -> CertStrategy {
        match self.cert_cursor {
            0 => CertStrategy::CertbotHttp,
            1 => CertStrategy::AcmeHttp,
            2 => CertStrategy::AcmeDnsCf,
            3 => CertStrategy::Existing,
            _ => CertStrategy::SelfSigned,
        }
    }

    fn step_metas(&self) -> Vec<StepMeta> {
        let cur = self.step as usize;
        Step::all()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let label = if *s == Step::Certs && !self.needs_certs() && cur > Step::Certs as usize
                {
                    "Certificates (n/a)"
                } else {
                    s.label()
                };
                let status = if i < cur {
                    StepStatus::Done
                } else if i == cur {
                    StepStatus::Current
                } else {
                    StepStatus::Pending
                };
                StepMeta { label, status }
            })
            .collect()
    }

    fn go_next(&mut self) -> anyhow::Result<()> {
        self.error = None;
        match self.step {
            Step::Mode => {
                self.step = Step::Identity;
            }
            Step::Identity => {
                self.validate_identity()?;
                // DNS check for production
                if self.setup_mode() == SetupMode::Production {
                    let domain = self.identity[1].value.trim().to_string();
                    if !domain.is_empty() {
                        if let Some(msg) = self.check_dns(&domain) {
                            self.dns_warning = Some(msg);
                            self.dns_cursor = 0;
                            return Ok(());
                        }
                    }
                }
                self.step = Step::Protocols;
            }
            Step::Protocols => {
                if self.enabled_protocols().is_empty() {
                    anyhow::bail!("select at least one protocol (Space toggles, a = all, n = none)");
                }
                if self.needs_certs() {
                    if self.setup_mode() == SetupMode::IpOnly {
                        self.cert_cursor = 4; // self-signed default
                    }
                    self.step = Step::Certs;
                    self.prepare_cert_extra();
                } else {
                    self.enter_details()?;
                }
            }
            Step::Certs => {
                if self.show_cert_extra {
                    // validate extra then proceed
                    self.apply_cert_extra()?;
                } else {
                    let strategy = self.cert_strategy();
                    if matches!(
                        strategy,
                        CertStrategy::AcmeDnsCf | CertStrategy::Existing
                    ) {
                        self.prepare_cert_extra();
                        self.show_cert_extra = true;
                        self.cert_extra_focus = 0;
                        return Ok(());
                    }
                }
                self.enter_details()?;
            }
            Step::Details => {
                self.save_current_detail()?;
                if self.detail_idx + 1 < self.detail_protos.len() {
                    self.detail_idx += 1;
                    self.load_detail_fields();
                } else {
                    self.step = Step::Review;
                }
            }
            Step::Review => {
                self.finished = true;
            }
        }
        self.max_reached = self.max_reached.max(self.step as usize);
        Ok(())
    }

    fn go_back(&mut self) {
        self.error = None;
        self.dns_warning = None;
        match self.step {
            Step::Mode => {}
            Step::Identity => self.step = Step::Mode,
            Step::Protocols => self.step = Step::Identity,
            Step::Certs => {
                if self.show_cert_extra {
                    self.show_cert_extra = false;
                } else {
                    self.step = Step::Protocols;
                }
            }
            Step::Details => {
                if self.detail_idx > 0 {
                    if let Err(e) = self.save_current_detail() {
                        self.error = Some(e.to_string());
                        return;
                    }
                    self.detail_idx -= 1;
                    self.load_detail_fields();
                } else if self.needs_certs() {
                    self.step = Step::Certs;
                    self.show_cert_extra = false;
                } else {
                    self.step = Step::Protocols;
                }
            }
            Step::Review => {
                self.step = Step::Details;
                if !self.detail_protos.is_empty() {
                    self.detail_idx = self.detail_protos.len() - 1;
                    self.load_detail_fields();
                }
            }
        }
    }

    fn validate_identity(&self) -> anyhow::Result<()> {
        if self.identity[0].value.trim().is_empty() {
            anyhow::bail!("server address is required (public IP or domain used in the share links)");
        }
        if self.setup_mode() == SetupMode::Production && self.identity[1].value.trim().is_empty() {
            anyhow::bail!("domain is required for production setup");
        }
        let sub_port = self.identity[5].value.trim();
        if !sub_port.is_empty() {
            let port: u16 = sub_port
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid subscription port: {sub_port:?}"))?;
            if port == 0 {
                anyhow::bail!("subscription port must be between 1 and 65535");
            }
        }
        Ok(())
    }

    fn check_dns(&self, domain: &str) -> Option<String> {
        let mut expected = self.identity[0].value.trim().to_string();
        let (v4, v6) = diagnostics::split_expected_ips(&expected);
        if v4.is_empty() && v6.is_empty() && !self.detected_ip.is_empty() {
            expected = self.detected_ip.clone();
        }
        let (expected_ipv4, expected_ipv6) = diagnostics::split_expected_ips(&expected);
        let (result, err) =
            diagnostics::check_domain_dns(domain, &expected_ipv4, &expected_ipv6);
        if err.is_none() && diagnostics::dns_matches(&result) {
            return None;
        }
        Some(format_dns_mismatch(
            domain,
            &expected_ipv4,
            &expected_ipv6,
            &result,
            err.as_ref(),
        ))
    }

    fn prepare_cert_extra(&mut self) {
        let previous = std::mem::take(&mut self.cert_extra);
        let mut fields = match self.cert_strategy() {
            CertStrategy::AcmeDnsCf => vec![Field {
                label: "Cloudflare API token",
                help: "DNS edit permission for the zone",
                value: String::new(),
                password: true,
                pos: 0,
            }],
            CertStrategy::Existing => vec![
                Field {
                    label: "Certificate path",
                    help: "fullchain.pem",
                    value: "/etc/sub-maker/certs/fullchain.pem".into(),
                    password: false,
                    pos: 0,
                },
                Field {
                    label: "Private key path",
                    help: "privkey.pem",
                    value: "/etc/sub-maker/certs/privkey.pem".into(),
                    password: false,
                    pos: 0,
                },
            ],
            _ => vec![],
        };
        for f in fields.iter_mut() {
            f.pos = f.value.len();
        }
        // Keep what the user already typed when re-entering with the same
        // field set (e.g. after back-navigation); only reset on change.
        if previous.len() == fields.len()
            && previous
                .iter()
                .zip(fields.iter())
                .all(|(old, new)| old.label == new.label)
        {
            for (old, new) in previous.into_iter().zip(fields.iter_mut()) {
                new.value = old.value;
                new.pos = old.pos.min(new.value.len());
            }
        }
        self.cert_extra = fields;
    }

    fn apply_cert_extra(&mut self) -> anyhow::Result<()> {
        if self.cert_strategy() == CertStrategy::Existing && self.cert_extra.len() >= 2 {
            let cert = self.cert_extra[0].value.trim();
            let key = self.cert_extra[1].value.trim();
            let domain = self.identity[1].value.trim();
            diagnostics::validate_certificate_pair(cert, key, domain)?;
        }
        self.show_cert_extra = false;
        Ok(())
    }

    fn enter_details(&mut self) -> anyhow::Result<()> {
        self.detail_protos = self.enabled_protocols();
        self.detail_idx = 0;
        self.load_detail_fields();
        self.step = Step::Details;
        Ok(())
    }

    fn load_detail_fields(&mut self) {
        let p = &self.detail_protos[self.detail_idx];
        let remark = capitalize_first(p);
        let port = self.ports.get(p).copied().unwrap_or(0);
        let existing = self.creds.get(p).cloned().unwrap_or_default();
        let default_sni = if p == "reality" {
            existing
                .get("sni")
                .cloned()
                .unwrap_or_else(|| "www.apple.com".into())
        } else {
            existing
                .get("sni")
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| self.identity[1].value.trim().to_string())
        };

        let mut fields = vec![
            Field {
                label: "Listen port",
                help: "Inbound port on this server",
                value: port.to_string(),
                password: false,
                pos: 0,
            },
            Field {
                label: "Remark / tag",
                help: "Name shown in clients",
                value: existing.get("remark").cloned().unwrap_or(remark),
                password: false,
                pos: 0,
            },
            Field {
                label: "SNI / target",
                help: "TLS SNI or Reality handshake target",
                value: default_sni,
                password: false,
                pos: 0,
            },
        ];
        if matches!(p.as_str(), "hysteria2" | "tuic" | "anytls" | "ss2022") {
            fields.push(Field {
                label: "Password",
                help: "Blank = auto-generate",
                value: existing.get("pass").cloned().unwrap_or_default(),
                password: true,
                pos: 0,
            });
        }
        if matches!(p.as_str(), "reality" | "tuic") {
            fields.push(Field {
                label: "UUID",
                help: "Blank = auto-generate",
                value: existing.get("uuid").cloned().unwrap_or_default(),
                password: false,
                pos: 0,
            });
        }
        if p == "reality" {
            fields.push(Field {
                label: "Reality public key",
                help: "Blank = generate during apply",
                value: existing.get("pbk").cloned().unwrap_or_default(),
                password: false,
                pos: 0,
            });
            fields.push(Field {
                label: "Reality shortId",
                help: "8 hex chars · blank = auto",
                value: existing.get("short_id").cloned().unwrap_or_default(),
                password: false,
                pos: 0,
            });
        }
        for f in fields.iter_mut() {
            f.pos = f.value.len();
        }
        self.detail_fields = fields;
        self.detail_focus = 0;
    }

    fn save_current_detail(&mut self) -> anyhow::Result<()> {
        if self.detail_protos.is_empty() {
            return Ok(());
        }
        let p = self.detail_protos[self.detail_idx].clone();
        let mut extra = HashMap::new();
        let mut fi = 0;
        let port_str = self.detail_fields[fi].value.trim();
        let port: u16 = port_str
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid listen port for {p}: {port_str:?}"))?;
        if port == 0 {
            anyhow::bail!("listen port for {p} must be between 1 and 65535");
        }
        self.ports.insert(p.clone(), port);
        fi += 1;
        let remark = self.detail_fields[fi].value.trim().to_string();
        if !remark.is_empty() {
            extra.insert("remark".into(), remark);
        }
        fi += 1;
        let sni = self.detail_fields[fi].value.trim().to_string();
        if !sni.is_empty() {
            extra.insert("sni".into(), sni);
        }
        fi += 1;

        if matches!(p.as_str(), "hysteria2" | "tuic" | "anytls" | "ss2022") {
            let mut pass = self.detail_fields[fi].value.trim().to_string();
            if pass.is_empty() {
                pass = if p == "ss2022" {
                    random_ss2022_secret()
                } else {
                    random_secret()
                };
            }
            extra.insert("pass".into(), pass);
            fi += 1;
        }
        if matches!(p.as_str(), "reality" | "tuic") {
            let mut uuid = self.detail_fields[fi].value.trim().to_string();
            if uuid.is_empty() {
                uuid = Uuid::new_v4().to_string();
            }
            extra.insert("uuid".into(), uuid);
            fi += 1;
        }
        if p == "reality" {
            let pbk = self.detail_fields[fi].value.trim().to_string();
            if !pbk.is_empty() {
                extra.insert("pbk".into(), pbk);
            }
            fi += 1;
            let mut sid = self.detail_fields[fi].value.trim().to_string();
            if sid.is_empty() {
                sid = random_hex(4);
            }
            extra.insert("short_id".into(), sid);
        }
        self.creds.insert(p, extra);
        Ok(())
    }

    fn build_config(&mut self) -> anyhow::Result<SetupConfig> {
        self.save_current_detail()?;
        let mut cfg = SetupConfig::default_config();
        cfg.setup_mode = self.setup_mode();
        cfg.server_addr = self.identity[0].value.trim().to_string();
        if cfg.server_addr.is_empty() {
            anyhow::bail!("server address is required");
        }
        cfg.domain = self.identity[1].value.trim().to_string();
        cfg.email = self.identity[2].value.trim().to_string();
        let token = self.identity[3].value.trim().to_string();
        cfg.sub_token = if token.is_empty() {
            random_token()
        } else {
            token
        };
        let passcode = self.identity[4].value.trim().to_string();
        let passcode = if passcode.is_empty() {
            config::generate_link_passcode()
        } else {
            passcode
        };
        cfg.set_link_passcode(&passcode)?;
        cfg.sub_port = match self.identity[5].value.trim() {
            "" => 8964,
            v => v
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid subscription port: {v:?}"))?,
        };
        if cfg.sub_port == 0 {
            anyhow::bail!("subscription port must be between 1 and 65535");
        }
        cfg.enabled_protocols = self.enabled_protocols();
        if cfg.enabled_protocols.is_empty() {
            anyhow::bail!("select at least one protocol");
        }
        cfg.ports = self.ports.clone();
        cfg.creds = self.creds.clone();

        if self.needs_certs() {
            cfg.cert_mode = self.cert_strategy();
            match cfg.cert_mode {
                CertStrategy::AcmeDnsCf if !self.cert_extra.is_empty() => {
                    cfg.acme_cf_token = self.cert_extra[0].value.trim().to_string();
                }
                CertStrategy::Existing if self.cert_extra.len() >= 2 => {
                    cfg.cert_path = self.cert_extra[0].value.trim().to_string();
                    cfg.key_path = self.cert_extra[1].value.trim().to_string();
                }
                _ => {}
            }
        } else {
            cfg.cert_mode = CertStrategy::SelfSigned;
        }
        Ok(cfg)
    }

    fn focused_field(&mut self) -> Option<&mut Field> {
        match self.step {
            Step::Identity => self.identity.get_mut(self.identity_focus),
            Step::Certs if self.show_cert_extra => self.cert_extra.get_mut(self.cert_extra_focus),
            Step::Details => self.detail_fields.get_mut(self.detail_focus),
            _ => None,
        }
    }

    fn type_into(&mut self, code: KeyCode, mods: KeyModifiers) {
        if let Some(field) = self.focused_field() {
            let edit = match code {
                KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => Edit::Clear,
                KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => Edit::Insert(c),
                KeyCode::Backspace => Edit::Backspace,
                KeyCode::Delete => Edit::Delete,
                KeyCode::Left => Edit::Left,
                KeyCode::Right => Edit::Right,
                KeyCode::Home => Edit::Home,
                KeyCode::End => Edit::End,
                _ => return,
            };
            edit_text(&mut field.value, &mut field.pos, edit);
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        if let Some(field) = self.focused_field() {
            insert_text(&mut field.value, &mut field.pos, text);
        }
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> anyhow::Result<bool> {
        // returns true when done
        if self.dns_warning.is_some() {
            match code {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.dns_cursor = (self.dns_cursor + 2) % 3;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.dns_cursor = (self.dns_cursor + 1) % 3;
                }
                KeyCode::Enter => match self.dns_cursor {
                    1 => {
                        self.dns_warning = None;
                        self.step = Step::Protocols;
                        self.max_reached = self.max_reached.max(self.step as usize);
                    }
                    2 => {
                        return Err(anyhow::anyhow!(
                            "DNS for {} does not point at this server",
                            self.identity[1].value.trim()
                        ));
                    }
                    _ => {
                        // Back to edit: keep everything typed.
                        self.dns_warning = None;
                    }
                },
                KeyCode::Esc => {
                    self.dns_warning = None;
                }
                KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => {
                    return Err(cancel_err());
                }
                _ => {}
            }
            return Ok(false);
        }

        // Global
        match code {
            KeyCode::Esc => return Err(cancel_err()),
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => return Err(cancel_err()),
            KeyCode::Char('q') if self.step == Step::Review => return Err(cancel_err()),
            _ => {}
        }

        match self.step {
            Step::Mode => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.mode_cursor = self.mode_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.mode_cursor = (self.mode_cursor + 1).min(1);
                }
                KeyCode::Enter | KeyCode::Right => {
                    if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                _ => {}
            },
            Step::Identity => match code {
                KeyCode::Up | KeyCode::BackTab => {
                    self.identity_focus = self.identity_focus.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Tab => {
                    if self.identity_focus + 1 < self.identity.len() {
                        self.identity_focus += 1;
                    }
                }
                KeyCode::Enter => {
                    if self.identity_focus + 1 < self.identity.len() {
                        self.identity_focus += 1;
                    } else if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                KeyCode::Left if mods.contains(KeyModifiers::CONTROL) => self.go_back(),
                other => self.type_into(other, mods),
            },
            Step::Protocols => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.proto_cursor = self.proto_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.proto_cursor = (self.proto_cursor + 1).min(PROTO_OPTIONS.len() - 1);
                }
                KeyCode::Char(' ') => {
                    self.proto_selected[self.proto_cursor] = !self.proto_selected[self.proto_cursor];
                }
                KeyCode::Char('a') => self.proto_selected = [true; 5],
                KeyCode::Char('n') => self.proto_selected = [false; 5],
                KeyCode::Enter | KeyCode::Right => {
                    if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                KeyCode::Left | KeyCode::BackTab => self.go_back(),
                _ => {}
            },
            Step::Certs => {
                if self.show_cert_extra {
                    match code {
                        KeyCode::Up | KeyCode::BackTab => {
                            self.cert_extra_focus = self.cert_extra_focus.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            if self.cert_extra_focus + 1 < self.cert_extra.len() {
                                self.cert_extra_focus += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if self.cert_extra_focus + 1 < self.cert_extra.len() {
                                self.cert_extra_focus += 1;
                            } else if let Err(e) = self.go_next() {
                                self.error = Some(e.to_string());
                            }
                        }
                        KeyCode::Left if mods.contains(KeyModifiers::CONTROL) => self.go_back(),
                        other => self.type_into(other, mods),
                    }
                } else {
                    match code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.cert_cursor = self.cert_cursor.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.cert_cursor = (self.cert_cursor + 1).min(CERT_OPTIONS.len() - 1);
                        }
                        KeyCode::Enter | KeyCode::Right => {
                            if let Err(e) = self.go_next() {
                                self.error = Some(e.to_string());
                            }
                        }
                        KeyCode::Left | KeyCode::BackTab => self.go_back(),
                        _ => {}
                    }
                }
            }
            Step::Details => match code {
                KeyCode::Up | KeyCode::BackTab => {
                    self.detail_focus = self.detail_focus.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Tab => {
                    if self.detail_focus + 1 < self.detail_fields.len() {
                        self.detail_focus += 1;
                    }
                }
                KeyCode::Enter => {
                    if self.detail_focus + 1 < self.detail_fields.len() {
                        self.detail_focus += 1;
                    } else if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                KeyCode::Left if mods.contains(KeyModifiers::CONTROL) => self.go_back(),
                other => self.type_into(other, mods),
            },
            Step::Review => match code {
                KeyCode::Enter => self.finished = true,
                KeyCode::Char('s') if mods.contains(KeyModifiers::CONTROL) => {
                    self.finished = true;
                }
                KeyCode::Left | KeyCode::BackTab => self.go_back(),
                _ => {}
            },
        }
        Ok(self.finished)
    }

    fn draw(&self, f: &mut ratatui::Frame) {
        let area = f.area();
        let (header, sidebar, main, footer) = root_chunks(area);

        let subtitle = if self.detected_ip.is_empty() {
            "configure a sing-box edge".into()
        } else {
            format!("detected {}", self.detected_ip)
        };
        draw_header(f, header, "setup wizard", &subtitle);
        draw_sidebar(f, sidebar, &self.step_metas());

        if let Some(ref warn) = self.dns_warning {
            self.draw_dns_modal(f, main, warn);
        } else {
            match self.step {
                Step::Mode => self.draw_mode(f, main),
                Step::Identity => self.draw_identity(f, main),
                Step::Protocols => self.draw_protocols(f, main),
                Step::Certs => self.draw_certs(f, main),
                Step::Details => self.draw_details(f, main),
                Step::Review => self.draw_review(f, main),
            }
        }

        if let Some(ref err) = self.error {
            let err_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(main)[1];
            f.render_widget(
                ratatui::widgets::Paragraph::new(Span::styled(
                    format!(" ⚠ {err}"),
                    Theme::danger(),
                )),
                err_area,
            );
        }

        let hints = self.footer_hints();
        draw_footer(f, footer, &hints);
    }

    fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        if self.dns_warning.is_some() {
            return vec![
                ("←/→", "choice"),
                ("Enter", "confirm"),
                ("Esc", "back"),
            ];
        }
        match self.step {
            Step::Mode => vec![
                ("↑↓", "select"),
                ("Enter", "next"),
                ("Esc", "quit"),
            ],
            Step::Identity | Step::Details => vec![
                ("Tab", "field"),
                ("Enter", "next"),
                ("^←", "back"),
                ("Esc", "quit"),
            ],
            Step::Protocols => vec![
                ("↑↓", "move"),
                ("Space", "toggle"),
                ("a/n", "all/none"),
                ("Enter", "next"),
                ("Esc", "quit"),
            ],
            Step::Certs if !self.show_cert_extra => vec![
                ("↑↓", "select"),
                ("Enter", "next"),
                ("←", "back"),
                ("Esc", "quit"),
            ],
            Step::Certs => vec![
                ("Tab", "field"),
                ("Enter", "next"),
                ("Esc", "quit"),
            ],
            Step::Review => vec![
                ("Enter", "apply setup"),
                ("←", "back"),
                ("Esc", "quit"),
            ],
        }
    }

    fn draw_mode(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let options = [
            "Production domain setup (recommended)".to_string(),
            "IP-only / advanced setup".to_string(),
        ];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);
        f.render_widget(
            ratatui::widgets::Paragraph::new(
                "Production verifies DNS, certificates, services, and subscription before claiming success.",
            )
            .style(Theme::muted())
            .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[0],
        );
        draw_option_list(
            f,
            chunks[1],
            "setup mode",
            &options,
            self.mode_cursor,
            None,
        );
    }

    fn draw_identity(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let fields: Vec<(String, String, bool, usize)> = self
            .identity
            .iter()
            .map(|fld| (fld.label.to_string(), fld.value.clone(), fld.password, fld.pos))
            .collect();
        let help = self
            .identity
            .get(self.identity_focus)
            .map(|f| f.help)
            .unwrap_or("");
        draw_fields(f, area, "server & domain", &fields, self.identity_focus, help);
    }

    fn draw_protocols(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let options: Vec<String> = PROTO_OPTIONS
            .iter()
            .map(|(id, desc)| format!("{id:<10}  {desc}"))
            .collect();
        draw_option_list(
            f,
            area,
            "protocols  ·  space toggles",
            &options,
            self.proto_cursor,
            Some(&self.proto_selected),
        );
    }

    fn draw_certs(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if self.show_cert_extra {
            let fields: Vec<(String, String, bool, usize)> = self
                .cert_extra
                .iter()
                .map(|fld| (fld.label.to_string(), fld.value.clone(), fld.password, fld.pos))
                .collect();
            let help = self
                .cert_extra
                .get(self.cert_extra_focus)
                .map(|f| f.help)
                .unwrap_or("");
            draw_fields(
                f,
                area,
                "certificate details",
                &fields,
                self.cert_extra_focus,
                help,
            );
        } else {
            let options: Vec<String> = CERT_OPTIONS.iter().map(|s| (*s).to_string()).collect();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(5)])
                .split(area);
            f.render_widget(
                ratatui::widgets::Paragraph::new(
                    "Hysteria2, TUIC, and AnyTLS need readable cert/key files before sing-box can start.",
                )
                .style(Theme::muted()),
                chunks[0],
            );
            draw_option_list(
                f,
                chunks[1],
                "certificate strategy",
                &options,
                self.cert_cursor,
                None,
            );
        }
    }

    fn draw_details(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let p = self
            .detail_protos
            .get(self.detail_idx)
            .map(|s| s.as_str())
            .unwrap_or("?");
        let title = format!(
            "node  {}/{}  ·  {p}",
            self.detail_idx + 1,
            self.detail_protos.len().max(1)
        );
        let fields: Vec<(String, String, bool, usize)> = self
            .detail_fields
            .iter()
            .map(|fld| (fld.label.to_string(), fld.value.clone(), fld.password, fld.pos))
            .collect();
        let help = self
            .detail_fields
            .get(self.detail_focus)
            .map(|f| f.help)
            .unwrap_or("");
        draw_fields(f, area, &title, &fields, self.detail_focus, help);
    }

    fn draw_review(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let protos = self.enabled_protocols().join(", ");
        let lines = vec![
            Line::from(Span::styled(
                "  Ready to apply",
                Theme::title(),
            )),
            Line::from(""),
            kv_line("mode", self.setup_mode().to_string()),
            kv_line("server", self.identity[0].value.clone()),
            kv_line("domain", {
                let d = self.identity[1].value.clone();
                if d.is_empty() {
                    "(none)".into()
                } else {
                    d
                }
            }),
            kv_line("sub port", self.identity[5].value.clone()),
            kv_line("protocols", protos),
            kv_line(
                "certs",
                if self.needs_certs() {
                    self.cert_strategy().to_string()
                } else {
                    "not required".into()
                },
            ),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Enter to write configs, fetch binaries, and start services.",
                Theme::muted(),
            )),
            Line::from(Span::styled(
                "  Demo mode (SUB_MAKER_DEMO=1) skips real service enable.",
                Theme::muted(),
            )),
        ];
        draw_review(f, area, lines);
    }

    fn draw_dns_modal(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect, warn: &str) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::danger())
            .title(Span::styled(" DNS mismatch ", Theme::danger()));
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(inner);

        f.render_widget(
            Paragraph::new(warn.to_string())
                .style(Theme::body())
                .wrap(Wrap { trim: true }),
            chunks[0],
        );

        let style_for = |i: usize| {
            if self.dns_cursor == i {
                Theme::selected()
            } else {
                Theme::muted()
            }
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Back to edit  ", style_for(0)),
                Span::raw("   "),
                Span::styled("  Continue anyway  ", style_for(1)),
                Span::raw("   "),
                Span::styled("  Abort setup  ", style_for(2)),
            ])),
            chunks[1],
        );
    }
}

pub(crate) fn format_dns_mismatch(
    domain: &str,
    expected_ipv4: &str,
    expected_ipv6: &str,
    result: &DnsCheckResult,
    lookup_err: Option<&anyhow::Error>,
) -> String {
    let mut b = String::new();
    if let Some(err) = lookup_err {
        b.push_str(&format!("Lookup error: {err}\n\n"));
    }
    if !expected_ipv4.is_empty() {
        b.push_str(&format!("Create:  A {domain} → {expected_ipv4}\n"));
    }
    if !expected_ipv6.is_empty() {
        b.push_str(&format!("Create:  AAAA {domain} → {expected_ipv6}\n"));
    }
    if !result.resolved_a.is_empty() {
        b.push_str(&format!("Current A:    {}\n", result.resolved_a.join(", ")));
    }
    if !result.resolved_aaaa.is_empty() {
        b.push_str(&format!(
            "Current AAAA: {}\n",
            result.resolved_aaaa.join(", ")
        ));
    }
    b.push_str("\nACME HTTP-01 and domain clients may fail until DNS is fixed.");
    b
}

/// Run the full interactive setup wizard.
pub fn run_wizard() -> anyhow::Result<SetupConfig> {
    let mut tui = Tui::new()?;
    let mut wiz = Wizard::new();

    loop {
        tui.draw(|f| wiz.draw(f))?;
        match next_input(POLL)? {
            Some(Input::Key(key)) => match wiz.handle_key(key.code, key.modifiers) {
                Ok(true) => break,
                Ok(false) => {}
                Err(e) => return Err(e),
            },
            Some(Input::Paste(text)) => wiz.handle_paste(&text),
            None => {}
        }
    }

    // Drop TUI before returning so the terminal is restored before apply logs.
    drop(tui);
    let cfg = wiz.build_config()?;
    info!(
        enabled = ?cfg.enabled_protocols,
        port = cfg.sub_port,
        "TUI collection complete"
    );
    Ok(cfg)
}

