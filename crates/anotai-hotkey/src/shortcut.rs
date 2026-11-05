//! Atalho alternativo configurável (ex.: "Ctrl+Alt+Space"), registrado com
//! `RegisterHotKey` — funciona sem hook de teclado, para teclados sem tecla
//! Win e ambientes corporativos que bloqueiam hooks (Tech Spec 5.3).

use std::fmt;
use std::str::FromStr;

/// Combinação de modificadores + uma tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Código virtual da tecla principal.
    pub vk: u32,
}

impl Shortcut {
    /// Ctrl + Alt + Espaço.
    pub const DEFAULT_ALTERNATE: Self = Self {
        ctrl: true,
        alt: true,
        shift: false,
        win: false,
        vk: 0x20,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    #[error("atalho vazio")]
    Empty,
    #[error("tecla desconhecida: {0}")]
    UnknownKey(String),
    #[error("o atalho precisa de exatamente uma tecla além dos modificadores")]
    KeyCount,
    #[error("o atalho precisa de pelo menos um modificador (Ctrl, Alt, Shift ou Win)")]
    NoModifier,
}

fn key_from_name(name: &str) -> Option<u32> {
    let upper = name.to_uppercase();
    match upper.as_str() {
        "SPACE" | "ESPAÇO" | "ESPACO" => return Some(0x20),
        "ENTER" => return Some(0x0D),
        "TAB" => return Some(0x09),
        _ => {}
    }
    let bytes = upper.as_bytes();
    if let [c] = bytes
        && (c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some(u32::from(*c));
    }
    let n: u32 = upper.strip_prefix('F')?.parse().ok()?;
    (1..=24).contains(&n).then_some(0x70 + n - 1)
}

fn key_name(vk: u32) -> String {
    match vk {
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        _ => char::from_u32(vk).map_or_else(|| format!("0x{vk:02X}"), String::from),
    }
}

impl FromStr for Shortcut {
    type Err = ShortcutError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Err(ShortcutError::Empty);
        }
        let mut sc = Shortcut {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            vk: 0,
        };
        let mut keys = 0;
        for part in s.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => sc.ctrl = true,
                "alt" => sc.alt = true,
                "shift" => sc.shift = true,
                "win" | "super" | "meta" => sc.win = true,
                "" => return Err(ShortcutError::KeyCount),
                _ => {
                    sc.vk = key_from_name(part)
                        .ok_or_else(|| ShortcutError::UnknownKey(part.to_owned()))?;
                    keys += 1;
                }
            }
        }
        if keys != 1 {
            return Err(ShortcutError::KeyCount);
        }
        if !(sc.ctrl || sc.alt || sc.shift || sc.win) {
            return Err(ShortcutError::NoModifier);
        }
        Ok(sc)
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
            (self.win, "Win+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        f.write_str(&key_name(self.vk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_alternate() {
        assert_eq!("Ctrl+Alt+Space".parse(), Ok(Shortcut::DEFAULT_ALTERNATE));
        assert_eq!(
            " ctrl + alt + espaço ".parse(),
            Ok(Shortcut::DEFAULT_ALTERNATE)
        );
    }

    #[test]
    fn parses_letters_digits_and_function_keys() {
        let sc: Shortcut = "Shift+Alt+d".parse().unwrap_or(Shortcut::DEFAULT_ALTERNATE);
        assert_eq!((sc.shift, sc.alt, sc.vk), (true, true, 0x44));
        assert_eq!("Ctrl+9".parse::<Shortcut>().map(|s| s.vk), Ok(0x39));
        assert_eq!("Ctrl+F12".parse::<Shortcut>().map(|s| s.vk), Ok(0x7B));
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        assert_eq!("".parse::<Shortcut>(), Err(ShortcutError::Empty));
        assert_eq!("Space".parse::<Shortcut>(), Err(ShortcutError::NoModifier));
        assert_eq!("Ctrl+Alt".parse::<Shortcut>(), Err(ShortcutError::KeyCount));
        assert_eq!("Ctrl+A+B".parse::<Shortcut>(), Err(ShortcutError::KeyCount));
        assert_eq!(
            "Ctrl+Banana".parse::<Shortcut>(),
            Err(ShortcutError::UnknownKey("Banana".into()))
        );
        assert!("Ctrl+F25".parse::<Shortcut>().is_err());
    }

    #[test]
    fn display_round_trips() {
        for s in ["Ctrl+Alt+Space", "Shift+Win+F5", "Ctrl+K"] {
            let sc: Result<Shortcut, _> = s.parse();
            assert_eq!(sc.map(|v| v.to_string()), Ok(s.to_owned()));
        }
    }
}
