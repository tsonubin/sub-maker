use crate::config::SetupConfig;

#[derive(Debug, Clone, Default)]
pub struct Links {
    pub subscription: String,
    pub raw: String,
    pub ip_fallback: String,
}

pub fn build_links(cfg: &SetupConfig) -> Links {
    let host = if cfg.domain.is_empty() {
        cfg.server_addr.as_str()
    } else {
        cfg.domain.as_str()
    };
    Links {
        subscription: format!(
            "http://{}:{}/sub?token={}",
            host, cfg.sub_port, cfg.sub_token
        ),
        raw: format!(
            "http://{}:{}/raw?token={}",
            host, cfg.sub_port, cfg.sub_token
        ),
        ip_fallback: format!(
            "http://{}:{}/sub?token={}",
            cfg.server_addr, cfg.sub_port, cfg.sub_token
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SetupConfig;

    #[test]
    fn build_links_prefers_domain() {
        let links = build_links(&SetupConfig {
            server_addr: "43.245.198.178".into(),
            domain: "proxy.example.com".into(),
            sub_port: 8964,
            sub_token: "tok".into(),
            ..SetupConfig::default()
        });
        assert_eq!(
            links.subscription,
            "http://proxy.example.com:8964/sub?token=tok"
        );
        assert_eq!(
            links.ip_fallback,
            "http://43.245.198.178:8964/sub?token=tok"
        );
    }

    #[test]
    fn build_links_falls_back_to_ip() {
        let links = build_links(&SetupConfig {
            server_addr: "43.245.198.178".into(),
            sub_port: 8964,
            sub_token: "tok".into(),
            ..SetupConfig::default()
        });
        assert_eq!(
            links.subscription,
            "http://43.245.198.178:8964/sub?token=tok"
        );
    }
}
