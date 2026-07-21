use crate::cli::load_config;
use crate::diagnostics;

pub fn print_links() -> anyhow::Result<()> {
    let cfg = load_config()?;
    let links = diagnostics::build_links(&cfg);
    println!("Subscription:\n  {}", links.subscription);
    println!("Raw nodes:\n  {}", links.raw);
    if !cfg.domain.is_empty() && !cfg.server_addr.is_empty() {
        println!("IP fallback:\n  {}", links.ip_fallback);
    }
    Ok(())
}
