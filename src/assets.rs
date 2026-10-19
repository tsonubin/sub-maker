/// Embedded template contents (mirrors Go embed of templates/).

pub fn template(name: &str) -> Option<&'static str> {
    match name {
        "inbound_anytls.json.tpl" => Some(include_str!("../templates/inbound_anytls.json.tpl")),
        "inbound_hysteria2.json.tpl" => {
            Some(include_str!("../templates/inbound_hysteria2.json.tpl"))
        }
        "inbound_reality.json.tpl" => Some(include_str!("../templates/inbound_reality.json.tpl")),
        "inbound_ss2022.json.tpl" => Some(include_str!("../templates/inbound_ss2022.json.tpl")),
        "inbound_tuic.json.tpl" => Some(include_str!("../templates/inbound_tuic.json.tpl")),
        "sing-box.service.tpl" => Some(include_str!("../templates/sing-box.service.tpl")),
        "singbox_base.json.tpl" => Some(include_str!("../templates/singbox_base.json.tpl")),
        "sub-maker-sub.service.tpl" => {
            Some(include_str!("../templates/sub-maker-sub.service.tpl"))
        }
        "sub-maker-renew.service.tpl" => {
            Some(include_str!("../templates/sub-maker-renew.service.tpl"))
        }
        "sub-maker-renew.timer.tpl" => {
            Some(include_str!("../templates/sub-maker-renew.timer.tpl"))
        }
        "subconverter.service.tpl" => Some(include_str!("../templates/subconverter.service.tpl")),
        "subconverter_pref.ini.tpl" => {
            Some(include_str!("../templates/subconverter_pref.ini.tpl"))
        }
        _ => None,
    }
}

/// Minimal Go-like template substitution for `{{ .Key }}` / `{{.Key}}` placeholders.
pub fn render_simple(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{ .{key} }}}}"), value);
        out = out.replace(&format!("{{{{.{key}}}}}"), value);
        out = out.replace(&format!("{{{{ .{key}}}}}"), value);
        out = out.replace(&format!("{{{{.{key} }}}}"), value);
    }
    out
}

/// Like `render_simple`, but JSON-string-escapes substituted values so that
/// quotes/backslashes/control characters in credentials cannot corrupt the
/// generated document. Keys in `bare_keys` are substituted verbatim because
/// their template context is a JSON number (e.g. ports).
pub fn render_json(template: &str, vars: &[(&str, String)], bare_keys: &[&str]) -> String {
    let escaped: Vec<(&str, String)> = vars
        .iter()
        .map(|(key, value)| {
            if bare_keys.contains(key) {
                (*key, value.clone())
            } else {
                // serde_json yields a quoted string; templates add their own quotes.
                let quoted = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into());
                (*key, quoted[1..quoted.len() - 1].to_string())
            }
        })
        .collect();
    render_simple(template, &escaped)
}
