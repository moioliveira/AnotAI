//! Instala os atalhos reais do Windows por um instante (sem UI).

#![cfg(windows)]

use anotai_hotkey::{HotkeyListener, ListenerOptions};

#[test]
fn installs_hook_and_alternate_shortcut() -> Result<(), anotai_hotkey::Error> {
    let listener = HotkeyListener::start(ListenerOptions::default(), |_| {})?;
    let status = listener.status().clone();
    eprintln!("status dos atalhos: {status:?}");
    assert_eq!(
        status.hook_error, None,
        "o hook WH_KEYBOARD_LL deve instalar"
    );
    listener.stop();
    Ok(())
}

#[test]
fn restarting_releases_the_shortcut() -> Result<(), anotai_hotkey::Error> {
    // Se `stop` não desregistrasse o atalho, a segunda instalação falharia.
    let first = HotkeyListener::start(ListenerOptions::default(), |_| {})?;
    let alternate_ok = first.status().alternate_error.is_none();
    first.stop();
    let second = HotkeyListener::start(ListenerOptions::default(), |_| {})?;
    if alternate_ok {
        assert_eq!(second.status().alternate_error, None);
    }
    second.stop();
    Ok(())
}
