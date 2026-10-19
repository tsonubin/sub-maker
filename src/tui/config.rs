//! Multi-step config editor — same chrome as setup, prefilled values.

use std::collections::HashMap;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use tracing::info;
use uuid::Uuid;

use crate::config::{CertStrategy, SetupConfig, SetupMode};
use crate::diagnostics;
use crate::tui::terminal::{cancel_err, next_key, Tui};
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    draw_fields, draw_footer, draw_header, draw_option_list, draw_review, draw_sidebar, kv_line,
    root_chunks, StepMeta, StepStatus,
};
use crate::tui::wizard::format_dns_mismatch;
use crate::tui::{
    capitalize_first, random_hex, random_secret, random_ss2022_secret, random_token,
};

const POLL: Duration = Duration::from_millis(80);

const PROTO_OPTIONS: &[(&str, &str)] = &[
    ("reality", "VLESS + Reality-Vision"),
    ("hysteria2", "Hysteria2"),
    ("tuic", "TUIC v5"),
    ("anytls", "AnyTLS"),
    ("ss2022", "Shadowsocks 2022"),
];

const CERT_OPTIONS: &[&str] = &[
    "Certbot HTTP-01",
    "acme.sh HTTP-01",
    "ACME DNS-01 Cloudflare",
    "Existing certificate files",
    "Self-signed",
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
}

struct Editor {
    step: Step,
    mode_cursor: usize,
    detected_ip: String,
    identity: Vec<Field>,
    identity_focus: usize,
    proto_selected: [bool; 5],
    proto_cursor: usize,
    cert_cursor: usize,
    cert_extra: Vec<Field>,
    cert_extra_focus: usize,
    show_cert_extra: bool,
    detail_protos: Vec<String>,
    detail_idx: usize,
    detail_fields: Vec<Field>,
    detail_focus: usize,
    creds: HashMap<String, HashMap<String, String>>,
    ports: HashMap<String, u16>,
    /// Original passcode hash — blank passcode field keeps it.
    keep_passcode: bool,
    original: SetupConfig,
    dns_warning: Option<String>,
    dns_cursor: usize,
    error: Option<String>,
    finished: bool,
}

impl Editor {
    fn new(existing: SetupConfig) -> Self {
        let mut cfg = existing.clone();
        apply_config_defaults(&mut cfg);

        let mut detected_ip = String::new();
        match diagnostics::detect_public_ip() {
            Ok(ip) => {
                info!(ip = %ip, "detected server public IP");
                detected_ip = ip;
            }
            Err(err) => info!(error = %err, "could not auto-detect server public IP"),
        }

        let mut proto_selected = [false; 5];
        for (i, (id, _)) in PROTO_OPTIONS.iter().enumerate() {
            proto_selected[i] = cfg.enabled_protocols.is_empty()
                || cfg.enabled_protocols.iter().any(|p| p == id);
        }

        let cert_cursor = match cfg.cert_mode {
            CertStrategy::CertbotHttp => 0,
            CertStrategy::AcmeHttp => 1,
            CertStrategy::AcmeDnsCf => 2,
            CertStrategy::Existing => 3,
            CertStrategy::SelfSigned => 4,
        };

        let identity = vec![
            Field {
                label: "Server address",
                help: "Public IP or domain for client links",
                value: cfg.server_addr.clone(),
                password: false,
            },
            Field {
                label: "Domain",
                help: "Required in production mode",
                value: cfg.domain.clone(),
                password: false,
            },
            Field {
                label: "ACME email",
                help: "Contact for certificate issuance",
                value: cfg.email.clone(),
                password: false,
            },
            Field {
                label: "Subscription token",
                help: "Changing this invalidates old URLs",
                value: cfg.sub_token.clone(),
                password: false,
            },
            Field {
                label: "New SSH passcode",
                help: "Blank keeps the current passcode",
                value: String::new(),
                password: true,
            },
            Field {
                label: "Sub port",
                help: "Subscription listen port",
                value: cfg.sub_port.to_string(),
                password: false,
            },
        ];

        let mut cert_extra = Vec::new();
        if !cfg.acme_cf_token.is_empty() {
            cert_extra.push(Field {
                label: "Cloudflare API token",
                help: "DNS edit scope",
                value: cfg.acme_cf_token.clone(),
                password: true,
            });
        }
        if !cfg.cert_path.is_empty() || !cfg.key_path.is_empty() {
            // populated when entering existing mode
        }

        Self {
            step: Step::Mode,
            mode_cursor: if cfg.setup_mode == SetupMode::IpOnly {
                1
            } else {
                0
            },
            detected_ip,
            identity,
            identity_focus: 0,
            proto_selected,
            proto_cursor: 0,
            cert_cursor,
            cert_extra,
            cert_extra_focus: 0,
            show_cert_extra: false,
            detail_protos: Vec::new(),
            detail_idx: 0,
            detail_fields: Vec::new(),
            detail_focus: 0,
            creds: cfg.creds.clone(),
            ports: cfg.ports.clone(),
            keep_passcode: true,
            original: cfg,
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
        [
            Step::Mode,
            Step::Identity,
            Step::Protocols,
            Step::Certs,
            Step::Details,
            Step::Review,
        ]
        .iter()
        .map(|s| {
            let i = *s as usize;
            let cur = self.step as usize;
            StepMeta {
                label: s.label(),
                status: if i < cur {
                    StepStatus::Done
                } else if i == cur {
                    StepStatus::Current
                } else {
                    StepStatus::Pending
                },
            }
        })
        .collect()
    }

    fn type_into(field: &mut Field, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => field.value.clear(),
            KeyCode::Backspace => {
                field.value.pop();
            }
            KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => field.value.push(c),
            _ => {}
        }
    }

    fn go_next(&mut self) -> anyhow::Result<()> {
        self.error = None;
        match self.step {
            Step::Mode => self.step = Step::Identity,
            Step::Identity => {
                if self.identity[0].value.trim().is_empty() && self.original.server_addr.is_empty()
                {
                    anyhow::bail!(
                        "server address is required (public IP or domain used in the share links)"
                    );
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
                if self.setup_mode() == SetupMode::Production
                    && self.identity[1].value.trim().is_empty()
                {
                    anyhow::bail!("domain is required for production setup");
                }
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
                    self.step = Step::Certs;
                    self.prepare_cert_extra();
                } else {
                    self.enter_details();
                }
            }
            Step::Certs => {
                if self.show_cert_extra {
                    self.apply_cert_extra()?;
                } else if matches!(
                    self.cert_strategy(),
                    CertStrategy::AcmeDnsCf | CertStrategy::Existing
                ) {
                    self.prepare_cert_extra();
                    self.show_cert_extra = true;
                    self.cert_extra_focus = 0;
                    return Ok(());
                }
                self.enter_details();
            }
            Step::Details => {
                self.save_detail()?;
                if self.detail_idx + 1 < self.detail_protos.len() {
                    self.detail_idx += 1;
                    self.load_detail();
                } else {
                    self.step = Step::Review;
                }
            }
            Step::Review => self.finished = true,
        }
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
                    if let Err(e) = self.save_detail() {
                        self.error = Some(e.to_string());
                        return;
                    }
                    self.detail_idx -= 1;
                    self.load_detail();
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
                    self.load_detail();
                }
            }
        }
    }

    fn check_dns(&self, domain: &str) -> Option<String> {
        let mut expected = self.identity[0].value.trim().to_string();
        let (v4, v6) = diagnostics::split_expected_ips(&expected);
        if v4.is_empty() && v6.is_empty() && !self.detected_ip.is_empty() {
            expected = self.detected_ip.clone();
        }
        let (e4, e6) = diagnostics::split_expected_ips(&expected);
        let (result, err) = diagnostics::check_domain_dns(domain, &e4, &e6);
        if err.is_none() && diagnostics::dns_matches(&result) {
            None
        } else {
            Some(format_dns_mismatch(domain, &e4, &e6, &result, err.as_ref()))
        }
    }

    fn prepare_cert_extra(&mut self) {
        let previous = std::mem::take(&mut self.cert_extra);
        let mut fields = match self.cert_strategy() {
            CertStrategy::AcmeDnsCf => vec![Field {
                label: "Cloudflare API token",
                help: "Blank keeps existing token",
                value: self.original.acme_cf_token.clone(),
                password: true,
            }],
            CertStrategy::Existing => vec![
                Field {
                    label: "Certificate path",
                    help: "fullchain.pem",
                    value: if self.original.cert_path.is_empty() {
                        "/etc/sub-maker/certs/fullchain.pem".into()
                    } else {
                        self.original.cert_path.clone()
                    },
                    password: false,
                },
                Field {
                    label: "Private key path",
                    help: "privkey.pem",
                    value: if self.original.key_path.is_empty() {
                        "/etc/sub-maker/certs/privkey.pem".into()
                    } else {
                        self.original.key_path.clone()
                    },
                    password: false,
                },
            ],
            _ => vec![],
        };
        // Keep in-progress edits when re-entering with the same field set.
        if previous.len() == fields.len()
            && previous
                .iter()
                .zip(fields.iter())
                .all(|(old, new)| old.label == new.label)
        {
            for (old, new) in previous.into_iter().zip(fields.iter_mut()) {
                new.value = old.value;
            }
        }
        self.cert_extra = fields;
    }

    fn apply_cert_extra(&mut self) -> anyhow::Result<()> {
        if self.cert_strategy() == CertStrategy::Existing && self.cert_extra.len() >= 2 {
            diagnostics::validate_certificate_pair(
                self.cert_extra[0].value.trim(),
                self.cert_extra[1].value.trim(),
                self.identity[1].value.trim(),
            )?;
        }
        self.show_cert_extra = false;
        Ok(())
    }

    fn enter_details(&mut self) {
        self.detail_protos = self.enabled_protocols();
        self.detail_idx = 0;
        self.load_detail();
        self.step = Step::Details;
    }

    fn load_detail(&mut self) {
        let p = &self.detail_protos[self.detail_idx];
        let existing = self.creds.get(p).cloned().unwrap_or_default();
        let port = self.ports.get(p).copied().unwrap_or(0);
        let remark = existing
            .get("remark")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| capitalize_first(p));
        let default_sni = existing.get("sni").cloned().unwrap_or_else(|| {
            if p == "reality" {
                "www.apple.com".into()
            } else {
                self.identity[1].value.trim().to_string()
            }
        });

        let mut fields = vec![
            Field {
                label: "Listen port",
                help: "Inbound port",
                value: port.to_string(),
                password: false,
            },
            Field {
                label: "Remark / tag",
                help: "Client display name",
                value: remark,
                password: false,
            },
            Field {
                label: "SNI / target",
                help: "TLS SNI or Reality target",
                value: default_sni,
                password: false,
            },
        ];
        if matches!(p.as_str(), "hysteria2" | "tuic" | "anytls" | "ss2022") {
            fields.push(Field {
                label: "Password",
                help: "Blank keeps current secret",
                value: String::new(),
                password: true,
            });
        }
        if matches!(p.as_str(), "reality" | "tuic") {
            fields.push(Field {
                label: "UUID",
                help: "Blank keeps current UUID",
                value: existing.get("uuid").cloned().unwrap_or_default(),
                password: false,
            });
        }
        if p == "reality" {
            fields.push(Field {
                label: "Reality public key",
                help: "Blank keeps existing",
                value: existing.get("pbk").cloned().unwrap_or_default(),
                password: false,
            });
            fields.push(Field {
                label: "Reality shortId",
                help: "8 hex · blank auto if empty",
                value: existing.get("short_id").cloned().unwrap_or_default(),
                password: false,
            });
        }
        self.detail_fields = fields;
        self.detail_focus = 0;
    }

    fn save_detail(&mut self) -> anyhow::Result<()> {
        if self.detail_protos.is_empty() {
            return Ok(());
        }
        let p = self.detail_protos[self.detail_idx].clone();
        let mut extra = self.creds.get(&p).cloned().unwrap_or_default();
        let mut fi = 0;
        let port_str = self.detail_fields[fi].value.trim();
        if !port_str.is_empty() {
            let port: u16 = port_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid listen port for {p}: {port_str:?}"))?;
            if port == 0 {
                anyhow::bail!("listen port for {p} must be between 1 and 65535");
            }
            self.ports.insert(p.clone(), port);
        }
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
            let pass = self.detail_fields[fi].value.trim().to_string();
            if !pass.is_empty() {
                extra.insert("pass".into(), pass);
            } else if extra.get("pass").map(|s| s.is_empty()).unwrap_or(true) {
                let gen = if p == "ss2022" {
                    random_ss2022_secret()
                } else {
                    random_secret()
                };
                extra.insert("pass".into(), gen);
            }
            fi += 1;
        }
        if matches!(p.as_str(), "reality" | "tuic") {
            let mut uuid = self.detail_fields[fi].value.trim().to_string();
            if uuid.is_empty() {
                uuid = extra
                    .get("uuid")
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
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
                sid = extra
                    .get("short_id")
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| random_hex(4));
            }
            extra.insert("short_id".into(), sid);
        }
        self.creds.insert(p, extra);
        Ok(())
    }

    fn build_config(&mut self) -> anyhow::Result<SetupConfig> {
        self.save_detail()?;
        let mut cfg = self.original.clone();
        cfg.setup_mode = self.setup_mode();
        cfg.server_addr = self.identity[0].value.trim().to_string();
        if cfg.server_addr.is_empty() {
            cfg.server_addr = self.original.server_addr.clone();
        }
        cfg.domain = self.identity[1].value.trim().to_string();
        cfg.email = self.identity[2].value.trim().to_string();
        cfg.sub_token = self.identity[3].value.trim().to_string();
        if cfg.sub_token.is_empty() {
            cfg.sub_token = random_token();
        }
        let new_pass = self.identity[4].value.trim().to_string();
        if !new_pass.is_empty() {
            cfg.set_link_passcode(&new_pass)?;
            self.keep_passcode = false;
        }
        cfg.sub_port = match self.identity[5].value.trim() {
            "" => self.original.sub_port,
            v => v
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid subscription port: {v:?}"))?,
        };
        if cfg.sub_port == 0 {
            cfg.sub_port = 8964;
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
                    let t = self.cert_extra[0].value.trim().to_string();
                    if !t.is_empty() {
                        cfg.acme_cf_token = t;
                    }
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
        let _ = self.keep_passcode;
        Ok(cfg)
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> anyhow::Result<bool> {
        if self.dns_warning.is_some() {
            match code {
                KeyCode::Left | KeyCode::Right => self.dns_cursor = 1 - self.dns_cursor,
                KeyCode::Enter => {
                    if self.dns_cursor == 1 {
                        self.dns_warning = None;
                        self.step = Step::Protocols;
                    } else {
                        anyhow::bail!(
                            "DNS for {} does not point at this server",
                            self.identity[1].value.trim()
                        );
                    }
                }
                KeyCode::Esc => self.dns_warning = None,
                KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => {
                    return Err(cancel_err());
                }
                _ => {}
            }
            return Ok(false);
        }

        match code {
            KeyCode::Esc => return Err(cancel_err()),
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => {
                return Err(cancel_err());
            }
            _ => {}
        }

        match self.step {
            Step::Mode => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.mode_cursor = self.mode_cursor.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.mode_cursor = (self.mode_cursor + 1).min(1)
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
                    self.identity_focus = self.identity_focus.saturating_sub(1)
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
                other => Self::type_into(&mut self.identity[self.identity_focus], other, mods),
            },
            Step::Protocols => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.proto_cursor = self.proto_cursor.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.proto_cursor = (self.proto_cursor + 1).min(4)
                }
                KeyCode::Char(' ') => {
                    self.proto_selected[self.proto_cursor] = !self.proto_selected[self.proto_cursor]
                }
                KeyCode::Char('a') => self.proto_selected = [true; 5],
                KeyCode::Char('n') => self.proto_selected = [false; 5],
                KeyCode::Enter | KeyCode::Right => {
                    if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                KeyCode::Left => self.go_back(),
                _ => {}
            },
            Step::Certs if self.show_cert_extra => match code {
                KeyCode::Up | KeyCode::BackTab => {
                    self.cert_extra_focus = self.cert_extra_focus.saturating_sub(1)
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
                other if !self.cert_extra.is_empty() => {
                    Self::type_into(&mut self.cert_extra[self.cert_extra_focus], other, mods)
                }
                _ => {}
            },
            Step::Certs => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.cert_cursor = self.cert_cursor.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.cert_cursor = (self.cert_cursor + 1).min(CERT_OPTIONS.len() - 1)
                }
                KeyCode::Enter | KeyCode::Right => {
                    if let Err(e) = self.go_next() {
                        self.error = Some(e.to_string());
                    }
                }
                KeyCode::Left => self.go_back(),
                _ => {}
            },
            Step::Details => match code {
                KeyCode::Up | KeyCode::BackTab => {
                    self.detail_focus = self.detail_focus.saturating_sub(1)
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
                other if !self.detail_fields.is_empty() => {
                    Self::type_into(&mut self.detail_fields[self.detail_focus], other, mods)
                }
                _ => {}
            },
            Step::Review => match code {
                KeyCode::Enter => self.finished = true,
                KeyCode::Char('s') if mods.contains(KeyModifiers::CONTROL) => {
                    self.finished = true
                }
                KeyCode::Left | KeyCode::BackTab => self.go_back(),
                _ => {}
            },
        }
        Ok(self.finished)
    }

    fn draw(&self, f: &mut ratatui::Frame) {
        let (header, sidebar, main, footer) = root_chunks(f.area());
        let sub = if self.detected_ip.is_empty() {
            "edit existing setup".into()
        } else {
            format!("detected {} · blank secrets keep current", self.detected_ip)
        };
        draw_header(f, header, "config editor", &sub);
        draw_sidebar(f, sidebar, &self.step_metas());

        if let Some(ref warn) = self.dns_warning {
            use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::danger())
                .title(Span::styled(" DNS mismatch ", Theme::danger()));
            let inner = block.inner(main);
            f.render_widget(Clear, main);
            f.render_widget(block, main);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(inner);
            f.render_widget(
                Paragraph::new(warn.clone())
                    .style(Theme::body())
                    .wrap(Wrap { trim: true }),
                chunks[0],
            );
            let yes = self.dns_cursor == 1;
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "  Abort  ",
                        if !yes {
                            Theme::selected()
                        } else {
                            Theme::muted()
                        },
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "  Continue  ",
                        if yes {
                            Theme::selected()
                        } else {
                            Theme::muted()
                        },
                    ),
                ])),
                chunks[1],
            );
        } else {
            match self.step {
                Step::Mode => {
                    let opts = [
                        "Production domain setup".into(),
                        "IP-only / advanced setup".into(),
                    ];
                    draw_option_list(f, main, "setup mode", &opts, self.mode_cursor, None);
                }
                Step::Identity => {
                    let fields: Vec<_> = self
                        .identity
                        .iter()
                        .map(|x| (x.label.to_string(), x.value.clone(), x.password))
                        .collect();
                    let help = self
                        .identity
                        .get(self.identity_focus)
                        .map(|x| x.help)
                        .unwrap_or("");
                    draw_fields(f, main, "server & domain", &fields, self.identity_focus, help);
                }
                Step::Protocols => {
                    let opts: Vec<String> = PROTO_OPTIONS
                        .iter()
                        .map(|(id, d)| format!("{id:<10}  {d}"))
                        .collect();
                    draw_option_list(
                        f,
                        main,
                        "protocols",
                        &opts,
                        self.proto_cursor,
                        Some(&self.proto_selected),
                    );
                }
                Step::Certs if self.show_cert_extra => {
                    let fields: Vec<_> = self
                        .cert_extra
                        .iter()
                        .map(|x| (x.label.to_string(), x.value.clone(), x.password))
                        .collect();
                    let help = self
                        .cert_extra
                        .get(self.cert_extra_focus)
                        .map(|x| x.help)
                        .unwrap_or("");
                    draw_fields(
                        f,
                        main,
                        "certificate details",
                        &fields,
                        self.cert_extra_focus,
                        help,
                    );
                }
                Step::Certs => {
                    let opts: Vec<String> = CERT_OPTIONS.iter().map(|s| (*s).into()).collect();
                    draw_option_list(
                        f,
                        main,
                        "certificate strategy",
                        &opts,
                        self.cert_cursor,
                        None,
                    );
                }
                Step::Details => {
                    let p = self
                        .detail_protos
                        .get(self.detail_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("?");
                    let title = format!(
                        "node {}/{} · {p}",
                        self.detail_idx + 1,
                        self.detail_protos.len().max(1)
                    );
                    let fields: Vec<_> = self
                        .detail_fields
                        .iter()
                        .map(|x| (x.label.to_string(), x.value.clone(), x.password))
                        .collect();
                    let help = self
                        .detail_fields
                        .get(self.detail_focus)
                        .map(|x| x.help)
                        .unwrap_or("");
                    draw_fields(f, main, &title, &fields, self.detail_focus, help);
                }
                Step::Review => {
                    let lines = vec![
                        Line::from(Span::styled("  Apply configuration changes", Theme::title())),
                        Line::from(""),
                        kv_line("mode", self.setup_mode().to_string()),
                        kv_line("server", self.identity[0].value.clone()),
                        kv_line("domain", self.identity[1].value.clone()),
                        kv_line("sub port", self.identity[5].value.clone()),
                        kv_line("protocols", self.enabled_protocols().join(", ")),
                        kv_line(
                            "certs",
                            if self.needs_certs() {
                                self.cert_strategy().to_string()
                            } else {
                                "n/a".into()
                            },
                        ),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  Enter applies · blank secrets keep existing values.",
                            Theme::muted(),
                        )),
                    ];
                    draw_review(f, main, lines);
                }
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

        draw_footer(
            f,
            footer,
            &[
                ("Tab/↑↓", "navigate"),
                ("Enter", "next / apply"),
                ("^←", "back"),
                ("Esc", "quit"),
            ],
        );
    }
}

fn apply_config_defaults(cfg: &mut SetupConfig) {
    let defaults = SetupConfig::default_config();
    if cfg.sub_port == 0 {
        cfg.sub_port = defaults.sub_port;
    }
    if cfg.enabled_protocols.is_empty() {
        cfg.enabled_protocols = defaults.enabled_protocols.clone();
    }
    for (protocol, port) in defaults.ports {
        cfg.ports.entry(protocol).or_insert(port);
    }
}

pub fn run_config_wizard(existing: SetupConfig) -> anyhow::Result<SetupConfig> {
    let mut tui = Tui::new()?;
    let mut ed = Editor::new(existing);

    loop {
        tui.draw(|f| ed.draw(f))?;
        if let Some(key) = next_key(POLL)? {
            match ed.handle_key(key.code, key.modifiers) {
                Ok(true) => break,
                Ok(false) => {}
                Err(e) => return Err(e),
            }
        }
    }
    drop(tui);
    let cfg = ed.build_config()?;
    info!(
        enabled = ?cfg.enabled_protocols,
        port = cfg.sub_port,
        "TUI configuration complete"
    );
    Ok(cfg)
}
