//! Comandos chamados pela UI (`invoke`). Erros voltam como texto legível.

use std::collections::BTreeMap;

use anotai_core::Mode;
use anotai_store::{DailyStat, DictionaryTerm, Transcription};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::hotkey::{self, HotkeyStatus};
use crate::pipeline::{self, Db};
use crate::settings::{self, Settings};
use crate::{secrets, tray, ui};

type CmdResult<T> = Result<T, String>;

fn db(app: &AppHandle) -> CmdResult<State<'_, Db>> {
    app.try_state::<Db>()
        .ok_or_else(|| "O histórico não está disponível (banco não abriu).".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    hotkey: HotkeyStatus,
    data_dir: Option<String>,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        hotkey: hotkey::status(&app),
        data_dir: pipeline::data_dir().map(|p| p.display().to_string()),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::get(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, new: Settings) -> CmdResult<Settings> {
    let old = settings::save(&app, new.clone())?;
    if old.hotkey != new.hotkey {
        hotkey::start_listener(&app);
    }
    if old.launch_at_startup != new.launch_at_startup {
        apply_autostart(&app, new.launch_at_startup);
    }
    if old.mode != new.mode {
        on_mode_changed(&app, new.mode);
    }
    Ok(new)
}

pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(e) = result {
        log::warn!("falha ao configurar o início com o Windows: {e}");
    }
}

/// Alterna ditado/tradução (botões da tela e menu da bandeja).
#[tauri::command]
pub fn set_mode(app: AppHandle, mode: Mode) -> CmdResult<()> {
    let mut cfg = settings::get(&app);
    if cfg.mode == mode {
        return Ok(());
    }
    cfg.mode = mode;
    settings::save(&app, cfg)?;
    on_mode_changed(&app, mode);
    Ok(())
}

fn on_mode_changed(app: &AppHandle, mode: Mode) {
    let (state, _) = ui::current(app);
    ui::set_state(app, state, mode);
    tray::set_translation_checked(app, mode == Mode::Translation);
}

#[tauri::command]
pub fn api_key_status() -> BTreeMap<&'static str, bool> {
    secrets::status()
}

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> CmdResult<()> {
    secrets::set(&provider, &key)
}

#[tauri::command]
pub async fn history_list(
    app: AppHandle,
    query: Option<String>,
    limit: Option<i64>,
) -> CmdResult<Vec<Transcription>> {
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let db = db(&app)?;
    match query.as_deref().map(str::trim) {
        Some(q) if !q.is_empty() => db.0.search(q, limit).await,
        _ => db.0.recent(limit).await,
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_delete(app: AppHandle, id: String) -> CmdResult<()> {
    db(&app)?
        .0
        .delete_transcription(&id)
        .await
        .map_err(|e| e.to_string())
}

/// "Apagar tudo" (LGPD): histórico, dicionário, estatísticas e chaves.
#[tauri::command]
pub async fn delete_all_data(app: AppHandle) -> CmdResult<()> {
    db(&app)?.0.delete_all().await.map_err(|e| e.to_string())?;
    secrets::clear_all();
    if let Some(dir) = pipeline::data_dir().map(|d| d.join("recordings"))
        && dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&dir)
    {
        log::warn!("falha ao apagar gravações: {e}");
    }
    Ok(())
}

/// Exporta tudo em JSON para a pasta Documentos e devolve o caminho.
#[tauri::command]
pub async fn export_data(app: AppHandle) -> CmdResult<String> {
    let data = db(&app)?.0.export_json().await.map_err(|e| e.to_string())?;
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("pasta Documentos indisponível: {e}"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let path = dir.join(format!("AnotAI-export-{stamp}.json"));
    let json = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn dictionary_list(app: AppHandle) -> CmdResult<Vec<DictionaryTerm>> {
    db(&app)?.0.dictionary().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dictionary_add(
    app: AppHandle,
    term: String,
    spoken_as: Option<String>,
) -> CmdResult<DictionaryTerm> {
    if term.trim().is_empty() {
        return Err("Informe o termo.".into());
    }
    db(&app)?
        .0
        .add_term(&term, spoken_as.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dictionary_remove(app: AppHandle, id: String) -> CmdResult<()> {
    db(&app)?
        .0
        .remove_term(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stats(app: AppHandle, days: Option<u32>) -> CmdResult<Vec<DailyStat>> {
    db(&app)?
        .0
        .stats(days.unwrap_or(30))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn models_status(app: AppHandle) -> Vec<crate::models::ModelStatus> {
    crate::models::status(&app)
}

/// Baixa o modelo; o progresso chega pelo evento `model-progress`.
#[tauri::command]
pub async fn model_download(app: AppHandle, id: String) -> CmdResult<()> {
    crate::models::download(&app, &id).await
}

#[tauri::command]
pub fn model_delete(app: AppHandle, id: String) -> CmdResult<()> {
    crate::models::delete(&app, &id)
}

/// Liga/desliga o microfone para o teste do onboarding (o nível chega pelo
/// evento `audio-level`; nada é gravado).
#[tauri::command]
pub fn mic_test(app: AppHandle, on: bool) {
    hotkey::mic_test(&app, on);
}

/// Consentimento LGPD para processar voz e texto na nuvem.
#[tauri::command]
pub fn set_cloud_consent(app: AppHandle, granted: bool) -> CmdResult<Settings> {
    let mut cfg = settings::get(&app);
    cfg.cloud_consent_at = granted.then(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default()
    });
    settings::save(&app, cfg.clone())?;
    log::info!("consentimento de nuvem: {granted}");
    Ok(cfg)
}
