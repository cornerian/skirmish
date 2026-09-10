//! One host-facing adapter for all canonical menu input devices.

use crate::{
    controller::host::{ControllerInfo, ControllerState},
    menu::{
        ConditionId, InputFrame, MenuDefinition, MenuEffect, MenuRuntime, RuntimeError,
        interaction::{InteractionError, InteractionMap},
    },
};
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
    mouse::MouseButton,
};
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

    /// Enter another declarative menu without resetting physical input state.
    ///
    /// Keeping the keyboard/controller edge detectors alive across a menu
    /// handoff prevents a held confirm or back button from becoming a fresh
    /// press in the destination. The last pointer position is re-hit-tested
    /// against the new presentation map, so a stationary mouse can focus the
    /// destination item once its canonical entrance cooldown expires.
    pub fn enter_menu(
        &mut self,
        definition: MenuDefinition,
        interaction: InteractionMap,
        enabled_flags: impl IntoIterator<Item = ConditionId>,
        transform: Option<PresentationTransform>,
    ) -> Result<(), MenuHostError> {
        interaction.validate(&definition)?;
        let runtime = MenuRuntime::new(definition, enabled_flags)?;

        self.runtime = runtime;
        self.interaction = interaction;
        self.pointer.retarget(transform, &self.interaction);
        Ok(())
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

    /// Adapt one SDL pointer event into the host's retained input latches.
    ///
    /// Window identity and focus are checked here so the production event loop
    /// and tests share the same boundary before commands reach the canonical
    /// menu runtime. This does not advance menu time; [`Self::tick`] remains the
    /// only state-machine boundary.
    pub fn handle_sdl_pointer_event(
        &mut self,
        event: &Event,
        expected_window_id: u32,
        focused: bool,
        transform: Option<PresentationTransform>,
    ) -> bool {
        match event {
            Event::MouseMotion {
                window_id, x, y, ..
            } if *window_id == expected_window_id && focused => {
                self.pointer_motion([*x, *y], transform);
                true
            }
            Event::MouseButtonDown {
                window_id,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if *window_id == expected_window_id && focused => {
                self.pointer_primary_down([*x, *y], transform);
                true
            }
            Event::Window {
                window_id,
                win_event: WindowEvent::MouseLeave,
                ..
            } if *window_id == expected_window_id => {
                self.pointer_leave();
                true
            }
            _ => false,
        }
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
            melee::{
                MAIN_MENU_ID, MainItem, VERSUS_ENTRY_COOLDOWN_FRAMES, VERSUS_MENU_ID, VersusItem,
                main_definition, main_interaction_map, resolve_internal_destination,
                versus_definition, versus_interaction_map,
            },
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

    #[test]
    fn entering_a_menu_preserves_button_edges_across_the_handoff() {
        let mut host = MenuHost::new(main_definition(), main_interaction_map(), []).unwrap();
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }

        host.key(Scancode::Return, true, false);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::ActionRequested { .. }]
        ));

        let mut destination = main_definition();
        destination.initial_cooldown_frames = 0;
        host.enter_menu(destination, main_interaction_map(), [], None)
            .unwrap();

        // The Return key is still physically held, so entering a new menu
        // must not reinterpret it as a second press.
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
    fn entering_a_menu_reprojects_stationary_pointer_into_the_new_map() {
        let mut host = MenuHost::new(main_definition(), main_interaction_map(), []).unwrap();
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let versus_center = [193.227_35, 195.221_6];
        host.pointer_motion(versus_center, transform);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::SelectionChanged { selected, .. }]
                if selected.as_str() == MainItem::Versus.id()
        ));

        let mut destination = main_definition();
        destination.id = "mod.menu".into();
        destination.initial_cooldown_frames = 0;
        for (index, item) in destination.items.iter_mut().enumerate() {
            item.id = format!("mod.item.{index}").into();
        }
        destination.default_item = "mod.item.0".into();
        let mut destination_map = main_interaction_map();
        for (index, region) in destination_map.regions.iter_mut().enumerate() {
            region.item = format!("mod.item.{index}").into();
        }

        host.enter_menu(destination, destination_map, [], transform)
            .unwrap();
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::SelectionChanged { selected, .. }]
                if selected.as_str() == "mod.item.1"
        ));
    }

    #[test]
    fn main_to_versus_mouse_handoff_uses_the_new_canonical_runtime() {
        let mut host = MenuHost::new(main_definition(), main_interaction_map(), []).unwrap();
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }

        let versus_center = [193.227_35, 195.221_6];
        host.pointer_primary_down(versus_center, transform);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [
                MenuEffect::SelectionChanged { selected, .. },
                MenuEffect::ActionRequested { action, .. },
            ] if selected.as_str() == MainItem::Versus.id()
                && action.destination.as_str() == "melee.menu.versus"
        ));

        host.enter_menu(versus_definition(), versus_interaction_map(), [], transform)
            .unwrap();
        for _ in 0..VERSUS_ENTRY_COOLDOWN_FRAMES {
            assert!(host.tick(&[], true).is_empty());
        }

        // The stationary pointer now addresses the VS slot at the same
        // authored location. Its first open tick focuses without leaking the
        // click that entered VS.
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::SelectionChanged { selected, .. }]
                if selected.as_str() == VersusItem::Tournament.id()
        ));
        host.pointer_primary_down(versus_center, transform);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::ActionRequested { item: Some(item), action, .. }]
                if item.as_str() == VersusItem::Tournament.id()
                    && action.destination.as_str() == VersusItem::Tournament.destination()
        ));
    }

    #[test]
    fn held_back_does_not_fall_through_from_versus_to_the_title() {
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let mut host = MenuHost::new(versus_definition(), versus_interaction_map(), []).unwrap();
        for _ in 0..VERSUS_ENTRY_COOLDOWN_FRAMES {
            assert!(host.tick(&[], true).is_empty());
        }

        host.key(Scancode::Escape, true, false);
        let effects = host.tick(&[], true);
        let destination = match effects.as_slice() {
            [MenuEffect::ActionRequested { action, .. }] => &action.destination,
            other => panic!("unexpected VS Back effects: {other:?}"),
        };
        let entry = resolve_internal_destination(
            &crate::menu::MenuId::from(VERSUS_MENU_ID),
            destination,
            5,
        )
        .unwrap();
        host.enter_menu(entry.definition, entry.interaction, [], transform)
            .unwrap();

        for _ in 0..5 {
            assert!(host.tick(&[], true).is_empty());
        }
        assert_eq!(host.runtime().definition().id.as_str(), MAIN_MENU_ID);
        assert_eq!(host.runtime().selected().id.as_str(), MainItem::Versus.id());
        assert!(
            host.tick(&[], true).is_empty(),
            "held Back must not become a new Main-menu edge"
        );
    }

    #[test]
    fn rejected_menu_entry_preserves_runtime_pointer_map_and_latches() {
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let mut host = MenuHost::new(main_definition(), main_interaction_map(), []).unwrap();
        for _ in 0..20 {
            assert!(host.tick(&[], true).is_empty());
        }
        host.pointer_motion([193.227_35, 195.221_6], transform);

        let mut invalid_map = versus_interaction_map();
        invalid_map.regions[0].item = "missing.item".into();
        assert!(
            host.enter_menu(versus_definition(), invalid_map, [], transform)
                .is_err()
        );
        assert_eq!(host.runtime().definition().id.as_str(), MAIN_MENU_ID);
        assert!(matches!(
            host.tick(&[], true).as_slice(),
            [MenuEffect::SelectionChanged { selected, .. }]
                if selected.as_str() == MainItem::Versus.id()
        ));
    }
}
