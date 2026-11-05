//! Liga os atalhos globais (Ctrl + Win e o alternativo) ao microfone e ao
//! pipeline de ditado.

use std::sync::{Arc, Mutex};

use anotai_audio::Recorder;
use anotai_core::{AppState, CoreEvent, HotkeyAction, Mode};
use anotai_hotkey::{GestureConfig, HotkeyListener, ListenerOptions, ListenerStatus, Shortcut};
use anotai_inject::Target;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{continuous, pipeline, settings, ui};

/// Um ditado em andamento, capturado no key-down do atalho.
#[derive(Debug, Clone)]
pub struct Session {
    /// Janela que tinha o foco: o texto volta para ela.
    pub target: Option<Target>,
    pub target_app: Option<String>,
    /// "hold" (push-to-talk) ou "toggle" (modo contínuo).
    pub trigger: &'static str,
    pub mode: Mode,
}

#[derive(Default)]
pub struct HotkeyState {
    listener: Mutex<Option<HotkeyListener>>,
    status: Mutex<HotkeyStatus>,
    recorder: Mutex<Option<Arc<Recorder>>>,
    session: Mutex<Option<Session>>,
    /// Modo contínuo com VAD em andamento.
    continuous: Mutex<Option<continuous::Handle>>,
}

/// Situação dos atalhos, para a tela de configurações.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub ctrl_win_error: Option<String>,
    pub alternate: Option<String>,
    pub alternate_error: Option<String>,
}

pub fn init(app: &AppHandle) {
    app.manage(HotkeyState::default());
    let level_app = app.clone();
    match Recorder::spawn(Arc::new(move |rms| {
        ui::emit(&level_app, &CoreEvent::AudioLevel { rms });
    })) {
        Ok(rec) => {
            if let Ok(mut slot) = app.state::<HotkeyState>().recorder.lock() {
                *slot = Some(Arc::new(rec));
            }
        }
        Err(err) => log::error!("gravador indisponível: {err}"),
    }
    start_listener(app);
}

/// (Re)instala os atalhos com as configurações atuais.
pub fn start_listener(app: &AppHandle) {
    let state = app.state::<HotkeyState>();
    // O atalho alternativo precisa ser liberado antes de registrar de novo.
    let old = state.listener.lock().ok().and_then(|mut s| s.take());
    if let Some(old) = old {
        old.stop();
    }

    let cfg = settings::get(app);
    let alternate = match cfg.hotkey.alternate.trim() {
        "" => None,
        s => s.parse::<Shortcut>().ok(),
    };
    let options = ListenerOptions {
        gesture: GestureConfig {
            hold_threshold_ms: cfg.hotkey.hold_threshold_ms,
            double_tap_ms: cfg.hotkey.double_tap_ms,
        },
        alternate,
        // Teclas injetadas só em debug, para os testes ponta a ponta.
        accept_injected: cfg!(debug_assertions)
            && std::env::var_os("ANOTAI_HOTKEY_ACCEPT_INJECTED").is_some(),
    };
    let handle = app.clone();
    let mut status = HotkeyStatus {
        alternate: alternate.map(|s| s.to_string()),
        ..HotkeyStatus::default()
    };
    match HotkeyListener::start(options, move |action| on_action(&handle, action)) {
        Ok(listener) => {
            let ListenerStatus {
                hook_error,
                alternate_error,
            } = listener.status().clone();
            log_status(&hook_error, &alternate_error, alternate);
            status.ctrl_win_error = hook_error;
            status.alternate_error = alternate_error;
            if let Ok(mut slot) = state.listener.lock() {
                *slot = Some(listener);
            }
        }
        Err(err) => {
            log::error!("{err}");
            status.ctrl_win_error = Some(err.to_string());
            ui::notify(app, "Atalho indisponível", &err.to_string());
        }
    }
    if let Ok(mut s) = state.status.lock() {
        *s = status;
    }
}

fn log_status(hook: &Option<String>, alternate: &Option<String>, sc: Option<Shortcut>) {
    match hook {
        None => log::info!("atalho Ctrl + Win ativo"),
        Some(err) => log::warn!("Ctrl + Win indisponível: {err}"),
    }
    match (alternate, sc) {
        (None, Some(sc)) => log::info!("atalho alternativo {sc} ativo"),
        (Some(err), _) => log::warn!("{err}"),
        (None, None) => {}
    }
}

pub fn status(app: &AppHandle) -> HotkeyStatus {
    app.state::<HotkeyState>()
        .status
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}

/// Remove os atalhos e fecha o microfone (chamado ao sair).
pub fn shutdown(app: &AppHandle) {
    let state = app.state::<HotkeyState>();
    let listener = state.listener.lock().ok().and_then(|mut slot| slot.take());
    if let Some(listener) = listener {
        listener.stop();
    }
    if let Ok(mut slot) = state.recorder.lock() {
        slot.take();
    }
}

fn recorder(app: &AppHandle) -> Option<Arc<Recorder>> {
    app.state::<HotkeyState>()
        .recorder
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

fn set_session(app: &AppHandle, f: impl FnOnce(&mut Option<Session>)) {
    if let Ok(mut s) = app.state::<HotkeyState>().session.lock() {
        f(&mut s);
    }
}

fn on_action(app: &AppHandle, action: HotkeyAction) {
    match action {
        HotkeyAction::Arm => {
            if let Some(rec) = recorder(app) {
                rec.arm();
            }
            // O foco ainda está no app de destino: guardar já.
            let session = Session {
                target: anotai_inject::capture_target(),
                target_app: anotai_platform::foreground_app().and_then(|a| a.exe),
                trigger: "hold",
                mode: settings::get(app).mode,
            };
            set_session(app, |s| *s = Some(session));
        }
        HotkeyAction::Disarm | HotkeyAction::Cancel => {
            if let Some(rec) = recorder(app) {
                rec.discard();
            }
            set_session(app, |s| *s = None);
            if action == HotkeyAction::Cancel {
                let (_, mode) = ui::current(app);
                ui::set_state(app, AppState::Idle, mode);
            }
        }
        HotkeyAction::StartPushToTalk | HotkeyAction::StartContinuous => {
            let trigger = if action == HotkeyAction::StartContinuous {
                "toggle"
            } else {
                "hold"
            };
            let mut mode = settings::get(app).mode;
            set_session(app, |s| {
                if let Some(s) = s {
                    s.trigger = trigger;
                    mode = s.mode;
                }
            });
            warn_if_target_elevated(app);
            ui::set_state(app, AppState::Listening, mode);
            if action == HotkeyAction::StartContinuous {
                start_continuous(app);
            }
        }
        HotkeyAction::StopContinuous if stop_continuous(app) => {}
        HotkeyAction::StopPushToTalk | HotkeyAction::StopContinuous => {
            let session = app
                .state::<HotkeyState>()
                .session
                .lock()
                .ok()
                .and_then(|mut s| s.take());
            match (recorder(app), session) {
                // Finaliza aqui, na thread de gestos: assim um novo Arm logo
                // em seguida nunca se mistura com este áudio.
                (Some(rec), Some(session)) => pipeline::run(app.clone(), rec.finish(), session),
                _ => {
                    let (_, mode) = ui::current(app);
                    ui::set_state(app, AppState::Idle, mode);
                }
            }
        }
    }
}

/// Teste de microfone (onboarding): só mede o nível, descarta o áudio.
pub fn mic_test(app: &AppHandle, on: bool) {
    if let Some(rec) = recorder(app) {
        if on {
            rec.arm();
        } else {
            rec.discard();
        }
    }
}

/// Com VAD, o modo contínuo insere a cada pausa; sem VAD, segue o fluxo
/// normal (tudo sai no StopContinuous).
fn start_continuous(app: &AppHandle) {
    let state = app.state::<HotkeyState>();
    let session = state.session.lock().ok().and_then(|mut s| s.take());
    let (Some(rec), Some(session)) = (recorder(app), session) else {
        return;
    };
    match continuous::start(app, rec, session.clone()) {
        Some(handle) => {
            if let Ok(mut slot) = state.continuous.lock() {
                *slot = Some(handle);
            }
        }
        // Sem VAD: devolve a sessão para o StopContinuous usar.
        None => set_session(app, |s| *s = Some(session)),
    }
}

/// `true` se havia um modo contínuo com VAD (agora encerrado).
fn stop_continuous(app: &AppHandle) -> bool {
    let handle = app
        .state::<HotkeyState>()
        .continuous
        .lock()
        .ok()
        .and_then(|mut h| h.take());
    match handle {
        Some(h) => {
            h.stop();
            true
        }
        None => false,
    }
}

/// O Windows (UIPI) impede inserir texto num app que roda como administrador
/// quando o AnotAI não roda. Avisa logo no início do ditado.
fn warn_if_target_elevated(app: &AppHandle) {
    if anotai_platform::foreground_blocked_by_uipi() {
        let exe = anotai_platform::foreground_app()
            .and_then(|t| t.exe)
            .unwrap_or_else(|| "o app em primeiro plano".into());
        ui::notify(
            app,
            "Este app roda como administrador",
            &format!(
                "O AnotAI não consegue inserir texto em {exe}. Abra o AnotAI como administrador para ditar nele."
            ),
        );
    }
}
