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
        /// Passcode for SSH link retrieval
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
        error!(error = %err, "command failed");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
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
