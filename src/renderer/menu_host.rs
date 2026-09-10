//! One host-facing adapter for all canonical menu input devices.

use crate::{
    controller::host::{ControllerInfo, ControllerState},
    menu::{
        ConditionId, InputFrame, MenuDefinition, MenuEffect, MenuRuntime, RuntimeError,
        interaction::{InteractionError, InteractionMap},
    },
};
use sdl3::keyboard::Scancode;
use thiserror::Error;

use super::{
    controls::{ControllerPorts, DigitalMenuInput, KeyboardInput, PointerInput},
    viewport::PresentationTransform,
};

const HOST_INPUT_SOURCES: usize = 5;

/// Owns input latches and one renderer-independent menu runtime.
///
/// SDL events only update latches. [`Self::tick`] is the sole state-machine
/// boundary, so event frequency and ordering cannot advance menu time.
pub struct MenuHost {
    runtime: MenuRuntime,
    interaction: InteractionMap,
    keyboard: KeyboardInput,
    pointer: PointerInput,
    controller_ports: ControllerPorts,
    digital: DigitalMenuInput<HOST_INPUT_SOURCES>,
    input_armed: bool,
    previous_buttons: [u32; HOST_INPUT_SOURCES],
    previous_accepts_host_input: bool,
}

impl MenuHost {
    pub fn new(
        definition: MenuDefinition,
        interaction: InteractionMap,
        enabled_flags: impl IntoIterator<Item = ConditionId>,
    ) -> Result<Self, MenuHostError> {
        interaction.validate(&definition)?;
        let runtime = MenuRuntime::new(definition, enabled_flags)?;
        Ok(Self {
            runtime,
            interaction,
            keyboard: KeyboardInput::default(),
            pointer: PointerInput::default(),
            controller_ports: ControllerPorts::default(),
            digital: DigitalMenuInput::default(),
            input_armed: true,
            previous_buttons: [0; HOST_INPUT_SOURCES],
            previous_accepts_host_input: true,
        })
    }

    pub const fn runtime(&self) -> &MenuRuntime {
        &self.runtime
    }

    pub fn key(&mut self, code: Scancode, down: bool, repeat: bool) {
        self.keyboard.key(code, down, repeat);
    }

    pub fn pointer_motion(
        &mut self,
        window_point: [f32; 2],
        transform: Option<PresentationTransform>,
    ) {
        self.pointer
            .motion(window_point, transform, &self.interaction);
    }

    pub fn pointer_primary_down(
        &mut self,
        window_point: [f32; 2],
        transform: Option<PresentationTransform>,
    ) {
        self.pointer
            .primary_down(window_point, transform, &self.interaction);
    }

    pub fn pointer_reproject(&mut self, transform: Option<PresentationTransform>) {
        self.pointer.reproject(transform, &self.interaction);
    }

    pub fn pointer_leave(&mut self) {
        self.pointer.leave();
    }

    /// Release every host-side latch after focus loss.
    pub fn clear_input(&mut self) {
        self.keyboard.clear();
        self.pointer.clear();
        self.controller_ports.release();
        self.input_armed = false;
        self.previous_accepts_host_input = false;
    }

    /// Advance exactly one canonical menu tick.
    pub fn tick(
        &mut self,
        controllers: &[(ControllerInfo, ControllerState)],
        accepts_host_input: bool,
    ) -> Vec<MenuEffect> {
        let controller_buttons = self.controller_ports.sample(controllers);
        let source_buttons = [
            self.keyboard.sample(),
            controller_buttons[0],
            controller_buttons[1],
            controller_buttons[2],
            controller_buttons[3],
        ];
        self.previous_buttons = source_buttons;
        self.previous_accepts_host_input = accepts_host_input;
        self.tick_with_buttons(source_buttons, accepts_host_input, true)
    }

    /// Advance one old catch-up tick using the last state sampled on time.
    ///
    /// Newly latched keyboard and pointer edges remain pending for the final,
    /// current tick instead of being replayed backward through missed time.
    pub fn tick_previous(&mut self) -> Vec<MenuEffect> {
        self.tick_with_buttons(
            self.previous_buttons,
            self.previous_accepts_host_input,
            false,
        )
    }

    fn tick_with_buttons(
        &mut self,
        source_buttons: [u32; HOST_INPUT_SOURCES],
        accepts_host_input: bool,
        sample_pointer: bool,
    ) -> Vec<MenuEffect> {
        let mut commands = self.digital.sample(source_buttons);
        if !accepts_host_input {
            self.input_armed = false;
            if sample_pointer {
                self.pointer.sample(false);
            }
            return self.runtime.tick(&InputFrame::default());
        }
        if !self.input_armed {
            if source_buttons.into_iter().all(|buttons| buttons == 0) {
                self.input_armed = true;
            } else {
                if sample_pointer {
                    self.pointer.sample(false);
                }
                return self.runtime.tick(&InputFrame::default());
            }
        }
        if sample_pointer {
            commands.extend(self.pointer.sample(self.runtime.cooldown_frames() == 0));
        }
        self.runtime.tick(&InputFrame::new(commands))
    }
}

#[derive(Debug, Error)]
pub enum MenuHostError {
    #[error(transparent)]
    Interaction(#[from] InteractionError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        menu::{
            ActionTrigger, MenuEffect,
            interaction::{HitRect, ItemHitRegion},
            melee::{MainItem, main_definition, main_interaction_map},
        },
        renderer::viewport::MELEE_AUTHORED_EXTENT,
    };

    fn test_interaction() -> InteractionMap {
        InteractionMap {
            regions: vec![
                ItemHitRegion {
                    item: MainItem::OnePlayer.id().into(),
                    bounds: HitRect {
                        min: [0.0, 0.0],
                        max: [640.0, 240.0],
                    },
                },
                ItemHitRegion {
                    item: MainItem::Versus.id().into(),
                    bounds: HitRect {
                        min: [0.0, 240.0],
                        max: [640.0, 480.0],
                    },
                },
            ],
        }
    }

    #[test]
    fn pointer_and_keyboard_share_one_tick_and_runtime() {
        let mut host = MenuHost::new(main_definition(), test_interaction(), []).unwrap();
        let transform =
            PresentationTransform::new([1280, 720], [2560, 1440], MELEE_AUTHORED_EXTENT);

        host.pointer_motion([640.0, 540.0], transform);
        host.key(Scancode::Return, true, false);
        host.key(Scancode::Return, false, false);
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }

        let effects = host.tick(&[], true);
        assert!(matches!(
            effects.as_slice(),
            [MenuEffect::SelectionChanged { selected, .. }]
                if selected.as_str() == MainItem::Versus.id()
        ));
        assert_eq!(host.runtime().cooldown_frames(), 0);

        host.pointer_primary_down([640.0, 540.0], transform);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::ActionRequested {
                trigger: ActionTrigger::Confirm,
                item: Some(item),
                action,
            }] if item.as_str() == MainItem::Versus.id()
                && action.destination.as_str() == MainItem::Versus.destination()
        ));
    }

    #[test]
    fn projected_main_label_click_reaches_its_exact_destination() {
        let mut host = MenuHost::new(main_definition(), main_interaction_map(), []).unwrap();
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }
        let transform =
            PresentationTransform::new([1280, 960], [2560, 1920], MELEE_AUTHORED_EXTENT);
        // Twice the independently recorded authored center of the VS label.
        host.pointer_primary_down([386.454_7, 388.632_2], transform);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [
                MenuEffect::SelectionChanged { selected, .. },
                MenuEffect::ActionRequested {
                    trigger: ActionTrigger::Confirm,
                    item: Some(item),
                    action,
                }
            ] if selected.as_str() == MainItem::Versus.id()
                && item == selected
                && action.destination.as_str() == MainItem::Versus.destination()
        ));
    }

    #[test]
    fn focus_loss_drops_pending_edges_but_menu_time_keeps_advancing() {
        let mut host = MenuHost::new(main_definition(), test_interaction(), []).unwrap();
        host.key(Scancode::Return, true, false);
        assert!(host.tick(&[], false).is_empty());
        assert_eq!(host.runtime().cooldown_frames(), 19);
        for _ in 0..19 {
            assert!(host.tick(&[], true).is_empty());
        }
        assert_eq!(host.runtime().cooldown_frames(), 0);
        assert!(host.tick(&[], true).is_empty());
    }

    #[test]
    fn held_input_must_return_neutral_after_focus_loss() {
        let mut host = MenuHost::new(main_definition(), test_interaction(), []).unwrap();
        host.clear_input();
        for _ in 0..20 {
            assert!(host.tick(&[], false).is_empty());
        }

        host.key(Scancode::Return, true, false);
        assert!(host.tick(&[], true).is_empty());
        host.key(Scancode::Return, false, false);
        assert!(host.tick(&[], true).is_empty());
        host.key(Scancode::Return, true, false);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::ActionRequested { .. }]
        ));
    }

    #[test]
    fn catch_up_ticks_repeat_the_previous_hold_not_new_edges() {
        let mut host = MenuHost::new(main_definition(), test_interaction(), []).unwrap();
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }

        host.key(Scancode::Down, true, false);
        host.tick(&[], true);
        assert_eq!(host.runtime().selected().id.as_str(), MainItem::Versus.id());
        for _ in 0..20 {
            assert!(host.tick_previous().is_empty());
        }
        host.tick_previous();
        assert_eq!(
            host.runtime().selected().id.as_str(),
            MainItem::Trophies.id()
        );

        // A new confirmation edge remains latched while old ticks are caught up.
        host.key(Scancode::Return, true, false);
        assert!(host.tick_previous().is_empty());
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::ActionRequested { .. }]
        ));
    }
}
