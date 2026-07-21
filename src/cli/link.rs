use std::io::{self, BufRead, Write};

use crate::cli::load_config;
use crate::diagnostics;

pub fn print_subscription_link_with_passcode(
    passcode: &str,
    read_stdin: bool,
) -> anyhow::Result<()> {
    let cfg = load_config()?;
    if cfg.link_passcode_salt.is_empty() || cfg.link_passcode_hash.is_empty() {
        anyhow::bail!("SSH link passcode is not configured; rerun `sudo sub-maker setup`");
    }
    let passcode = if passcode.is_empty() {
        read_passcode(read_stdin)?
    } else {
        passcode.to_string()
    };
    if !cfg.verify_link_passcode(&passcode) {
        anyhow::bail!("invalid passcode");
    }
    println!("{}", diagnostics::build_links(&cfg).subscription);
    Ok(())
}

fn read_passcode(read_stdin: bool) -> anyhow::Result<String> {
    if read_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        let value = line.trim().to_string();
        if value.is_empty() {
            anyhow::bail!("missing passcode on stdin");
        }
        return Ok(value);
    }
    // Interactive password prompt when TTY is available.
    eprint!("Passcode: ");
    let _ = io::stderr().flush();
    match rpassword::read_password() {
        Ok(value) => {
            let value = value.trim().to_string();
            if value.is_empty() {
                anyhow::bail!("passcode required; use --passcode-stdin for non-interactive SSH commands");
            }
            Ok(value)
        }
        Err(_) => {
            anyhow::bail!("passcode required; use --passcode-stdin for non-interactive SSH commands")
        }
    }
}
