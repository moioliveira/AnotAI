//! Shell Tauri do AnotAI: janelas, bandeja, configurações e pipeline de ditado.

mod commands;
mod continuous;
mod engine;
mod hotkey;
mod models;
mod pipeline;
mod secrets;
mod settings;
mod tray;
mod ui;

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::MacosLauncher;

/// Evento único pelo qual o núcleo fala com a UI (ver `src/lib/core.ts`).
pub const CORE_EVENT: &str = "core-event";

pub fn run() {
    tauri::Builder::default()
        // Segunda instância só traz a janela da primeira para frente.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::save_settings,
            commands::set_mode,
            commands::api_key_status,
            commands::set_api_key,
            commands::history_list,
            commands::history_delete,
            commands::delete_all_data,
            commands::export_data,
            commands::dictionary_list,
            commands::dictionary_add,
            commands::dictionary_remove,
            commands::stats,
            commands::models_status,
            commands::model_download,
            commands::model_delete,
            commands::mic_test,
            commands::set_cloud_consent,
        ])
        .setup(|app| {
            let handle = app.handle();
            let cfg = settings::init(handle);
            app.manage(ui::UiState::default());
            open_database(handle, cfg.history.retention_days);
            commands::apply_autostart(handle, cfg.launch_at_startup);
            app.manage(models::Downloads::default());
            engine::init(handle);
            tray::create(handle, cfg.mode)?;
            hotkey::init(handle);
            // Janelas por último: o Tauri cria as do tauri.conf.json antes do
            // setup, e a UI embutida (release) chama comandos na hora, antes
            // de o estado existir. Por isso elas têm `create: false` e nascem
            // aqui, com tudo registrado.
            for window in app.config().app.windows.clone() {
                tauri::WebviewWindowBuilder::from_config(handle, &window)?.build()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fechar a janela principal só esconde: o app continua na bandeja.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|err| {
            log::error!("falha ao iniciar o AnotAI: {err}");
            std::process::exit(1);
        })
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                hotkey::shutdown(app);
            }
        });
}

/// Abre `%APPDATA%\AnotAI\anotai.db` e aplica a retenção. Se falhar, o app
/// funciona sem histórico.
fn open_database(app: &AppHandle, retention_days: u32) {
    let Some(dir) = pipeline::data_dir() else {
        log::error!("APPDATA não definido: histórico desativado");
        return;
    };
    let result = tauri::async_runtime::block_on(async {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let store = anotai_store::Store::open(&dir.join("anotai.db"))
            .await
            .map_err(|e| e.to_string())?;
        if retention_days > 0 {
            match store.purge_older_than(retention_days).await {
                Ok(n) if n > 0 => log::info!("retenção: {n} transcrições antigas apagadas"),
                Ok(_) => {}
                Err(e) => log::warn!("falha na retenção: {e}"),
            }
        }
        Ok::<_, String>(store)
    });
    match result {
        Ok(store) => {
            app.manage(pipeline::Db(store));
        }
        Err(e) => log::error!("histórico indisponível: {e}"),
    }
}

pub(crate) fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
