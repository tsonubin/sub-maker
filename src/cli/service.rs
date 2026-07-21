use crate::diagnostics;
use crate::setup::subconverter_installed;

const MANAGED_SERVICES: &[&str] = &["sing-box", "subconverter", "sub-maker-sub"];

pub fn service_command(action: &str) -> anyhow::Result<()> {
    let services: Vec<&str> = if matches!(action, "start" | "stop" | "restart") {
        installed_managed_services()
    } else {
        MANAGED_SERVICES.to_vec()
    };

    match action {
        "status" => {
            let mut args: Vec<&str> = vec![action];
            args.extend(services.iter().copied());
            args.push("--no-pager");
            let out = diagnostics::systemctl_output(&args)?;
            print!("{out}");
            Ok(())
        }
        "start" | "stop" | "restart" => {
            let mut args: Vec<&str> = vec![action];
            args.extend(services.iter().copied());
            diagnostics::run_systemctl(&args)
        }
        other => anyhow::bail!("unsupported service command: {other}"),
    }
}

fn installed_managed_services() -> Vec<&'static str> {
    MANAGED_SERVICES
        .iter()
        .copied()
        .filter(|service| !(*service == "subconverter" && !subconverter_installed()))
        .collect()
}
