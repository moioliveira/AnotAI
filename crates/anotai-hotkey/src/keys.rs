//! Rastreamento de teclas para o hook de baixo nível, sem dependência de SO.
//!
//! Recebe cada tecla física (código virtual + pressionada/solta) e decide:
//! - qual [`Input`] enviar à máquina de gestos;
//! - se o key-up da Win ou do Alt deve ser "mascarado": soltar a Win sozinha
//!   abre o menu Iniciar e soltar o Alt sozinho ativa a barra de menus do app
//!   em foco (o texto colado depois iria para o menu, não para o campo).
//!
//! Privacidade: só Alt e Win são identificados. Qualquer outra tecla vira
//! [`Input::OtherKey`] sem código, e só enquanto Alt + Win estão pressionados;
//! fora disso nada é registrado.

use crate::gesture::Input;

pub const VK_MENU: u32 = 0x12;
pub const VK_LMENU: u32 = 0xA4;
pub const VK_RMENU: u32 = 0xA5;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;
/// Tecla não atribuída usada como máscara (mesma técnica do AutoHotkey).
pub const VK_MASK: u16 = 0xE8;

/// O que fazer com uma tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyOutcome {
    /// Evento para a máquina de gestos.
    pub input: Option<Input>,
    /// Bloquear este key-up (Win ou Alt) e reinjetar máscara + key-up.
    pub mask_up: bool,
}

#[derive(Debug, Default)]
pub struct KeyTracker {
    lalt: bool,
    ralt: bool,
    lwin: bool,
    rwin: bool,
    combo_active: bool,
    /// A Win atual participou de um Alt + Win: o key-up precisa de máscara.
    win_used: bool,
    /// Idem para o Alt.
    alt_used: bool,
}

impl KeyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn alt(&self) -> bool {
        self.lalt || self.ralt
    }

    fn win(&self) -> bool {
        self.lwin || self.rwin
    }

    /// Processa uma tecla física. `now` em ms monotônicos.
    pub fn on_key(&mut self, vk: u32, down: bool, now: u64) -> KeyOutcome {
        let is_win = matches!(vk, VK_LWIN | VK_RWIN);
        let is_alt = matches!(vk, VK_LMENU | VK_RMENU | VK_MENU);
        match vk {
            VK_LMENU | VK_MENU => self.lalt = down,
            VK_RMENU => self.ralt = down,
            VK_LWIN => self.lwin = down,
            VK_RWIN => self.rwin = down,
            _ => {
                return KeyOutcome {
                    input: (down && self.combo_active).then_some(Input::OtherKey(now)),
                    mask_up: false,
                };
            }
        }

        let mut out = KeyOutcome::default();
        let combo = self.alt() && self.win();
        if down && combo && !self.combo_active {
            // Auto-repeat de Alt/Win não reentra aqui: combo_active já é true.
            self.combo_active = true;
            self.win_used = true;
            self.alt_used = true;
            out.input = Some(Input::ComboDown(now));
        } else if !down && self.combo_active && !combo {
            self.combo_active = false;
            out.input = Some(Input::ComboUp(now));
        }

        if is_win && !down && self.win_used {
            out.mask_up = true;
            if !self.win() {
                self.win_used = false;
            }
        }
        if is_alt && !down && self.alt_used {
            out.mask_up = true;
            if !self.alt() {
                self.alt_used = false;
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
    fn alt_then_win_forms_combo_and_masks_both_ups() {
        let mut t = KeyTracker::new();
        assert_eq!(press(&mut t, VK_LMENU, 0).input, None);
        assert_eq!(press(&mut t, VK_LWIN, 10).input, Some(Input::ComboDown(10)));
        let up = release(&mut t, VK_LWIN, 500);
        assert_eq!(up.input, Some(Input::ComboUp(500)));
        assert!(up.mask_up);
        // O Alt solto depois também é mascarado (senão ativa o menu do app).
        let alt_up = release(&mut t, VK_LMENU, 510);
        assert_eq!(alt_up.input, None);
        assert!(alt_up.mask_up);
    }

    #[test]
    fn win_then_right_alt_also_works() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_RWIN, 0);
        assert_eq!(press(&mut t, VK_RMENU, 5).input, Some(Input::ComboDown(5)));
        let alt_up = release(&mut t, VK_RMENU, 300);
        assert_eq!(alt_up.input, Some(Input::ComboUp(300)));
        assert!(alt_up.mask_up);
        // Alt soltou primeiro: a Win ainda precisa da máscara ao soltar.
        let up = release(&mut t, VK_RWIN, 320);
        assert_eq!(up.input, None);
        assert!(up.mask_up);
    }

    #[test]
    fn ctrl_win_no_longer_triggers() {
        let mut t = KeyTracker::new();
        assert_eq!(press(&mut t, 0xA2, 0).input, None); // LCtrl
        assert_eq!(press(&mut t, VK_LWIN, 10).input, None);
        assert!(!release(&mut t, VK_LWIN, 100).mask_up);
    }

    #[test]
    fn auto_repeat_does_not_repeat_combo_down() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LMENU, 0);
        press(&mut t, VK_LWIN, 0);
        assert_eq!(press(&mut t, VK_LWIN, 30).input, None);
        assert_eq!(press(&mut t, VK_LMENU, 60).input, None);
    }

    #[test]
    fn other_key_only_reported_during_combo() {
        let mut t = KeyTracker::new();
        assert_eq!(press(&mut t, 0x41, 0).input, None); // 'A' sozinho
        press(&mut t, VK_LMENU, 10);
        assert_eq!(press(&mut t, 0x41, 20).input, None); // Alt + A
        press(&mut t, VK_LWIN, 30);
        assert_eq!(press(&mut t, 0x25, 40).input, Some(Input::OtherKey(40))); // ←
        assert_eq!(release(&mut t, 0x25, 50).input, None);
    }

    #[test]
    fn win_or_alt_alone_is_not_masked() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LWIN, 0);
        assert_eq!(release(&mut t, VK_LWIN, 100), KeyOutcome::default());
        press(&mut t, VK_LMENU, 200);
        assert_eq!(release(&mut t, VK_LMENU, 300), KeyOutcome::default());
    }

    #[test]
    fn both_win_keys_mask_until_last_released() {
        let mut t = KeyTracker::new();
        press(&mut t, VK_LMENU, 0);
        press(&mut t, VK_LWIN, 0);
        press(&mut t, VK_RWIN, 5);
        assert!(release(&mut t, VK_LWIN, 100).mask_up);
        assert!(release(&mut t, VK_RWIN, 110).mask_up);
        assert!(release(&mut t, VK_LMENU, 120).mask_up);
        // Próximas Win e Alt sozinhas não são mascaradas.
        press(&mut t, VK_LWIN, 500);
        assert!(!release(&mut t, VK_LWIN, 600).mask_up);
        press(&mut t, VK_LMENU, 700);
        assert!(!release(&mut t, VK_LMENU, 800).mask_up);
    }
}
