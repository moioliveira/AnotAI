//! Máquina de estados do gesto Ctrl + Win (seção 5.3 do Tech Spec).
//!
//! - Segurar por mais de `hold_threshold_ms` → push-to-talk até soltar.
//! - Dois toques curtos, com a segunda pressão até `double_tap_ms` depois de
//!   soltar a primeira → liga o modo contínuo; um novo toque desliga.
//! - Outra tecla junto com Ctrl + Win → cancela e deixa o atalho do Windows
//!   passar (ex.: Ctrl + Win + ← para trocar de área de trabalho).
//!
//! O tempo entra como milissegundos monotônicos (`u64`) para a máquina ser
//! determinística nos testes. Quem chama deve enviar [`Input::Tick`]
//! periodicamente (ex.: a cada 20 ms) enquanto a máquina não estiver em
//! repouso, para que o limiar de "segurar" dispare sem esperar outro evento.

use serde::{Deserialize, Serialize};

/// Limiares configuráveis (`hotkey.holdThresholdMs`, `hotkey.doubleTapMs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GestureConfig {
    pub hold_threshold_ms: u64,
    pub double_tap_ms: u64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            hold_threshold_ms: 250,
            double_tap_ms: 350,
        }
    }
}

/// Eventos vindos do hook de teclado, com o instante em ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// Ctrl e Win ficaram pressionados juntos.
    ComboDown(u64),
    /// Ctrl ou Win foi solto.
    ComboUp(u64),
    /// Outra tecla foi pressionada enquanto Ctrl + Win estavam pressionados.
    OtherKey(u64),
    /// Passagem do tempo, sem tecla.
    Tick(u64),
}

impl Input {
    fn at(self) -> u64 {
        match self {
            Self::ComboDown(t) | Self::ComboUp(t) | Self::OtherKey(t) | Self::Tick(t) => t,
        }
    }
}

/// O que o núcleo deve fazer em resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// Combo pressionado: abrir o microfone já, antes de saber se é toque ou
    /// segurar, para não perder a primeira sílaba.
    Arm,
    /// Era só um toque ou um atalho do Windows: descartar o áudio armado.
    Disarm,
    StartPushToTalk,
    StopPushToTalk,
    StartContinuous,
    StopContinuous,
    /// Interrompe a escuta sem transcrever (outra tecla entrou no atalho).
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    /// Combo pressionado, ainda sem saber se é toque ou segurar.
    Pressed {
        down_at: u64,
        second_tap: bool,
    },
    /// Primeiro toque solto; aguardando o segundo.
    AwaitingSecondTap {
        released_at: u64,
    },
    /// Push-to-talk ativo.
    Holding,
    /// Modo contínuo ativo.
    Continuous,
    /// Combo pressionado durante o modo contínuo.
    ContinuousPressed,
}

/// Máquina de estados do gesto. Não faz I/O.
#[derive(Debug, Clone)]
pub struct GestureMachine {
    config: GestureConfig,
    state: State,
}

impl GestureMachine {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            state: State::Idle,
        }
    }

    /// `true` quando a máquina não precisa de [`Input::Tick`].
    pub fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle | State::Holding | State::Continuous)
    }

    /// Processa um evento e devolve as ações resultantes, em ordem (zero a
    /// duas: soltar depois do limiar sem `Tick` intermediário gera início e
    /// fim do push-to-talk de uma vez).
    pub fn handle(&mut self, input: Input) -> impl Iterator<Item = Action> + use<> {
        let now = input.at();
        // Transições que dependem só do tempo vêm antes do evento em si.
        let timed = self.advance(now);
        let action = self.apply(input, now);
        timed.into_iter().chain(action)
    }

    fn advance(&mut self, now: u64) -> Option<Action> {
        match self.state {
            State::Pressed { down_at, .. }
                if now.saturating_sub(down_at) >= self.config.hold_threshold_ms =>
            {
                self.state = State::Holding;
                Some(Action::StartPushToTalk)
            }
            State::AwaitingSecondTap { released_at }
                if now.saturating_sub(released_at) > self.config.double_tap_ms =>
            {
                // Toque simples isolado: só desarma o microfone.
                self.state = State::Idle;
                Some(Action::Disarm)
            }
            _ => None,
        }
    }

    fn apply(&mut self, input: Input, now: u64) -> Option<Action> {
        use State as S;
        match (self.state, input) {
            (_, Input::Tick(_)) => None,

            (S::Idle, Input::ComboDown(_)) => {
                self.state = S::Pressed {
                    down_at: now,
                    second_tap: false,
                };
                Some(Action::Arm)
            }
            (S::AwaitingSecondTap { .. }, Input::ComboDown(_)) => {
                self.state = S::Pressed {
                    down_at: now,
                    second_tap: true,
                };
                None
            }
            (S::Pressed { second_tap, .. }, Input::ComboUp(_)) => {
                if second_tap {
                    self.state = S::Continuous;
                    Some(Action::StartContinuous)
                } else {
                    self.state = S::AwaitingSecondTap { released_at: now };
                    None
                }
            }
            (S::Pressed { .. } | S::AwaitingSecondTap { .. }, Input::OtherKey(_)) => {
                self.state = S::Idle;
                Some(Action::Disarm)
            }

            (S::Holding, Input::ComboUp(_)) => {
                self.state = S::Idle;
                Some(Action::StopPushToTalk)
            }
            (S::Holding, Input::OtherKey(_)) => {
                self.state = S::Idle;
                Some(Action::Cancel)
            }

            (S::Continuous, Input::ComboDown(_)) => {
                self.state = S::ContinuousPressed;
                None
            }
            (S::ContinuousPressed, Input::ComboUp(_)) => {
                self.state = S::Idle;
                Some(Action::StopContinuous)
            }
            // Atalho do Windows usado durante o modo contínuo: segue ditando.
            (S::ContinuousPressed, Input::OtherKey(_)) => {
                self.state = S::Continuous;
                None
            }

            // Eventos repetidos ou fora de ordem (ex.: auto-repeat do teclado).
            _ => None,
        }
    }
}

impl Default for GestureMachine {
    fn default() -> Self {
        Self::new(GestureConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Input::*;

    fn run_all(inputs: &[Input]) -> Vec<Action> {
        let mut m = GestureMachine::default();
        inputs.iter().flat_map(|&i| m.handle(i)).collect()
    }

    /// Só as ações de escuta (sem Arm/Disarm).
    fn run(inputs: &[Input]) -> Vec<Action> {
        run_all(inputs)
            .into_iter()
            .filter(|a| !matches!(a, Action::Arm | Action::Disarm))
            .collect()
    }

    #[test]
    fn arm_on_press_disarm_after_lone_tap() {
        let actions = run_all(&[ComboDown(0), ComboUp(100), Tick(300), Tick(460)]);
        assert_eq!(actions, [Action::Arm, Action::Disarm]);
    }

    #[test]
    fn hold_is_armed_before_start() {
        let actions = run_all(&[ComboDown(0), Tick(260), ComboUp(900)]);
        assert_eq!(
            actions,
            [Action::Arm, Action::StartPushToTalk, Action::StopPushToTalk]
        );
    }

    #[test]
    fn double_tap_stays_armed_between_taps() {
        let actions = run_all(&[ComboDown(0), ComboUp(80), ComboDown(200), ComboUp(260)]);
        assert_eq!(actions, [Action::Arm, Action::StartContinuous]);
    }

    #[test]
    fn windows_shortcut_disarms() {
        let actions = run_all(&[ComboDown(0), OtherKey(50), ComboUp(90)]);
        assert_eq!(actions, [Action::Arm, Action::Disarm]);
    }

    #[test]
    fn hold_starts_and_stops_push_to_talk() {
        let actions = run(&[ComboDown(0), Tick(100), Tick(250), Tick(900), ComboUp(1000)]);
        assert_eq!(actions, [Action::StartPushToTalk, Action::StopPushToTalk]);
    }

    #[test]
    fn hold_detected_on_release_without_ticks() {
        let actions = run(&[ComboDown(0), ComboUp(600)]);
        assert_eq!(actions, [Action::StartPushToTalk, Action::StopPushToTalk]);
    }

    #[test]
    fn single_tap_does_nothing() {
        let actions = run(&[ComboDown(0), ComboUp(100), Tick(500), Tick(2000)]);
        assert!(actions.is_empty());
    }

    #[test]
    fn double_tap_toggles_continuous_mode() {
        let actions = run(&[
            ComboDown(0),
            ComboUp(100),
            ComboDown(300),
            ComboUp(380),
            Tick(5000),
            ComboDown(10_000),
            ComboUp(10_080),
        ]);
        assert_eq!(actions, [Action::StartContinuous, Action::StopContinuous]);
    }

    #[test]
    fn second_tap_too_late_is_a_new_first_tap() {
        let actions = run(&[ComboDown(0), ComboUp(100), ComboDown(500), ComboUp(560)]);
        assert!(actions.is_empty());
    }

    #[test]
    fn second_press_held_is_push_to_talk() {
        let actions = run(&[
            ComboDown(0),
            ComboUp(100),
            ComboDown(300),
            Tick(600),
            ComboUp(900),
        ]);
        assert_eq!(actions, [Action::StartPushToTalk, Action::StopPushToTalk]);
    }

    #[test]
    fn other_key_before_hold_lets_windows_shortcut_through() {
        let actions = run(&[ComboDown(0), OtherKey(80), ComboUp(150), Tick(1000)]);
        assert!(actions.is_empty());
    }

    #[test]
    fn other_key_while_holding_cancels() {
        let actions = run(&[ComboDown(0), Tick(300), OtherKey(400), ComboUp(500)]);
        assert_eq!(actions, [Action::StartPushToTalk, Action::Cancel]);
    }

    #[test]
    fn windows_shortcut_during_continuous_keeps_dictating() {
        let actions = run(&[
            ComboDown(0),
            ComboUp(50),
            ComboDown(150),
            ComboUp(200),
            ComboDown(3000),
            OtherKey(3050),
            ComboUp(3100),
        ]);
        assert_eq!(actions, [Action::StartContinuous]);
    }

    #[test]
    fn key_repeat_is_ignored() {
        let actions = run(&[
            ComboDown(0),
            ComboDown(30),
            ComboDown(60),
            Tick(300),
            ComboUp(400),
        ]);
        assert_eq!(actions, [Action::StartPushToTalk, Action::StopPushToTalk]);
    }

    #[test]
    fn thresholds_are_configurable() {
        let mut m = GestureMachine::new(GestureConfig {
            hold_threshold_ms: 500,
            double_tap_ms: 200,
        });
        assert_eq!(m.handle(ComboDown(0)).next(), Some(Action::Arm));
        assert_eq!(m.handle(Tick(400)).next(), None);
        assert_eq!(m.handle(Tick(500)).next(), Some(Action::StartPushToTalk));
    }
}
