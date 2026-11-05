//! Detecção do atalho de ditado.
//!
//! - [`gesture`]: máquina de estados pura (segurar / toque duplo);
//! - [`keys`]: rastreia Ctrl/Win e decide quando mascarar a Win (puro);
//! - [`shortcut`]: atalho alternativo configurável (ex.: Ctrl + Alt + Espaço);
//! - `windows`: hook `WH_KEYBOARD_LL` e `RegisterHotKey`, cada um numa thread;
//! - [`HotkeyListener`]: junta tudo e chama um callback com cada [`Action`].

pub mod gesture;
pub mod keys;
pub mod shortcut;
#[cfg(windows)]
mod windows;

use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use gesture::{Action, GestureConfig, GestureMachine, Input};
pub use shortcut::{Shortcut, ShortcutError};

/// Erros deste módulo.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("falha ao instalar o hook de teclado: {0}")]
    HookInstall(String),
    #[error("o atalho {0} já está em uso por outro programa: {1}")]
    ShortcutTaken(String, String),
    #[error("nenhum atalho pôde ser ativado (hook: {hook}; alternativo: {alternate})")]
    NoTrigger { hook: String, alternate: String },
    #[error("atalho de teclado não suportado neste sistema")]
    Unsupported,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Intervalo dos `Tick`s enquanto um gesto está em andamento.
const TICK: Duration = Duration::from_millis(20);

/// Milissegundos monotônicos desde o primeiro uso (relógio único do hook e dos ticks).
pub(crate) fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Opções do listener.
#[derive(Debug, Clone, Copy)]
pub struct ListenerOptions {
    pub gesture: GestureConfig,
    /// Atalho alternativo via `RegisterHotKey` (`None` desliga).
    pub alternate: Option<Shortcut>,
    /// Aceitar teclas injetadas por outros programas (SendInput). Só para
    /// testes ponta a ponta; em produção fica `false` para não reagir a
    /// automações nem a outros apps de atalho.
    pub accept_injected: bool,
}

impl Default for ListenerOptions {
    fn default() -> Self {
        Self {
            gesture: GestureConfig::default(),
            alternate: Some(Shortcut::DEFAULT_ALTERNATE),
            accept_injected: false,
        }
    }
}

/// O que ficou ativo. Pelo menos um dos dois funciona quando `start` dá certo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListenerStatus {
    /// Erro do hook Ctrl + Win, se não instalou.
    pub hook_error: Option<String>,
    /// Erro do atalho alternativo, se configurado e não registrou.
    pub alternate_error: Option<String>,
}

/// Escuta os atalhos globais e entrega cada [`Action`] ao callback, numa
/// thread própria (nunca na thread do hook).
pub struct HotkeyListener {
    #[cfg(windows)]
    hook: Option<windows::MsgThread>,
    #[cfg(windows)]
    alternate: Option<windows::MsgThread>,
    worker: Option<JoinHandle<()>>,
    status: ListenerStatus,
}

impl HotkeyListener {
    pub fn start<F>(options: ListenerOptions, on_action: F) -> Result<Self>
    where
        F: Fn(Action) + Send + 'static,
    {
        #[cfg(windows)]
        {
            let (tx, rx) = mpsc::channel::<Input>();
            let mut status = ListenerStatus::default();

            let hook = windows::spawn_hook(tx.clone(), options.accept_injected)
                .map_err(|e| status.hook_error = Some(e.to_string()))
                .ok();
            let alternate = options.alternate.and_then(|sc| {
                windows::spawn_shortcut(sc, tx.clone())
                    .map_err(|e| status.alternate_error = Some(e.to_string()))
                    .ok()
            });
            // Só as threads de teclado ficam com o Sender: quando elas param,
            // o loop de gestos termina sozinho.
            drop(tx);

            if hook.is_none() && alternate.is_none() {
                return Err(Error::NoTrigger {
                    hook: status.hook_error.unwrap_or_default(),
                    alternate: status.alternate_error.unwrap_or_else(|| "desligado".into()),
                });
            }

            let worker = std::thread::Builder::new()
                .name("anotai-hotkey-gesture".into())
                .spawn(move || run_gesture_loop(rx, options.gesture, on_action))
                .map_err(|e| Error::HookInstall(e.to_string()))?;
            Ok(Self {
                hook,
                alternate,
                worker: Some(worker),
                status,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (options, on_action);
            Err(Error::Unsupported)
        }
    }

    /// Quais atalhos ficaram ativos.
    pub fn status(&self) -> &ListenerStatus {
        &self.status
    }

    /// Remove o hook e o atalho e espera as threads terminarem.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        #[cfg(windows)]
        {
            if let Some(mut t) = self.hook.take() {
                t.stop();
            }
            if let Some(mut t) = self.alternate.take() {
                t.stop();
            }
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::warn!("a thread de gestos terminou com pânico");
        }
    }
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_gesture_loop<F>(rx: mpsc::Receiver<Input>, config: GestureConfig, on_action: F)
where
    F: Fn(Action),
{
    let mut machine = GestureMachine::new(config);
    loop {
        let input = if machine.is_idle() {
            match rx.recv() {
                Ok(input) => input,
                Err(_) => break,
            }
        } else {
            match rx.recv_timeout(TICK) {
                Ok(input) => input,
                Err(RecvTimeoutError::Timeout) => Input::Tick(now_ms()),
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };
        for action in machine.handle(input) {
            tracing::debug!(?action, "gesto");
            on_action(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gesture_loop_emits_hold_via_ticks_and_ends_on_disconnect() {
        let (tx, rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            run_gesture_loop(rx, GestureConfig::default(), move |a| {
                let _ = out_tx.send(a);
            });
        });

        let t0 = now_ms();
        tx.send(Input::ComboDown(t0)).ok();
        assert_eq!(
            out_rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(Action::Arm)
        );
        // Sem novos eventos: só os ticks podem disparar o push-to-talk.
        assert_eq!(
            out_rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(Action::StartPushToTalk)
        );
        tx.send(Input::ComboUp(now_ms())).ok();
        assert_eq!(
            out_rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(Action::StopPushToTalk)
        );
        drop(tx);
        assert!(worker.join().is_ok());
    }
}
