use std::process::Command;
use std::time::Duration;

pub fn run_systemctl(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("systemctl").args(args).output()?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "systemctl {}: {}\n{}{}",
            args.join(" "),
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(())
}

pub fn systemctl_output(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("systemctl").args(args).output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        anyhow::bail!("systemctl {}: {}", args.join(" "), output.status);
    }
    Ok(text)
}

pub fn service_active(service: &str) -> anyhow::Result<()> {
    run_systemctl(&["is-active", "--quiet", service])
}

pub fn verify_subscription_endpoint(url: &str) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| anyhow::anyhow!("request subscription endpoint: {e}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("subscription endpoint returned {}", resp.status());
    }
    let body = resp
        .text()
        .map_err(|e| anyhow::anyhow!("read subscription response: {e}"))?;
    if !body.contains("proxy-groups:") {
        anyhow::bail!("subscription endpoint did not return Clash YAML");
    }
    Ok(())
}
