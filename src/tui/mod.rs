mod config;
mod terminal;
mod theme;
mod widgets;
mod wizard;

pub use config::run_config_wizard;
pub use terminal::UserCancelled;
pub use wizard::run_wizard;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use rand::RngCore;

pub(crate) fn random_token() -> String {
    let secret = random_secret();
    let take = secret.len().min(16);
    format!("sub-{}", &secret[..take])
}

pub(crate) fn random_secret() -> String {
    let mut b = [0u8; 18];
    rand::thread_rng().fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

pub(crate) fn random_ss2022_secret() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    STANDARD.encode(b)
}

pub(crate) fn random_hex(size: usize) -> String {
    let mut b = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub(crate) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
