//! One-line input field shared by the setup wizard and the config editor.

use crossterm::event::{KeyCode, KeyModifiers};

use super::{edit_text, insert_text, Edit};

pub struct Field {
    pub label: &'static str,
    pub help: &'static str,
    pub value: String,
    pub password: bool,
    pub pos: usize,
}

impl Field {
    /// Applies a keystroke as a text edit. Returns false when the key is not
    /// a text-editing key (navigation keys are handled by the screens).
    pub fn edit(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        let edit = match code {
            KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => Edit::Clear,
            KeyCode::Char(c) if !mods.contains(KeyModifiers::CONTROL) => Edit::Insert(c),
            KeyCode::Backspace => Edit::Backspace,
            KeyCode::Delete => Edit::Delete,
            KeyCode::Left => Edit::Left,
            KeyCode::Right => Edit::Right,
            KeyCode::Home => Edit::Home,
            KeyCode::End => Edit::End,
            _ => return false,
        };
        edit_text(&mut self.value, &mut self.pos, edit);
        true
    }

    /// Inserts pasted text at the cursor (control characters stripped).
    pub fn paste(&mut self, text: &str) {
        insert_text(&mut self.value, &mut self.pos, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> Field {
        Field {
            label: "label",
            help: "help",
            value: String::new(),
            password: false,
            pos: 0,
        }
    }

    #[test]
    fn field_edit_accepts_only_text_keys() {
        let mut f = field();
        assert!(f.edit(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(f.edit(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(f.value, "ab");

        assert!(!f.edit(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!f.edit(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert_eq!(f.value, "ab", "non-edit and control keys must not type");

        f.edit(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(f.value, "");

        f.paste("hi\nthere");
        assert_eq!(f.value, "hithere", "pasted control characters are dropped");
    }
}
