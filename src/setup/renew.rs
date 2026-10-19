use std::fs;
use std::path::PathBuf;
use tracing::info;

use crate::config::{self, CertStrategy, SetupConfig};
use crate::diagnostics;
use crate::setup::certs::{
    copy_certbot_certs_to_etc, copy_certs_to_etc, generate_self_signed_cert,
};

/// Refreshes the managed certificate copies so sing-box keeps serving a valid
/// certificate after the ACME client renews the originals. Returns true when
/// the managed certificate content changed (caller should restart sing-box).
///
/// Issuance/renewal itself stays with the ACME client (certbot timer, acme.sh
/// cron); this only re-copies and validates — except self-signed certs, which
/// are regenerated when they expire within 14 days.
pub fn refresh_managed_certs(cfg: &SetupConfig) -> anyhow::Result<bool> {
    if !diagnostics::protocols_require_local_cert(&cfg.enabled_protocols) {
        info!("no enabled protocol needs a local certificate");
        return Ok(false);
    }
    let managed_dir = PathBuf::from(config::etc_base()).join("sub-maker").join("certs");
    let managed_cert = managed_dir.join("fullchain.pem");
    let managed_key = managed_dir.join("privkey.pem");
    let before = fs::read(&managed_cert).ok();

    // `existing` points at user-managed files; everything else uses the
    // managed copies under the etc base.
    let (cert_path, key_path) = if cfg.cert_mode == CertStrategy::Existing {
        (cfg.cert_path.clone(), cfg.key_path.clone())
    } else {
        (
            managed_cert.to_string_lossy().into_owned(),
            managed_key.to_string_lossy().into_owned(),
        )
    };

    if config::demo_mode() {
        // Demo never talks to ACME clients; mirror setup's demo behavior.
        if before.is_none() || expiring_soon(&managed_cert, 14)? {
            generate_self_signed_cert(&cfg.domain, &cfg.server_addr)?;
        }
    } else {
        match cfg.cert_mode {
            CertStrategy::CertbotHttp => copy_certbot_certs_to_etc(&cfg.domain)?,
            CertStrategy::AcmeHttp | CertStrategy::AcmeDnsCf => copy_certs_to_etc(&cfg.domain)?,
            CertStrategy::SelfSigned => {
                if before.is_none() || expiring_soon(&managed_cert, 14)? {
                    generate_self_signed_cert(&cfg.domain, &cfg.server_addr)?;
                }
            }
            CertStrategy::Existing => {
                // User-managed files; nothing to copy.
            }
        }
    }

    diagnostics::validate_certificate_pair(&cert_path, &key_path, &cfg.domain)?;
    let after = fs::read(&managed_cert).ok();
    Ok(before != after)
}

fn expiring_soon(cert_path: &PathBuf, days: i64) -> anyhow::Result<bool> {
    diagnostics::certificate_expires_within(&cert_path.to_string_lossy(), days)
}
