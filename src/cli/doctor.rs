use crate::cli::{config_path, load_config, nodes_path, print_links};
use crate::diagnostics;
use crate::setup::subconverter_installed;

const MANAGED_SERVICES: &[&str] = &["sing-box", "subconverter", "sub-maker-sub"];

pub fn doctor() -> anyhow::Result<()> {
    let cfg = load_config().map_err(|e| {
        anyhow::anyhow!("load config {}: {e}", config_path().display())
    })?;

    let mut ok = true;
    let mut check = |name: &str, err: Option<anyhow::Error>| {
        if let Some(e) = err {
            ok = false;
            println!("[FAIL] {name}: {e}");
        } else {
            println!("[ OK ] {name}");
        }
    };

    check("config file", None);
    let nodes_err = match std::fs::read(nodes_path()) {
        Ok(nodes) if nodes.is_empty() => Some(anyhow::anyhow!("nodes file is empty")),
        Ok(_) => None,
        Err(e) => Some(e.into()),
    };
    check("nodes file", nodes_err);

    let public_ip = diagnostics::detect_public_ip().unwrap_or_default();
    if !public_ip.is_empty() {
        println!("[INFO] detected public IP: {public_ip}");
    }

    if !cfg.domain.is_empty() {
        let (expected_ipv4, expected_ipv6) = if public_ip.is_empty() {
            diagnostics::split_expected_ips(&cfg.server_addr)
        } else {
            diagnostics::split_expected_ips(&public_ip)
        };
        let (result, dns_err) =
            diagnostics::check_domain_dns(&cfg.domain, &expected_ipv4, &expected_ipv6);
        let dns_err = match dns_err {
            Some(e) => Some(e),
            None if !diagnostics::dns_matches(&result) => Some(anyhow::anyhow!(
                "resolved A={:?} AAAA={:?}, expected {} {}",
                result.resolved_a,
                result.resolved_aaaa,
                expected_ipv4,
                expected_ipv6
            )),
            None => None,
        };
        check("domain DNS", dns_err);
    }

    if diagnostics::protocols_require_local_cert(&cfg.enabled_protocols) {
        check(
            "certificate files",
            diagnostics::validate_certificate_pair(&cfg.cert_path, &cfg.key_path, &cfg.domain)
                .err(),
        );
    }

    for service in MANAGED_SERVICES {
        if *service == "subconverter" && !subconverter_installed() {
            println!("[SKIP] service subconverter: binary not installed");
            continue;
        }
        check(
            &format!("service {service}"),
            diagnostics::service_active(service).err(),
        );
    }

    let local_url = format!(
        "http://127.0.0.1:{}/sub?token={}",
        cfg.sub_port, cfg.sub_token
    );
    check(
        "local subscription endpoint",
        diagnostics::verify_subscription_endpoint(&local_url).err(),
    );

    if let Err(err) = print_links() {
        check("links", Some(err));
    }
    if !ok {
        anyhow::bail!("doctor found failing checks");
    }
    Ok(())
}
