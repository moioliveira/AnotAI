//! Rastreamento de teclas para o hook de baixo nível, sem dependência de SO.
//!
//! Recebe cada tecla física (código virtual + pressionada/solta) e decide:
//! - qual [`Input`] enviar à máquina de gestos;
//! - se o key-up da Win deve ser "mascarado" para o Windows não abrir o menu
//!   Iniciar (Tech Spec 5.3).
//!
//! Privacidade: só Ctrl e Win são identificados. Qualquer outra tecla vira
//! [`Input::OtherKey`] sem código, e só enquanto Ctrl + Win estão pressionados;
//! fora disso nada é registrado.

use crate::gesture::Input;

pub const VK_CONTROL: u32 = 0x11;
pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_RCONTROL: u32 = 0xA3;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;
/// Tecla não atribuída usada como máscara (mesma técnica do AutoHotkey).
pub const VK_MASK: u16 = 0xE8;

/// O que fazer com uma tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyOutcome {
    /// Evento para a máquina de gestos.
    pub input: Option<Input>,
    /// Bloquear este key-up da Win e reinjetar máscara + key-up.
    pub mask_win_up: bool,
}

#[derive(Debug, Default)]
pub struct KeyTracker {
    lctrl: bool,
    rctrl: bool,
    lwin: bool,
    rwin: bool,
    combo_active: bool,
    /// A Win atual participou de um Ctrl + Win: o key-up precisa de máscara.
    win_used: bool,
}

impl KeyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn ctrl(&self) -> bool {
        self.lctrl || self.rctrl
    }

    fn win(&self) -> bool {
        self.lwin || self.rwin
    }

    /// Processa uma tecla física. `now` em ms monotônicos.
    pub fn on_key(&mut self, vk: u32, down: bool, now: u64) -> KeyOutcome {
        let is_win = matches!(vk, VK_LWIN | VK_RWIN);
        match vk {
            VK_LCONTROL | VK_CONTROL => self.lctrl = down,
            VK_RCONTROL => self.rctrl = down,
            VK_LWIN => self.lwin = down,
            VK_RWIN => self.rwin = down,
            _ => {
                return KeyOutcome {
                    input: (down && self.combo_active).then_some(Input::OtherKey(now)),
                    mask_win_up: false,
                };
            }
        }

        let mut out = KeyOutcome::default();
        let combo = self.ctrl() && self.win();
        if down && combo && !self.combo_active {
            // Auto-repeat de Ctrl/Win não reentra aqui: combo_active já é true.
            self.combo_active = true;
            self.win_used = true;
            out.input = Some(Input::ComboDown(now));
        } else if !down && self.combo_active && !combo {
            self.combo_active = false;
            out.input = Some(Input::ComboUp(now));
        }

        if is_win && !down && self.win_used {
            out.mask_win_up = true;
            if !self.win() {
                self.win_used = false;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(t: &mut KeyTracker, vk: u32, now: u64) -> KeyOutcome {
        t.on_key(vk, true, now)
    }
    fn release(t: &mut KeyTracker, vk: u32, now: u64) -> KeyOutcome {
        t.on_key(vk, false, now)
    }

    #[test]
    fn ctrl_then_win_forms_combo_and_masks_win_up() {
        let mut t = KeyTracker::new();
        assert_eq!(press(&mut t, VK_LCONTROL, 0).input, None);
        assert_eq!(press(&mut t, VK_LWIN, 10).input, Some(Input::ComboDown(10)));
        let up = release(&mut t, VK_LWIN, 500);
        assert_eq!(up.input, Some(Input::ComboUp(500)));
        assert!(up.mask_win_up);
        assert_eq!(release(&mut t, VK_LCONTROL, 510), KeyOutcome::default());
    }

    #[test]
    fn win_then_right_ctrl_also_works() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_RWIN, 0);
        assert_eq!(
            press(&mut t, VK_RCONTROL, 5).input,
            Some(Input::ComboDown(5))
        );
        assert_eq!(
            release(&mut t, VK_RCONTROL, 300).input,
            Some(Input::ComboUp(300))
        );
        // Ctrl soltou primeiro: a Win ainda precisa da máscara ao soltar.
        let up = release(&mut t, VK_RWIN, 320);
        assert_eq!(up.input, None);
        assert!(up.mask_win_up);
    }

    #[test]
    fn auto_repeat_does_not_repeat_combo_down() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LCONTROL, 0);
        press(&mut t, VK_LWIN, 0);
        assert_eq!(press(&mut t, VK_LWIN, 30).input, None);
        assert_eq!(press(&mut t, VK_LCONTROL, 60).input, None);
    }

    #[test]
    fn other_key_only_reported_during_combo() {
        let mut t = KeyTracker::new();
        assert_eq!(press(&mut t, 0x41, 0).input, None); // 'A' sozinho
        press(&mut t, VK_LCONTROL, 10);
        assert_eq!(press(&mut t, 0x41, 20).input, None); // Ctrl + A
        press(&mut t, VK_LWIN, 30);
        assert_eq!(press(&mut t, 0x25, 40).input, Some(Input::OtherKey(40))); // ←
        assert_eq!(release(&mut t, 0x25, 50).input, None);
    }

    #[test]
    fn win_alone_is_not_masked() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LWIN, 0);
        let up = release(&mut t, VK_LWIN, 100);
        assert_eq!(up, KeyOutcome::default());
    }

    #[test]
    fn both_win_keys_mask_until_last_released() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LCONTROL, 0);
        press(&mut t, VK_LWIN, 0);
        press(&mut t, VK_RWIN, 5);
        assert!(release(&mut t, VK_LWIN, 100).mask_win_up);
        assert!(release(&mut t, VK_RWIN, 110).mask_win_up);
        release(&mut t, VK_LCONTROL, 120);
        // Próxima Win sozinha não é mascarada.
        press(&mut t, VK_LWIN, 500);
        assert!(!release(&mut t, VK_LWIN, 600).mask_win_up);
    }
}
