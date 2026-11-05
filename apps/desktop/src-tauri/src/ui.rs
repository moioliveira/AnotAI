//! Estado visível do app: eventos para a UI, overlay, ícone da bandeja e
//! notificações.

use std::sync::Mutex;

use anotai_core::{AppState, CoreEvent, Mode};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_notification::NotificationExt;

use crate::{CORE_EVENT, tray};

/// Margem entre o overlay e a barra de tarefas, em pixels físicos.
const OVERLAY_MARGIN: i32 = 24;

#[derive(Default)]
pub struct UiState(pub Mutex<(AppState, Mode)>);

pub fn emit(app: &AppHandle, event: &CoreEvent) {
    if let Err(err) = app.emit(CORE_EVENT, event) {
        log::warn!("falha ao emitir evento: {err}");
    }
}

/// Muda o estado visível (UI, overlay, bandeja).
pub fn set_state(app: &AppHandle, state: AppState, mode: Mode) {
    if let Some(ui) = app.try_state::<UiState>()
        && let Ok(mut current) = ui.0.lock()
    {
        if *current == (state, mode) {
            return;
        }
        *current = (state, mode);
    }
    log::info!("estado → {state:?} ({mode:?})");
    emit(app, &CoreEvent::StateChanged { state, mode });
    set_overlay_visible(app, state != AppState::Idle);
    if let Err(err) = tray::set_recording(app, state == AppState::Listening) {
        log::warn!("falha ao trocar o ícone da bandeja: {err}");
    }
}

pub fn current(app: &AppHandle) -> (AppState, Mode) {
    app.try_state::<UiState>()
        .and_then(|ui| ui.0.lock().ok().map(|g| *g))
        .unwrap_or_default()
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    log::info!("notificação: {title} — {body}");
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        log::warn!("falha ao mostrar notificação: {err}");
    }
}

fn set_overlay_visible(app: &AppHandle, visible: bool) {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return;
    };
    if !visible {
        let _ = overlay.hide();
        return;
    }
    if overlay.is_visible().unwrap_or(false) {
        return;
    }
    // Centro inferior da área de trabalho do monitor principal.
    if let (Ok(Some(monitor)), Ok(size)) = (overlay.primary_monitor(), overlay.outer_size()) {
        let area = monitor.work_area();
        let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - OVERLAY_MARGIN;
        let _ = overlay.set_position(PhysicalPosition::new(x, y));
    }
    // A janela não é focável: aparecer não tira o foco do app de destino.
    let _ = overlay.show();
}
