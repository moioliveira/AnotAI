//! Camada Win32: hook `WH_KEYBOARD_LL` (Ctrl + Win) e `RegisterHotKey`
//! (atalho alternativo). Cada um roda numa thread com loop de mensagens.
//!
//! O callback do hook roda na thread que o instalou e precisa responder em
//! < 1 ms (o Windows remove hooks lentos): ele só atualiza o [`KeyTracker`],
//! envia o [`Input`] por canal e, quando preciso, mascara o key-up da Win.

use std::cell::RefCell;
use std::sync::mpsc::{Sender, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS,
    KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, RegisterHotKey, SendInput, UnregisterHotKey, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
    PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_HOTKEY, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};
use windows::core::PCWSTR;

use crate::gesture::Input;
use crate::keys::{KeyTracker, VK_MASK};
use crate::shortcut::Shortcut;
use crate::{Error, Result, now_ms};

/// Marca as teclas que o próprio AnotAI injeta, para o hook ignorá-las.
pub const ANOTAI_EXTRA_INFO: usize = 0x414E_4F54; // "ANOT"

/// Intervalo de verificação da tecla do atalho alternativo enquanto pressionada.
const RELEASE_POLL: Duration = Duration::from_millis(10);

// ---------------------------------------------------------------------------
// Thread com loop de mensagens
// ---------------------------------------------------------------------------

/// O que roda dentro de uma [`MsgThread`]. `Drop` faz a limpeza (unhook etc.).
trait MsgLoop {
    fn on_message(&mut self, _msg: &MSG) {}
}

pub(crate) struct MsgThread {
    thread_id: u32,
    handle: Option<JoinHandle<()>>,
}

impl MsgThread {
    /// Cria a thread, roda `setup` nela e espera o resultado.
    fn spawn<S, L>(name: &str, setup: S) -> Result<Self>
    where
        S: FnOnce() -> Result<L> + Send + 'static,
        L: MsgLoop,
    {
        let (ready_tx, ready_rx) = sync_channel::<Result<u32>>(1);
        let handle = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run_msg_thread(setup, ready_tx))
            .map_err(|e| Error::HookInstall(e.to_string()))?;
        let thread_id = ready_rx
            .recv()
            .map_err(|_| Error::HookInstall("a thread terminou antes de ficar pronta".into()))??;
        Ok(Self {
            thread_id,
            handle: Some(handle),
        })
    }

    pub(crate) fn stop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        // SAFETY: só posta WM_QUIT na fila da thread.
        if let Err(e) = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        {
            tracing::warn!("falha ao encerrar a thread {}: {e}", self.thread_id);
            return;
        }
        if handle.join().is_err() {
            tracing::warn!("thread de teclado terminou com pânico");
        }
    }
}

impl Drop for MsgThread {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_msg_thread<S, L>(setup: S, ready: SyncSender<Result<u32>>)
where
    S: FnOnce() -> Result<L>,
    L: MsgLoop,
{
    let mut msg = MSG::default();
    // SAFETY: cria a fila de mensagens da thread antes de anunciar o id.
    let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
    let mut state = match setup() {
        Ok(state) => state,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    // SAFETY: id da thread atual.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    // SAFETY: loop padrão; GetMessageW devolve 0 em WM_QUIT e -1 em erro.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        state.on_message(&msg);
    }
    drop(state);
}

// ---------------------------------------------------------------------------
// Hook WH_KEYBOARD_LL (Ctrl + Win)
// ---------------------------------------------------------------------------

struct HookCtx {
    tracker: KeyTracker,
    tx: Sender<Input>,
    accept_injected: bool,
}

thread_local! {
    static CTX: RefCell<Option<HookCtx>> = const { RefCell::new(None) };
}

struct InstalledHook(HHOOK);

impl MsgLoop for InstalledHook {}

impl Drop for InstalledHook {
    fn drop(&mut self) {
        // SAFETY: o hook foi instalado por esta mesma thread.
        if let Err(e) = unsafe { UnhookWindowsHookEx(self.0) } {
            tracing::warn!("falha ao remover o hook: {e}");
        }
        CTX.with_borrow_mut(|ctx| *ctx = None);
        tracing::info!("hook de teclado removido");
    }
}

/// Instala o hook Ctrl + Win numa thread própria.
pub(crate) fn spawn_hook(tx: Sender<Input>, accept_injected: bool) -> Result<MsgThread> {
    MsgThread::spawn("anotai-hotkey-hook", move || {
        CTX.with_borrow_mut(|ctx| {
            *ctx = Some(HookCtx {
                tracker: KeyTracker::new(),
                tx,
                accept_injected,
            });
        });
        // SAFETY: registra `hook_proc` para esta thread/processo.
        let hook = unsafe {
            GetModuleHandleW(PCWSTR::null()).and_then(|module| {
                SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(hook_proc),
                    Some(HINSTANCE(module.0)),
                    0,
                )
            })
        };
        match hook {
            Ok(h) => {
                tracing::info!("hook de teclado instalado");
                Ok(InstalledHook(h))
            }
            Err(e) => {
                CTX.with_borrow_mut(|ctx| *ctx = None);
                Err(Error::HookInstall(e.to_string()))
            }
        }
    })
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: para WH_KEYBOARD_LL com HC_ACTION, lparam aponta para um KBDLLHOOKSTRUCT válido.
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if handle_key(kb, wparam.0 as u32) {
            return LRESULT(1);
        }
    }
    // SAFETY: repassa o evento para o próximo hook da cadeia.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Devolve `true` para bloquear o evento original.
fn handle_key(kb: &KBDLLHOOKSTRUCT, msg: u32) -> bool {
    if kb.dwExtraInfo == ANOTAI_EXTRA_INFO {
        return false;
    }
    let injected = (kb.flags.0 & LLKHF_INJECTED.0) != 0;
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

    let outcome = CTX.with_borrow_mut(|ctx| {
        let ctx = ctx.as_mut()?;
        if injected && !ctx.accept_injected {
            return None;
        }
        let outcome = ctx.tracker.on_key(kb.vkCode, down, now_ms());
        if let Some(input) = outcome.input {
            // Canal sem limite: nunca bloqueia o callback.
            let _ = ctx.tx.send(input);
        }
        Some(outcome)
    });

    // SendInput fora do borrow do contexto.
    match outcome {
        Some(o) if o.mask_win_up => mask_win_up(kb.vkCode as u16),
        _ => false,
    }
}

/// Injeta máscara (down/up) + key-up da Win, para o Windows não abrir o
/// Iniciar. Só bloqueia o key-up original se a injeção inteira deu certo;
/// senão a Win ficaria presa.
fn mask_win_up(win_vk: u16) -> bool {
    let key = |vk: u16, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: ANOTAI_EXTRA_INFO,
            },
        },
    };
    let inputs = [
        key(VK_MASK, KEYBD_EVENT_FLAGS(0)),
        key(VK_MASK, KEYEVENTF_KEYUP),
        key(win_vk, KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY),
    ];
    // SAFETY: `inputs` é um array válido de INPUT do tamanho informado.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

// ---------------------------------------------------------------------------
// RegisterHotKey (atalho alternativo, sem hook)
// ---------------------------------------------------------------------------

const HOTKEY_ID: i32 = 1;

struct RegisteredShortcut {
    vk: u32,
    tx: Sender<Input>,
}

impl MsgLoop for RegisteredShortcut {
    fn on_message(&mut self, msg: &MSG) {
        if msg.message != WM_HOTKEY || msg.wParam.0 != HOTKEY_ID as usize {
            return;
        }
        // WM_HOTKEY só avisa o key-down; o key-up é detectado consultando a
        // tecla. Bloquear esta thread enquanto ela está pressionada é ok:
        // ela não faz mais nada.
        let _ = self.tx.send(Input::ComboDown(now_ms()));
        // SAFETY: consulta o estado de uma tecla virtual.
        while unsafe { GetAsyncKeyState(self.vk as i32) } as u16 & 0x8000 != 0 {
            std::thread::sleep(RELEASE_POLL);
        }
        let _ = self.tx.send(Input::ComboUp(now_ms()));
    }
}

impl Drop for RegisteredShortcut {
    fn drop(&mut self) {
        // SAFETY: desregistra o atalho que esta thread registrou.
        let _ = unsafe { UnregisterHotKey(None, HOTKEY_ID) };
    }
}

/// Registra o atalho alternativo numa thread própria.
pub(crate) fn spawn_shortcut(shortcut: Shortcut, tx: Sender<Input>) -> Result<MsgThread> {
    MsgThread::spawn("anotai-hotkey-shortcut", move || {
        let mut mods = MOD_NOREPEAT;
        for (on, m) in [
            (shortcut.ctrl, MOD_CONTROL),
            (shortcut.alt, MOD_ALT),
            (shortcut.shift, MOD_SHIFT),
            (shortcut.win, MOD_WIN),
        ] {
            if on {
                mods |= m;
            }
        }
        // SAFETY: registra um atalho global associado a esta thread.
        unsafe { RegisterHotKey(None, HOTKEY_ID, HOT_KEY_MODIFIERS(mods.0), shortcut.vk) }
            .map_err(|e| Error::ShortcutTaken(shortcut.to_string(), e.to_string()))?;
        tracing::info!("atalho alternativo {shortcut} registrado");
        Ok(RegisteredShortcut {
            vk: shortcut.vk,
            tx,
        })
    })
}
