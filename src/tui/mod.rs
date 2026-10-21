mod config;
mod field;
mod terminal;
mod theme;
mod widgets;
mod wizard;

pub use config::run_config_wizard;
pub use field::Field;
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

/// A single text-editing operation on a one-line input field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edit {
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Clear,
}

/// Applies `edit` to a one-line field. `pos` is a byte offset and always
/// stays on a char boundary, so multibyte text is edited safely. Shared by
/// the wizard and the config editor so both screens behave identically.
pub(crate) fn edit_text(value: &mut String, pos: &mut usize, edit: Edit) {
    while *pos > value.len() || !value.is_char_boundary(*pos) {
        *pos -= 1;
    }
    match edit {
        Edit::Insert(c) => {
            if !c.is_control() {
                value.insert(*pos, c);
                *pos += c.len_utf8();
            }
        }
        Edit::Backspace => {
            if *pos > 0 {
                let step = value[..*pos].chars().next_back().map_or(0, char::len_utf8);
                *pos -= step;
                value.remove(*pos);
            }
        }
        Edit::Delete => {
            if *pos < value.len() {
                value.remove(*pos);
            }
        }
        Edit::Left => {
            if *pos > 0 {
                *pos -= value[..*pos].chars().next_back().map_or(0, char::len_utf8);
            }
        }
        Edit::Right => {
            if *pos < value.len() {
                *pos += value[*pos..].chars().next().map_or(0, char::len_utf8);
            }
        }
        Edit::Home => *pos = 0,
        Edit::End => *pos = value.len(),
        Edit::Clear => {
            value.clear();
            *pos = 0;
        }
    }
}

/// Inserts pasted text (control characters stripped) at the cursor.
pub(crate) fn insert_text(value: &mut String, pos: &mut usize, text: &str) {
    for c in text.chars().filter(|c| !c.is_control()) {
        edit_text(value, pos, Edit::Insert(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_text_tracks_cursor_across_multibyte_boundaries() {
        let mut v = String::new();
        let mut p = 0;
        insert_text(&mut v, &mut p, "héllo");
        assert_eq!((v.as_str(), p), ("héllo", "héllo".len()));

        edit_text(&mut v, &mut p, Edit::Home);
        edit_text(&mut v, &mut p, Edit::Right); // just after 'h'
        assert_eq!(p, 1);
        edit_text(&mut v, &mut p, Edit::Delete); // removes 'é', not half of it
        assert_eq!(v, "hllo");

        edit_text(&mut v, &mut p, Edit::End);
        edit_text(&mut v, &mut p, Edit::Backspace);
        assert_eq!(v, "hll");
        insert_text(&mut v, &mut p, "é!");
        assert_eq!(v, "hllé!");
    }
}
