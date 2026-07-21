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
