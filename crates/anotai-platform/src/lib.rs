//! Abstração por sistema operacional: windows (agora), linux e android (depois).

use serde::Serialize;

#[cfg(windows)]
mod windows;

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ainda não implementado: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Nível de privilégio de um processo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Elevation {
    Normal,
    /// Roda como administrador (ou o sistema negou consultar o token, o que
    /// para um processo comum indica privilégio maior que o nosso).
    Elevated,
    Unknown,
}

/// App que está em primeiro plano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundApp {
    /// Nome do executável, ex.: "OUTLOOK.EXE" (campo `target_app` do histórico).
    pub exe: Option<String>,
    pub elevation: Elevation,
}

/// O app em primeiro plano, se houver janela ativa.
pub fn foreground_app() -> Option<ForegroundApp> {
    #[cfg(windows)]
    {
        windows::foreground_app()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Se o próprio AnotAI roda como administrador.
pub fn current_process_elevation() -> Elevation {
    #[cfg(windows)]
    {
        windows::current_process_elevation()
    }
    #[cfg(not(windows))]
    {
        Elevation::Unknown
    }
}

/// `true` quando o app em primeiro plano tem mais privilégio que o AnotAI.
/// Nesse caso o Windows (UIPI) bloqueia o hook de teclado e a inserção de
/// texto nele.
pub fn foreground_blocked_by_uipi() -> bool {
    current_process_elevation() != Elevation::Elevated
        && foreground_app().is_some_and(|app| app.elevation == Elevation::Elevated)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn current_process_elevation_is_known() {
        assert_ne!(current_process_elevation(), Elevation::Unknown);
    }
}
