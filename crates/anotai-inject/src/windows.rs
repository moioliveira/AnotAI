//! Implementação Win32: SendInput, área de transferência e UI Automation.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU,
    VK_RETURN, VK_RWIN, VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use crate::{Error, Inserted, Result, Target};

/// Mesmo marcador do hook (`anotai-hotkey`): ele ignora o que injetamos.
const ANOTAI_EXTRA_INFO: usize = 0x414E_4F54;
const CLIPBOARD_RESTORE_DELAY: Duration = Duration::from_millis(300);
const MODIFIER_RELEASE_WAIT: Duration = Duration::from_secs(1);
const UIA_TIMEOUT: Duration = Duration::from_millis(300);

pub(crate) fn capture_target() -> Option<Target> {
    // SAFETY: consulta sem efeitos colaterais.
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then_some(Target {
        hwnd: hwnd.0 as isize,
    })
}

pub(crate) fn insert(text: &str, target: Option<Target>, how: Inserted) -> Result<Inserted> {
    if let Some(t) = target {
        ensure_foreground(t)?;
    }
    if focused_is_password() {
        return Err(Error::PasswordField);
    }
    wait_modifiers_released();
    match how {
        Inserted::Typed => type_text(text).map(|()| Inserted::Typed),
        Inserted::Pasted => paste_text(text).map(|()| Inserted::Pasted),
    }
}

/// Reativa a janela de origem se o foco mudou durante o ditado.
fn ensure_foreground(target: Target) -> Result<()> {
    let hwnd = HWND(target.hwnd as *mut _);
    // SAFETY: só compara/reativa handles de janela.
    unsafe {
        if GetForegroundWindow() == hwnd {
            return Ok(());
        }
        let _ = SetForegroundWindow(hwnd);
    }
    std::thread::sleep(Duration::from_millis(60));
    // SAFETY: idem.
    if unsafe { GetForegroundWindow() } == hwnd {
        Ok(())
    } else {
        Err(Error::FocusLost)
    }
}

/// UI Automation numa thread à parte, com limite de tempo: alguns apps
/// demoram a responder. Na dúvida, segue (não bloqueia o ditado).
fn focused_is_password() -> bool {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(query_is_password());
    });
    match rx.recv_timeout(UIA_TIMEOUT) {
        Ok(Some(is_password)) => is_password,
        Ok(None) => false,
        Err(_) => {
            tracing::warn!("UI Automation não respondeu a tempo");
            false
        }
    }
}

fn query_is_password() -> Option<bool> {
    // SAFETY: COM inicializado nesta thread e liberado ao fim.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()?;
        let result = (|| {
            let uia: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = uia.GetFocusedElement().ok()?;
            element.CurrentIsPassword().ok().map(|b| b.as_bool())
        })();
        CoUninitialize();
        result
    }
}

fn key_down(vk: u16) -> bool {
    // SAFETY: consulta o estado de uma tecla.
    (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000) != 0
}

/// Se Ctrl/Alt/Win/Shift ainda estiverem pressionados, o texto viraria
/// atalho. Espera soltar (até 1 s).
fn wait_modifiers_released() {
    let deadline = Instant::now() + MODIFIER_RELEASE_WAIT;
    let mods = [VK_CONTROL.0, VK_MENU.0, VK_SHIFT.0, VK_LWIN.0, VK_RWIN.0];
    while mods.iter().any(|&vk| key_down(vk)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn key(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: ANOTAI_EXTRA_INFO,
            },
        },
    }
}

/// Sequência de teclas para o texto. Quebra de linha vira Shift+Enter, que
/// pula linha sem enviar a mensagem em apps de chat.
pub(crate) fn text_to_inputs(text: &str) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(text.len() * 2);
    let normalized = text.replace("\r\n", "\n");
    for c in normalized.chars() {
        if c == '\n' {
            inputs.push(key(VK_SHIFT.0, 0, KEYBD_EVENT_FLAGS(0)));
            inputs.push(key(VK_RETURN.0, 0, KEYBD_EVENT_FLAGS(0)));
            inputs.push(key(VK_RETURN.0, 0, KEYEVENTF_KEYUP));
            inputs.push(key(VK_SHIFT.0, 0, KEYEVENTF_KEYUP));
            continue;
        }
        let mut buf = [0u16; 2];
        for &unit in c.encode_utf16(&mut buf).iter() {
            inputs.push(key(0, unit, KEYEVENTF_UNICODE));
            inputs.push(key(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
    }
    inputs
}

fn send(inputs: &[INPUT]) -> Result<()> {
    // SAFETY: slice válido de INPUT do tamanho informado.
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(Error::SendInputFailed)
    }
}

fn type_text(text: &str) -> Result<()> {
    send(&text_to_inputs(text))
}

fn paste_text(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| Error::Clipboard(e.to_string()))?;
    // Só texto é preservado; imagens/arquivos copiados antes se perdem.
    let previous = clipboard.get_text().ok();
    clipboard
        .set_text(text)
        .map_err(|e| Error::Clipboard(e.to_string()))?;

    let paste = [
        key(VK_CONTROL.0, 0, KEYBD_EVENT_FLAGS(0)),
        key(VK_V.0, 0, KEYBD_EVENT_FLAGS(0)),
        key(VK_V.0, 0, KEYEVENTF_KEYUP),
        key(VK_CONTROL.0, 0, KEYEVENTF_KEYUP),
    ];
    let result = send(&paste);

    std::thread::sleep(CLIPBOARD_RESTORE_DELAY);
    let restored = match previous {
        Some(prev) => clipboard.set_text(prev),
        None => clipboard.clear(),
    };
    if let Err(e) = restored {
        tracing::warn!("não foi possível restaurar a área de transferência: {e}");
    }
    result
}

pub(crate) fn set_clipboard(text: &str) -> Result<()> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| Error::Clipboard(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn describe(inputs: &[INPUT]) -> Vec<(u16, u16, bool)> {
        inputs
            .iter()
            .map(|i| {
                // SAFETY: todos os INPUT aqui são de teclado.
                let ki = unsafe { i.Anonymous.ki };
                (ki.wVk.0, ki.wScan, (ki.dwFlags.0 & KEYEVENTF_KEYUP.0) != 0)
            })
            .collect()
    }

    #[test]
    fn unicode_chars_become_down_up_pairs() {
        let d = describe(&text_to_inputs("çã"));
        assert_eq!(
            d,
            [
                (0, 0xE7, false),
                (0, 0xE7, true),
                (0, 0xE3, false),
                (0, 0xE3, true)
            ]
        );
    }

    #[test]
    fn newline_is_shift_enter() {
        let d = describe(&text_to_inputs("a\r\nb"));
        assert_eq!(d.len(), 2 + 4 + 2);
        assert_eq!(
            &d[2..6],
            [
                (VK_SHIFT.0, 0, false),
                (VK_RETURN.0, 0, false),
                (VK_RETURN.0, 0, true),
                (VK_SHIFT.0, 0, true)
            ]
        );
    }

    #[test]
    fn emoji_uses_surrogate_pair() {
        assert_eq!(text_to_inputs("👍").len(), 4);
    }

    #[test]
    fn all_inputs_carry_the_anotai_marker() {
        // SAFETY: todos os INPUT aqui são de teclado.
        assert!(
            text_to_inputs("ok\n")
                .iter()
                .all(|i| unsafe { i.Anonymous.ki.dwExtraInfo } == ANOTAI_EXTRA_INFO)
        );
    }
}
