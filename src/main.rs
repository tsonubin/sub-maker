mod assets;
mod cli;
mod config;
mod diagnostics;
mod generator;
mod server;
mod setup;
mod tui;

use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "sub-maker",
    version = VERSION,
    about = "TUI for top-5 sing-box proxy + Clash sub with subconverter rulesets",
    long_about = "sub-maker: TUI for top-5 sing-box proxy + Clash sub with subconverter rulesets\n\
See LICENSE and DISCLAIMER.md — very strict terms (forking/use requires written permission)."
)]
struct Cli {
    /// Run interactive TUI setup wizard
    #[arg(long)]
    setup: bool,

    /// Run the subscription server on :8964 (for systemd)
    #[arg(long)]
    serve: bool,

    /// Print current generated node URIs
    #[arg(long)]
    nodes: bool,

    /// Update sing-box and subconverter binaries
    #[arg(long)]
    update: bool,

    /// Refresh managed certificates and restart sing-box if they changed
    #[arg(long)]
    renew: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run interactive setup wizard
    Setup,
    /// Edit existing configuration
    Config,
    /// Alias for config
    Configure,
    /// Run the subscription server
    Serve,
    /// Print current node URIs
    Nodes,
    /// Update sing-box and subconverter binaries
    Update,
    /// Refresh managed certificates and restart sing-box if they changed
    Renew,
    /// Print subscription and raw node links
    Links,
    /// Print subscription URL after passcode verification
    Link {
        /// Passcode for SSH link retrieval (visible in process listings; prefer --passcode-stdin)
        #[arg(long, default_value = "")]
        passcode: String,
        /// Read passcode from stdin
        #[arg(long)]
        passcode_stdin: bool,
    },
    /// Run health checks
    Doctor,
    /// Show systemd service status
    Status,
    /// Restart managed services
    Restart,
    /// Start managed services
    Start,
    /// Stop managed services
    Stop,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let result = run(cli);
    if let Err(err) = result {
        if err.downcast_ref::<tui::UserCancelled>().is_some() {
            eprintln!("aborted");
            std::process::exit(130);
        }
        error!(error = %err, "command failed");
        std::process::exit(1);
    }
}

/// Rejects invocations that request more than one action. The flag and
/// subcommand vocabularies are both kept for compatibility, but a flag
/// silently winning over a subcommand (or another flag) hides user intent.
fn check_single_action(cli: &Cli) -> anyhow::Result<()> {
    let mut req: Vec<(&'static str, &'static str)> = Vec::new(); // (action id, label)
    if cli.setup {
        req.push(("setup", "--setup"));
    }
    if cli.serve {
        req.push(("serve", "--serve"));
    }
    if cli.nodes {
        req.push(("nodes", "--nodes"));
    }
    if cli.update {
        req.push(("update", "--update"));
    }
    if cli.renew {
        req.push(("renew", "--renew"));
    }
    if let Some(cmd) = &cli.command {
        let entry = match cmd {
            Commands::Setup => ("setup", "setup"),
            Commands::Config | Commands::Configure => ("config", "config"),
            Commands::Serve => ("serve", "serve"),
            Commands::Nodes => ("nodes", "nodes"),
            Commands::Update => ("update", "update"),
            Commands::Renew => ("renew", "renew"),
            Commands::Links => ("links", "links"),
            Commands::Link { .. } => ("link", "link"),
            Commands::Doctor => ("doctor", "doctor"),
            Commands::Status => ("service", "status"),
            Commands::Restart => ("service", "restart"),
            Commands::Start => ("service", "start"),
            Commands::Stop => ("service", "stop"),
        };
        req.push(entry);
    }
    // The same action named twice (e.g. `--setup setup`) is not a conflict.
    let mut distinct: Vec<&(&'static str, &'static str)> = Vec::new();
    for entry in &req {
        if !distinct.iter().any(|(id, _)| *id == entry.0) {
            distinct.push(entry);
        }
    }
    if distinct.len() > 1 {
        let labels: Vec<&str> = distinct.iter().map(|(_, label)| *label).collect();
        anyhow::bail!(
            "cannot combine {}; request one action and see sub-maker --help",
            labels.join(" with ")
        );
    }
    Ok(())
}

fn run(cli: Cli) -> anyhow::Result<()> {
    check_single_action(&cli)?;
    // Flag forms (compatible with Go CLI).
    if cli.setup {
        return run_setup();
    }
    if cli.serve {
        return server::run();
    }
    if cli.nodes {
        return print_nodes();
    }
    if cli.update {
        return run_update();
    }
    if cli.renew {
        return run_renew();
    }

    match cli.command {
        Some(Commands::Setup) => run_setup(),
        Some(Commands::Config) | Some(Commands::Configure) => run_config(),
        Some(Commands::Serve) => server::run(),
        Some(Commands::Nodes) => print_nodes(),
        Some(Commands::Update) => run_update(),
        Some(Commands::Renew) => run_renew(),
        Some(Commands::Links) => cli::print_links(),
        Some(Commands::Link {
            passcode,
            passcode_stdin,
        }) => cli::print_subscription_link_with_passcode(&passcode, passcode_stdin),
        Some(Commands::Doctor) => cli::doctor(),
        Some(Commands::Status) => cli::service_command("status"),
        Some(Commands::Restart) => cli::service_command("restart"),
        Some(Commands::Start) => cli::service_command("start"),
        Some(Commands::Stop) => cli::service_command("stop"),
        None => {
            // Match Go: bare invocation shows help.
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

fn run_setup() -> anyhow::Result<()> {
    let mut cfg = if config::demo_mode() {
        let mut cfg = config::SetupConfig::default_config();
        cfg.server_addr = "demo.server.example".into();
        cfg.domain = "example.com".into();
        cfg.sub_token = "demo-token-123".into();
        info!("DEMO mode: using defaults (no interactive TUI)");
        cfg
    } else {
        tui::run_wizard()?
    };
    setup::apply(&mut cfg)
}

fn run_config() -> anyhow::Result<()> {
    let cfg = cli::load_config().map_err(|e| {
        anyhow::anyhow!(
            "load config failed; run setup first ({}): {e}",
            cli::config_path().display()
        )
    })?;
    let mut cfg = tui::run_config_wizard(cfg)?;
    setup::apply(&mut cfg)
}

fn print_nodes() -> anyhow::Result<()> {
    let path = cli::nodes_path();
    let data = std::fs::read_to_string(&path).map_err(|e| {
        anyhow::anyhow!("read nodes failed; run setup first ({}): {e}", path.display())
    })?;
    print!("{data}");
    Ok(())
}

fn run_update() -> anyhow::Result<()> {
    info!("updating sing-box...");
    setup::download_sing_box("")?;
    info!("updating subconverter...");
    setup::download_subconverter("")?;
    println!("Binaries updated. Restart services with: sudo sub-maker restart");
    Ok(())
}

fn run_renew() -> anyhow::Result<()> {
    let cfg = cli::load_config().map_err(|e| {
        anyhow::anyhow!(
            "load config failed; run setup first ({}): {e}",
            cli::config_path().display()
        )
    })?;
    let changed = setup::refresh_managed_certs(&cfg)?;
    if changed {
        println!("managed certificate refreshed");
        if !config::demo_mode() {
            diagnostics::run_systemctl(&["restart", "sing-box"])?;
            println!("sing-box restarted");
        }
    } else {
        println!("managed certificate unchanged; nothing to restart");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Cli {
        Cli {
            setup: false,
            serve: false,
            nodes: false,
            update: false,
            renew: false,
            command: None,
        }
    }

    #[test]
    fn one_action_per_invocation() {
        let mut c = cli();
        c.nodes = true;
        c.command = Some(Commands::Setup);
        let err = check_single_action(&c).unwrap_err().to_string();
        assert!(err.contains("--nodes") && err.contains("setup"), "{err}");

        let mut c = cli();
        c.setup = true;
        c.command = Some(Commands::Setup);
        check_single_action(&c).expect("the same action twice is not a conflict");

        let mut c = cli();
        c.setup = true;
        c.serve = true;
        assert!(check_single_action(&c).is_err());

        check_single_action(&cli()).expect("no action is allowed (prints help)");
    }
}
