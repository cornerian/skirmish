//! Native menu bindings; simulation controller calibration remains separate.
use std::collections::HashSet;

use crate::controller::host::{Button, ControllerId, ControllerInfo, ControllerState};
use menus::input::pad;
use sdl3::keyboard::Scancode;

/// Retains a short key tap until the next menu tick and ignores OS key repeat.
#[derive(Default)]
pub struct KeyboardInput {
    held: HashSet<Scancode>,
    pressed: u32,
}

impl KeyboardInput {
    pub fn key(&mut self, code: Scancode, down: bool, repeat: bool) {
        if repeat {
            return;
        }
        if down {
            if self.held.insert(code) {
                self.pressed |= key_button(code);
            }
        } else {
            self.held.remove(&code);
        }
    }

    pub fn sample(&mut self) -> u32 {
        let pressed = std::mem::take(&mut self.pressed);
        self.held
            .iter()
            .fold(pressed, |all, &key| all | key_button(key))
    }

    pub fn clear(&mut self) {
        self.held.clear();
        self.pressed = 0;
    }
}

fn key_button(code: Scancode) -> u32 {
    match code {
        Scancode::Up => pad::UP as u32,
        Scancode::Down => pad::DOWN as u32,
        Scancode::Left => pad::LEFT as u32,
        Scancode::Right => pad::RIGHT as u32,
        Scancode::Return | Scancode::KpEnter | Scancode::Space | Scancode::Z => pad::A as u32,
        Scancode::Escape | Scancode::Backspace | Scancode::X => pad::B as u32,
        _ => 0,
    }
}

fn controller_button(button: Button) -> u32 {
    let mapped = match button {
        Button::South => pad::A,
        // SDL maps the GameCube's physical B to West. Accept it alongside the
        // conventional East cancel button for this menu presentation only.
        Button::East | Button::West => pad::B,
        Button::Start => pad::START,
        Button::DPadUp => pad::UP,
        Button::DPadDown => pad::DOWN,
        Button::DPadLeft => pad::LEFT,
        Button::DPadRight => pad::RIGHT,
        _ => 0,
    };
    mapped as u32
}

/// Stable menu ports across hot-plug events. Disconnecting a device releases its
/// inputs without shifting another controller into its port.
#[derive(Default)]
pub struct ControllerPorts {
    ids: [Option<ControllerId>; 4],
    directions: [u32; 4],
}

impl ControllerPorts {
    pub fn sample(&mut self, devices: &[(ControllerInfo, ControllerState)]) -> [u32; 4] {
        for port in 0..4 {
            if self.ids[port].is_some_and(|id| !devices.iter().any(|(info, _)| info.id == id)) {
                self.ids[port] = None;
                self.directions[port] = 0;
            }
        }
        for (info, _) in devices {
            if self.ids.contains(&Some(info.id)) {
                continue;
            }
            let preferred = info
                .player_index
                .map(usize::from)
                .filter(|&port| port < 4 && self.ids[port].is_none());
            if let Some(port) = preferred.or_else(|| self.ids.iter().position(Option::is_none)) {
                self.ids[port] = Some(info.id);
            }
        }
        std::array::from_fn(|port| {
            let Some((_, state)) = devices
                .iter()
                .find(|(info, _)| Some(info.id) == self.ids[port])
            else {
                return 0;
            };
            let buttons = state
                .buttons
                .iter()
                .fold(0, |all, button| all | controller_button(button));
            // Menu-only policy: enter a direction at half travel, release below
            // quarter travel. SDL's vertical axis is negative upwards.
            self.directions[port] = axis(
                state.left_stick[0],
                self.directions[port],
                pad::STICK_LEFT as u32,
                pad::STICK_RIGHT as u32,
            ) | axis(
                state.left_stick[1],
                self.directions[port],
                pad::STICK_UP as u32,
                pad::STICK_DOWN as u32,
            );
            buttons | self.directions[port]
        })
    }

    pub fn release(&mut self) {
        self.directions = [0; 4];
    }
}

fn axis(value: i16, previous: u32, negative: u32, positive: u32) -> u32 {
    if value <= -16_384 {
        negative
    } else if value >= 16_384 {
        positive
    } else if value.unsigned_abs() <= 8192 {
        0
    } else if value < 0 {
        previous & negative
    } else {
        previous & positive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::menu::{MenuEvent, MenuSession};
    use menus::{Action, Menu, MenuState, Unlocks};

    #[test]
    fn short_key_taps_survive_until_one_tick_and_focus_loss_releases_everything() {
        let mut input = KeyboardInput::default();
        input.key(Scancode::Return, true, false);
        input.key(Scancode::Return, false, false);
        assert_eq!(input.sample(), pad::A as u32);
        assert_eq!(input.sample(), 0);
        input.key(Scancode::Down, true, false);
        input.key(Scancode::Return, true, true);
        assert_eq!(input.sample(), pad::DOWN as u32);
        input.clear();
        assert_eq!(input.sample(), 0);
    }

    #[test]
    fn standard_and_gamecube_back_buttons_return_from_a_selected_menu() {
        for back in [Button::East, Button::West] {
            let mut session =
                MenuSession::from_state(MenuState::at(Menu::Main, 0, Unlocks::default()).unwrap());
            assert_eq!(
                session.tick([controller_button(Button::South), 0, 0, 0]),
                Some(MenuEvent::Navigation(Action::Entered(Menu::OnePlayer)))
            );
            assert_eq!(session.view().title, "1-P Mode");
            // Release Confirm and allow the translated transition cooldown.
            for _ in 0..5 {
                session.tick([0; 4]);
            }
            assert_eq!(
                session.tick([controller_button(back), 0, 0, 0]),
                Some(MenuEvent::Navigation(Action::Returned(Menu::Main))),
                "{back:?} must return to the parent menu"
            );
            assert_eq!(session.view().menu, Menu::Main);
            assert_eq!(session.view().selected_label, "1-P Mode");
        }
    }

    fn device(id: u32, x: i16) -> (ControllerInfo, ControllerState) {
        (
            ControllerInfo {
                id: ControllerId(id),
                name: format!("pad {id}"),
                path: None,
                vendor_id: None,
                product_id: None,
                player_index: None,
            },
            ControllerState {
                left_stick: [x, 0],
                ..Default::default()
            },
        )
    }

    #[test]
    fn unplugging_one_pad_does_not_move_another_and_new_pads_reuse_free_ports() {
        let mut ports = ControllerPorts::default();
        let right = pad::STICK_RIGHT as u32;
        assert_eq!(
            ports.sample(&[device(10, 20_000), device(20, 20_000)]),
            [right, right, 0, 0]
        );
        assert_eq!(ports.sample(&[device(20, 20_000)]), [0, right, 0, 0]);
        assert_eq!(
            ports.sample(&[device(20, 20_000), device(30, -20_000)]),
            [pad::STICK_LEFT as u32, right, 0, 0]
        );
        assert_eq!(ports.sample(&[]), [0; 4]);
    }

    #[test]
    fn analog_hysteresis_prevents_repeated_navigation_at_the_threshold() {
        let mut ports = ControllerPorts::default();
        for (x, expected) in [
            (15_000, 0),
            (17_000, pad::STICK_RIGHT),
            (12_000, pad::STICK_RIGHT),
            (8192, 0),
            (-17_000, pad::STICK_LEFT),
            (0, 0),
        ] {
            assert_eq!(ports.sample(&[device(1, x)])[0], expected as u32);
        }
    }
}
