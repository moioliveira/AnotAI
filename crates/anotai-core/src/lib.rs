//! Núcleo do AnotAI: orquestra o pipeline em tempo real, independente da UI.
//!
//! A UI só recebe [`CoreEvent`]s e chama comandos; o ditado funciona com a
//! janela fechada.

use serde::{Deserialize, Serialize};

pub use anotai_hotkey::{Action as HotkeyAction, GestureConfig};

/// Modo de operação (`mode.current`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Dictation,
    Translation,
}

/// Estado visível do app (seção 3.2 do PRD); dirige o overlay e o ícone da bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppState {
    #[default]
    Idle,
    Listening,
    Transcribing,
    Inserting,
    Error,
}

/// Eventos emitidos para a UI (`state-changed`, `audio-level`,
/// `transcript-partial`, `transcript-final`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
pub enum CoreEvent {
    StateChanged {
        state: AppState,
        mode: Mode,
    },
    AudioLevel {
        rms: f32,
    },
    TranscriptPartial {
        text: String,
    },
    TranscriptFinal {
        text: String,
    },
    /// Histórico ou estatísticas mudaram (a UI recarrega).
    HistoryChanged,
}

/// Erros do núcleo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hotkey(#[from] anotai_hotkey::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Converte uma ação do gesto no próximo estado do app.
pub fn next_state(current: AppState, action: HotkeyAction) -> AppState {
    match action {
        HotkeyAction::StartPushToTalk | HotkeyAction::StartContinuous => AppState::Listening,
        HotkeyAction::StopPushToTalk | HotkeyAction::StopContinuous => {
            if current == AppState::Listening {
                AppState::Transcribing
            } else {
                current
            }
        }
        HotkeyAction::Cancel => AppState::Idle,
        // Armar/desarmar o microfone não muda o que o usuário vê.
        HotkeyAction::Arm | HotkeyAction::Disarm => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_after_listening_goes_to_transcribing() {
        assert_eq!(
            next_state(AppState::Listening, HotkeyAction::StopPushToTalk),
            AppState::Transcribing
        );
        assert_eq!(
            next_state(AppState::Listening, HotkeyAction::Cancel),
            AppState::Idle
        );
    }

    #[test]
    fn events_serialize_with_kebab_case_tags() -> std::result::Result<(), serde_json::Error> {
        let json = serde_json::to_string(&CoreEvent::StateChanged {
            state: AppState::Listening,
            mode: Mode::Translation,
        })?;
        assert_eq!(
            json,
            r#"{"type":"state-changed","payload":{"state":"listening","mode":"translation"}}"#
        );
        Ok(())
    }
}
