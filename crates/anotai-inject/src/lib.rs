//! Insere o texto no campo onde o cursor estava (Tech Spec 5.2).
//!
//! - Até [`TYPE_LIMIT`] caracteres: digita com `SendInput` + `KEYEVENTF_UNICODE`.
//! - Acima disso: cola pela área de transferência e devolve o conteúdo
//!   anterior 300 ms depois.
//! - Campos de senha (UI Automation `IsPassword`) bloqueiam a inserção.

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

/// Acima disto, colar é mais rápido e confiável que digitar.
pub const TYPE_LIMIT: usize = 200;

/// `insert.method` nas configurações.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertMethod {
    #[default]
    Auto,
    Type,
    Paste,
}

/// Janela que tinha o foco quando o atalho foi pressionado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub hwnd: isize,
}

/// Como o texto foi inserido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Inserted {
    Typed,
    Pasted,
}

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("o cursor está num campo de senha; o texto não foi inserido")]
    PasswordField,
    #[error("a janela de destino não pôde ser reativada; o texto foi copiado")]
    FocusLost,
    #[error("o Windows recusou a digitação simulada (app elevado?)")]
    SendInputFailed,
    #[error("falha na área de transferência: {0}")]
    Clipboard(String),
    #[error("inserção não suportada neste sistema")]
    Unsupported,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Escolhe digitar ou colar.
pub fn choose_method(text: &str, method: InsertMethod) -> Inserted {
    match method {
        InsertMethod::Type => Inserted::Typed,
        InsertMethod::Paste => Inserted::Pasted,
        InsertMethod::Auto if text.chars().count() <= TYPE_LIMIT => Inserted::Typed,
        InsertMethod::Auto => Inserted::Pasted,
    }
}

/// Guarda a janela em primeiro plano (chamar no key-down do atalho).
pub fn capture_target() -> Option<Target> {
    #[cfg(windows)]
    {
        windows::capture_target()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Insere `text` no destino. Bloqueia por até ~1,5 s (espera teclas soltas,
/// restauração da área de transferência); chamar fora da thread de UI.
pub fn insert(text: &str, target: Option<Target>, method: InsertMethod) -> Result<Inserted> {
    if text.is_empty() {
        return Ok(Inserted::Typed);
    }
    #[cfg(windows)]
    {
        windows::insert(text, target, choose_method(text, method))
    }
    #[cfg(not(windows))]
    {
        let _ = (target, method);
        Err(Error::Unsupported)
    }
}

/// Só copia o texto (fallback quando a inserção falha).
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    #[cfg(windows)]
    {
        windows::set_clipboard(text)
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_types_short_and_pastes_long_text() {
        assert_eq!(choose_method("oi", InsertMethod::Auto), Inserted::Typed);
        let long = "a".repeat(TYPE_LIMIT + 1);
        assert_eq!(choose_method(&long, InsertMethod::Auto), Inserted::Pasted);
        assert_eq!(choose_method(&long, InsertMethod::Type), Inserted::Typed);
        assert_eq!(choose_method("oi", InsertMethod::Paste), Inserted::Pasted);
    }

    #[test]
    fn limit_counts_characters_not_bytes() {
        let accented = "ç".repeat(TYPE_LIMIT); // 400 bytes, 200 caracteres
        assert_eq!(
            choose_method(&accented, InsertMethod::Auto),
            Inserted::Typed
        );
    }
}
