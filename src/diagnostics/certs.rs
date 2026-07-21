use std::path::Path;
use std::time::SystemTime;
use x509_parser::prelude::*;

pub fn protocols_require_local_cert(protocols: &[String]) -> bool {
    protocols
        .iter()
        .any(|p| matches!(p.as_str(), "hysteria2" | "tuic" | "anytls"))
}

pub fn validate_certificate_pair(
    cert_path: &str,
    key_path: &str,
    domain: &str,
) -> anyhow::Result<()> {
    let cert_pem = std::fs::read(cert_path)
        .map_err(|e| anyhow::anyhow!("read certificate: {e}"))?;
    if !Path::new(key_path).exists() {
        anyhow::bail!("read private key: path not found: {key_path}");
    }

    let (_, pem) = parse_x509_pem(&cert_pem)
        .map_err(|e| anyhow::anyhow!("certificate file does not contain PEM data: {e}"))?;
    let (_, cert) = parse_x509_certificate(&pem.contents)
        .map_err(|e| anyhow::anyhow!("parse certificate: {e}"))?;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let not_before = cert.validity().not_before.timestamp();
    let not_after = cert.validity().not_after.timestamp();
    if now < not_before {
        anyhow::bail!(
            "certificate is not valid before {}",
            cert.validity()
                .not_before
                .to_rfc2822()
                .unwrap_or_else(|_| cert.validity().not_before.to_string())
        );
    }
    if now > not_after {
        anyhow::bail!(
            "certificate expired at {}",
            cert.validity()
                .not_after
                .to_rfc2822()
                .unwrap_or_else(|_| cert.validity().not_after.to_string())
        );
    }

    if !domain.is_empty() {
        // Check SAN DNS names and CN.
        let mut covers = false;
        if let Ok(Some(san)) = cert.subject_alternative_name() {
            for name in &san.value.general_names {
                if let x509_parser::extensions::GeneralName::DNSName(dns) = name {
                    if dns_matches(dns, domain) {
                        covers = true;
                        break;
                    }
                }
            }
        }
        if !covers {
            for attr in cert.subject().iter_common_name() {
                if let Ok(cn) = attr.as_str() {
                    if dns_matches(cn, domain) {
                        covers = true;
                        break;
                    }
                }
            }
        }
        if !covers {
            anyhow::bail!("certificate does not cover {domain}");
        }
    }
    Ok(())
}

fn dns_matches(pattern: &str, domain: &str) -> bool {
    if pattern.eq_ignore_ascii_case(domain) {
        return true;
    }
    if let Some(rest) = pattern.strip_prefix("*.") {
        // simple wildcard: *.example.com matches a.example.com
        if let Some((_, suffix)) = domain.split_once('.') {
            return suffix.eq_ignore_ascii_case(rest);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
    use std::fs;
    use std::time::{Duration, SystemTime};

    fn write_test_cert(dir: &std::path::Path, domain: &str) -> (String, String) {
        let mut params = CertificateParams::new(vec![domain.to_string()]).unwrap();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, domain);
        params.not_before = (SystemTime::now() - Duration::from_secs(3600)).into();
        params.not_after = (SystemTime::now() + Duration::from_secs(3600)).into();
        params
            .subject_alt_names
            .push(SanType::DnsName(domain.try_into().unwrap()));
        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();

        let cert_path = dir.join("fullchain.pem");
        let key_path = dir.join("privkey.pem");
        fs::write(&cert_path, cert.pem()).unwrap();
        fs::write(&key_path, key_pair.serialize_pem()).unwrap();
        (
            cert_path.to_string_lossy().into(),
            key_path.to_string_lossy().into(),
        )
    }

    #[test]
    fn protocols_require_local_cert_logic() {
        assert!(!protocols_require_local_cert(&[
            "reality".into(),
            "ss2022".into()
        ]));
        assert!(protocols_require_local_cert(&[
            "reality".into(),
            "hysteria2".into()
        ]));
    }

    #[test]
    fn validate_certificate_pair_accepts_matching_cert() {
        let dir = tempfile::tempdir().unwrap();
        let (cert_path, key_path) = write_test_cert(dir.path(), "proxy.example.com");
        validate_certificate_pair(&cert_path, &key_path, "proxy.example.com").unwrap();
    }

    #[test]
    fn validate_certificate_pair_rejects_wrong_domain() {
        let dir = tempfile::tempdir().unwrap();
        let (cert_path, key_path) = write_test_cert(dir.path(), "proxy.example.com");
        assert!(validate_certificate_pair(&cert_path, &key_path, "other.example.com").is_err());
    }
}
