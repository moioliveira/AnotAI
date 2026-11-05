//! Ícone e menu da bandeja (seção 3.4 do PRD).

use anotai_core::Mode;
use tauri::{
    AppHandle, Manager, Wry,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub const TRAY_ID: &str = "anotai-tray";

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.png");

/// Item "Modo tradução", para refletir mudanças feitas pela tela.
pub struct TrayItems {
    translation: CheckMenuItem<Wry>,
}

pub fn create(app: &AppHandle, mode: Mode) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir AnotAI", true, None::<&str>)?;
    let translation = CheckMenuItem::with_id(
        app,
        "translation",
        "Modo tradução (EN → PT)",
        true,
        mode == Mode::Translation,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &translation,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(TrayItems {
        translation: translation.clone(),
    });

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON_IDLE)?)
        .tooltip("AnotAI — segure Ctrl + Win para falar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main_window(app),
            "translation" => {
                let next = if crate::settings::get(app).mode == Mode::Translation {
                    Mode::Dictation
                } else {
                    Mode::Translation
                };
                if let Err(e) = crate::commands::set_mode(app.clone(), next) {
                    log::warn!("falha ao trocar o modo: {e}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn set_translation_checked(app: &AppHandle, checked: bool) {
    if let Some(items) = app.try_state::<TrayItems>()
        && let Err(e) = items.translation.set_checked(checked)
    {
        log::warn!("falha ao atualizar o menu da bandeja: {e}");
    }
}

/// Troca o ícone da bandeja conforme o estado (ocioso / gravando).
pub fn set_recording(app: &AppHandle, recording: bool) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let bytes = if recording { ICON_RECORDING } else { ICON_IDLE };
        tray.set_icon(Some(Image::from_bytes(bytes)?))?;
    }
    Ok(())
}
