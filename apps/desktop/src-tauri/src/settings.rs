//! Configurações do usuário (Tech Spec 6.2), em `settings.json` via
//! tauri-plugin-store. Campos ausentes assumem o padrão: arquivos antigos
//! continuam válidos quando surgem campos novos.

use std::sync::RwLock;

use anotai_core::Mode;
use anotai_inject::InsertMethod;
use anotai_stt::CloudProvider;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

const FILE: &str = "settings.json";
const KEY: &str = "settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HotkeySettings {
    /// Atalho alternativo (`RegisterHotKey`); vazio desliga.
    pub alternate: String,
    pub double_tap_ms: u64,
    pub hold_threshold_ms: u64,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            alternate: "Ctrl+Alt+Space".into(),
            double_tap_ms: 350,
            hold_threshold_ms: 250,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineMode {
    /// Nuvem; se a rede ou o provedor falhar, o modelo local (se baixado).
    #[default]
    Auto,
    Cloud,
    /// Só local: o áudio nunca sai do computador.
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EngineSettings {
    pub mode: EngineMode,
    /// Provisório até o benchmark da Sprint 0 (ADR-003).
    pub cloud_provider: CloudProvider,
    /// Modelo do motor local (ver `models::MODELS`).
    pub local_model: String,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            mode: EngineMode::Auto,
            cloud_provider: CloudProvider::Groq,
            local_model: "base".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PostprocSettings {
    pub correction: bool,
    pub remove_fillers: bool,
    pub llm_model: String,
}

impl Default for PostprocSettings {
    fn default() -> Self {
        Self {
            correction: true,
            remove_fillers: true,
            llm_model: anotai_postproc::DEFAULT_MODEL.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistorySettings {
    /// 0 = não guardar.
    pub retention_days: u32,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self { retention_days: 30 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotkey: HotkeySettings,
    pub engine: EngineSettings,
    pub postproc: PostprocSettings,
    pub mode: Mode,
    pub insert_method: InsertMethod,
    pub history: HistorySettings,
    pub save_recordings: bool,
    pub theme: Theme,
    pub launch_at_startup: bool,
    pub telemetry: bool,
    /// Quando o usuário consentiu com o envio de áudio à nuvem (LGPD, epoch
    /// ms). Sem consentimento, só o motor local é usado.
    pub cloud_consent_at: Option<i64>,
    pub onboarding_done: bool,
}

/// Configurações em memória (fonte da verdade durante a execução).
#[derive(Default)]
pub struct SettingsState(pub RwLock<Settings>);

/// Lê do disco (ou padrões) e registra no estado do app.
pub fn init(app: &AppHandle) -> Settings {
    let settings = load(app).unwrap_or_else(|| {
        let defaults = Settings {
            launch_at_startup: true,
            ..Settings::default()
        };
        if let Err(e) = persist(app, &defaults) {
            log::warn!("não foi possível gravar as configurações iniciais: {e}");
        }
        defaults
    });
    app.manage(SettingsState(RwLock::new(settings.clone())));
    settings
}

fn load(app: &AppHandle) -> Option<Settings> {
    let store = app.store(FILE).ok()?;
    let value = store.get(KEY)?;
    match serde_json::from_value(value) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("settings.json inválido, usando padrões: {e}");
            None
        }
    }
}

fn persist(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let store = app.store(FILE).map_err(|e| e.to_string())?;
    store.set(
        KEY,
        serde_json::to_value(settings).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())
}

/// Cópia atual das configurações.
pub fn get(app: &AppHandle) -> Settings {
    app.state::<SettingsState>()
        .0
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}

/// Valida, grava e devolve as configurações anteriores.
pub fn save(app: &AppHandle, new: Settings) -> Result<Settings, String> {
    let alternate = new.hotkey.alternate.trim();
    if !alternate.is_empty() {
        alternate
            .parse::<anotai_hotkey::Shortcut>()
            .map_err(|e| format!("Atalho alternativo inválido: {e}"))?;
    }
    if !crate::models::MODELS
        .iter()
        .any(|m| m.id == new.engine.local_model)
    {
        return Err(format!(
            "Modelo local desconhecido: {}",
            new.engine.local_model
        ));
    }
    if !(100..=1000).contains(&new.hotkey.hold_threshold_ms)
        || !(150..=1000).contains(&new.hotkey.double_tap_ms)
    {
        return Err("Tempos do atalho fora do intervalo permitido".into());
    }
    persist(app, &new)?;
    let state = app.state::<SettingsState>();
    let mut guard = state
        .0
        .write()
        .map_err(|_| "estado corrompido".to_string())?;
    Ok(std::mem::replace(&mut *guard, new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() -> Result<(), serde_json::Error> {
        let s: Settings =
            serde_json::from_str(r#"{"mode":"translation","hotkey":{"alternate":""}}"#)?;
        assert_eq!(s.mode, Mode::Translation);
        assert_eq!(s.hotkey.alternate, "");
        assert_eq!(s.hotkey.hold_threshold_ms, 250);
        assert_eq!(s.history.retention_days, 30);
        assert_eq!(s.engine.cloud_provider, CloudProvider::Groq);
        Ok(())
    }

    #[test]
    fn serializes_camel_case() -> Result<(), serde_json::Error> {
        let v = serde_json::to_value(Settings::default())?;
        assert!(v["hotkey"]["holdThresholdMs"].is_number());
        assert_eq!(v["engine"]["cloudProvider"], "groq");
        assert_eq!(v["insertMethod"], "auto");
        Ok(())
    }
}
