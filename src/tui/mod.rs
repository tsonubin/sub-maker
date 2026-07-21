mod config;
mod terminal;
mod theme;
mod widgets;
mod wizard;

pub use config::run_config_wizard;
pub use wizard::run_wizard;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use rand::RngCore;
use uuid::Uuid;

pub(crate) fn random_token() -> String {
    let secret = random_secret();
    let take = secret.len().min(16);
    format!("sub-{}", &secret[..take])
}

pub(crate) fn random_secret() -> String {
    let mut b = [0u8; 18];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return Uuid::new_v4().to_string();
    }
    URL_SAFE_NO_PAD.encode(b)
}

pub(crate) fn random_ss2022_secret() -> String {
    let mut b = [0u8; 16];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return random_secret();
    }
    STANDARD.encode(b)
}

pub(crate) fn random_hex(size: usize) -> String {
    let mut b = vec![0u8; size];
    if rand::thread_rng().try_fill_bytes(&mut b).is_err() {
        return "01234567".to_string();
    }
    hex::encode(b)
}

pub(crate) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
