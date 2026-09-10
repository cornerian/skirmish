//! Native menu bindings; simulation controller calibration remains separate.
use std::collections::HashSet;

use crate::{
    controller::host::{Button, ControllerId, ControllerInfo, ControllerState},
    menu::{Direction, ItemId, MenuCommand, interaction::InteractionMap},
};
use sdl3::keyboard::Scancode;

use super::viewport::PresentationTransform;

/// Dolphin/HSD digital button bits supplied to the original menu runtime.
pub mod pad {
    pub const LEFT: u32 = 1;
    pub const RIGHT: u32 = 1 << 1;
    pub const DOWN: u32 = 1 << 2;
    pub const UP: u32 = 1 << 3;
    pub const A: u32 = 1 << 8;
    pub const B: u32 = 1 << 9;
    pub const START: u32 = 1 << 12;
    pub const STICK_UP: u32 = 1 << 16;
    pub const STICK_DOWN: u32 = 1 << 17;
    pub const STICK_LEFT: u32 = 1 << 18;
    pub const STICK_RIGHT: u32 = 1 << 19;
}

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
        Scancode::Up => pad::UP,
        Scancode::Down => pad::DOWN,
        Scancode::Left => pad::LEFT,
        Scancode::Right => pad::RIGHT,
        Scancode::Return | Scancode::KpEnter | Scancode::Space | Scancode::Z => pad::A,
        Scancode::Escape | Scancode::Backspace | Scancode::X => pad::B,
        _ => 0,
    }
}

fn controller_button(button: Button) -> u32 {
    match button {
        Button::South => pad::A,
        // SDL maps the GameCube's physical B to West. Retain both physical
        // positions while adapting devices to the original HSD button word.
        Button::East | Button::West => pad::B,
        Button::Start => pad::START,
        Button::DPadUp => pad::UP,
        Button::DPadDown => pad::DOWN,
        Button::DPadLeft => pad::LEFT,
        Button::DPadRight => pad::RIGHT,
        _ => 0,
    }
}

const INITIAL_REPEAT_DELAY: u16 = 20;
const FIRST_REPEAT_PERIOD: u16 = 8;
const FASTER_REPEAT_AGE: u16 = 40;
const FASTER_REPEAT_PERIOD: u16 = 4;
const FASTEST_REPEAT_AGE: u16 = 100;
const FASTEST_REPEAT_PERIOD: u16 = 2;

#[derive(Clone, Copy)]
struct RepeatState {
    buttons: u32,
    timer: u16,
    age: u16,
}

impl Default for RepeatState {
    fn default() -> Self {
        Self {
            buttons: 0,
            timer: INITIAL_REPEAT_DELAY,
            age: 0,
        }
    }
}

impl RepeatState {
    fn sample(&mut self, buttons: u32) -> DigitalSample {
        let triggered = buttons & !self.buttons;
        let released = self.buttons & !buttons;
        self.buttons = buttons;

        if triggered != 0 || released != 0 {
            self.timer = INITIAL_REPEAT_DELAY;
            self.age = 0;
            return DigitalSample {
                triggered,
                repeated: triggered,
            };
        }

        if self.age < FASTEST_REPEAT_AGE {
            self.age += 1;
        }
        if self.timer != 0 {
            self.timer -= 1;
            return DigitalSample {
                triggered: 0,
                repeated: 0,
            };
        }

        self.timer = if self.age >= FASTEST_REPEAT_AGE {
            FASTEST_REPEAT_PERIOD
        } else if self.age >= FASTER_REPEAT_AGE {
            FASTER_REPEAT_PERIOD
        } else {
            FIRST_REPEAT_PERIOD
        };
        DigitalSample {
            triggered: 0,
            repeated: buttons,
        }
    }
}

#[derive(Clone, Copy)]
struct DigitalSample {
    triggered: u32,
    repeated: u32,
}

/// Converts independently repeated digital sources into canonical menu commands.
///
/// Each keyboard or controller source retains Melee's 20/8/4/2-frame repeat
/// schedule before the results are combined, matching the game's aggregate
/// virtual port without making host devices share a repeat timer.
pub struct DigitalMenuInput<const SOURCES: usize> {
    sources: [RepeatState; SOURCES],
}

impl<const SOURCES: usize> Default for DigitalMenuInput<SOURCES> {
    fn default() -> Self {
        Self {
            sources: [RepeatState::default(); SOURCES],
        }
    }
}

impl<const SOURCES: usize> DigitalMenuInput<SOURCES> {
    pub fn sample(&mut self, buttons: [u32; SOURCES]) -> Vec<MenuCommand> {
        let (triggered, repeated) = self.sources.iter_mut().zip(buttons).fold(
            (0, 0),
            |(all_triggered, all_repeated), (state, buttons)| {
                let sample = state.sample(buttons);
                (
                    all_triggered | sample.triggered,
                    all_repeated | sample.repeated,
                )
            },
        );

        let mut commands = Vec::with_capacity(7);
        if triggered & pad::A != 0 {
            commands.push(MenuCommand::Confirm);
        }
        if triggered & pad::START != 0 {
            commands.push(MenuCommand::Start);
        }
        if triggered & pad::B != 0 {
            commands.push(MenuCommand::Back);
        }
        for (mask, direction) in [
            (pad::UP | pad::STICK_UP, Direction::Up),
            (pad::DOWN | pad::STICK_DOWN, Direction::Down),
            (pad::LEFT | pad::STICK_LEFT, Direction::Left),
            (pad::RIGHT | pad::STICK_RIGHT, Direction::Right),
        ] {
            if repeated & mask != 0 {
                commands.push(MenuCommand::Navigate(direction));
            }
        }
        commands
    }

    pub fn clear(&mut self) {
        self.sources.fill(RepeatState::default());
    }
}

/// Bounded SDL-pointer state translated into canonical menu commands.
///
/// A hover transition is retained until an input-enabled menu tick accepts it.
/// A click is a one-tick edge and always carries its own hit-tested focus target,
/// so a stale hover can never activate another item.
#[derive(Default)]
pub struct PointerInput {
    hovered: Option<ItemId>,
    pending_focus: Option<ItemId>,
    pending_click: Option<ItemId>,
}

impl PointerInput {
    pub fn motion(
        &mut self,
        window_point: [f32; 2],
        transform: Option<PresentationTransform>,
        map: &InteractionMap,
    ) {
        let target = pointer_target(window_point, transform, map);
        if target != self.hovered {
            self.hovered = target.clone();
            self.pending_focus = target;
        }
    }

    pub fn primary_down(
        &mut self,
        window_point: [f32; 2],
        transform: Option<PresentationTransform>,
        map: &InteractionMap,
    ) {
        let target = pointer_target(window_point, transform, map);
        self.hovered = target.clone();
        self.pending_click = target.clone();
        self.pending_focus = target;
    }

    /// Drain commands for one fixed tick.
    ///
    /// `accepts_input` must reflect the canonical runtime's entrance/action
    /// cooldown. Click edges are discarded while blocked just like controller
    /// triggers, while a stationary hover remains pending for the first open
    /// tick.
    pub fn sample(&mut self, accepts_input: bool) -> Vec<MenuCommand> {
        if !accepts_input {
            self.pending_click = None;
            return Vec::new();
        }
        if let Some(item) = self.pending_click.take() {
            if self.pending_focus.as_ref() == Some(&item) {
                self.pending_focus = None;
            }
            return vec![MenuCommand::Focus(item), MenuCommand::Confirm];
        }
        self.pending_focus
            .take()
            .map(MenuCommand::Focus)
            .into_iter()
            .collect()
    }

    pub fn clear(&mut self) {
        self.hovered = None;
        self.pending_focus = None;
        self.pending_click = None;
    }
}

fn pointer_target(
    window_point: [f32; 2],
    transform: Option<PresentationTransform>,
    map: &InteractionMap,
) -> Option<ItemId> {
    transform
        .and_then(|transform| transform.window_to_authored(window_point))
        .and_then(|point| map.hit_test(point))
        .cloned()
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
                pad::STICK_LEFT,
                pad::STICK_RIGHT,
            ) | axis(
                state.left_stick[1],
                self.directions[port],
                pad::STICK_UP,
                pad::STICK_DOWN,
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
    use crate::{
        menu::{
            AnimationCue, AnimationId, EnableCondition, FrameRange, InputFrame, ItemPresentation,
            MenuAction, MenuDefinition, MenuEffect, MenuItem, MenuRuntime, NavigationAxis, SoundId,
            StartBehavior, interaction::HitRect, interaction::ItemHitRegion,
        },
        renderer::viewport::MELEE_AUTHORED_EXTENT,
    };

    #[test]
    fn short_key_taps_survive_until_one_tick_and_focus_loss_releases_everything() {
        let mut input = KeyboardInput::default();
        input.key(Scancode::Return, true, false);
        input.key(Scancode::Return, false, false);
        assert_eq!(input.sample(), pad::A);
        assert_eq!(input.sample(), 0);
        input.key(Scancode::Down, true, false);
        input.key(Scancode::Return, true, true);
        assert_eq!(input.sample(), pad::DOWN);
        input.clear();
        assert_eq!(input.sample(), 0);
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
        let right = pad::STICK_RIGHT;
        assert_eq!(
            ports.sample(&[device(10, 20_000), device(20, 20_000)]),
            [right, right, 0, 0]
        );
        assert_eq!(ports.sample(&[device(20, 20_000)]), [0, right, 0, 0]);
        assert_eq!(
            ports.sample(&[device(20, 20_000), device(30, -20_000)]),
            [pad::STICK_LEFT, right, 0, 0]
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
            assert_eq!(ports.sample(&[device(1, x)])[0], expected);
        }
    }

    #[test]
    fn digital_actions_are_edges_and_repress_after_release() {
        let mut input = DigitalMenuInput::<1>::default();
        assert_eq!(input.sample([pad::A]), [MenuCommand::Confirm]);
        assert!(input.sample([pad::A]).is_empty());
        assert!(input.sample([0]).is_empty());
        assert_eq!(input.sample([pad::A]), [MenuCommand::Confirm]);
    }

    #[test]
    fn directional_repeat_uses_the_source_20_8_4_2_schedule() {
        let mut input = DigitalMenuInput::<1>::default();
        let emitted = (0..=109)
            .filter(|_| input.sample([pad::UP]) == [MenuCommand::Navigate(Direction::Up)])
            .collect::<Vec<_>>();
        assert_eq!(
            emitted,
            [
                0, 21, 30, 39, 48, 53, 58, 63, 68, 73, 78, 83, 88, 93, 98, 103, 106, 109
            ]
        );
    }

    #[test]
    fn repeat_state_is_independent_per_physical_source() {
        let mut input = DigitalMenuInput::<2>::default();
        assert_eq!(
            input.sample([pad::UP, 0]),
            [MenuCommand::Navigate(Direction::Up)]
        );
        for _ in 0..9 {
            assert!(input.sample([pad::UP, 0]).is_empty());
        }
        assert_eq!(
            input.sample([pad::UP, pad::DOWN]),
            [MenuCommand::Navigate(Direction::Down)]
        );
        for _ in 0..10 {
            assert!(input.sample([pad::UP, pad::DOWN]).is_empty());
        }
        assert_eq!(
            input.sample([pad::UP, pad::DOWN]),
            [MenuCommand::Navigate(Direction::Up)]
        );
    }

    #[test]
    fn clear_releases_inputs_and_same_frame_priority_remains_in_the_runtime() {
        let mut input = DigitalMenuInput::<1>::default();
        assert_eq!(
            input.sample([pad::START | pad::B]),
            [MenuCommand::Start, MenuCommand::Back]
        );
        input.clear();
        assert_eq!(
            input.sample([pad::START | pad::B]),
            [MenuCommand::Start, MenuCommand::Back]
        );
    }

    fn pointer_definition() -> MenuDefinition {
        let item = |id: &str| MenuItem {
            id: id.into(),
            enabled_when: EnableCondition::Always,
            presentation: ItemPresentation {
                description: None,
                animation: AnimationCue {
                    id: AnimationId::from(format!("idle.{id}")),
                    frames: FrameRange {
                        start: 0.0,
                        end: 1.0,
                        loop_start: None,
                    },
                },
            },
            confirm: Some(MenuAction {
                destination: format!("destination.{id}").into(),
                sound: Some(SoundId(1)),
                transition: None,
                cooldown_frames: 5,
            }),
        };
        MenuDefinition {
            id: "pointer-test".into(),
            items: vec![item("one"), item("two")],
            default_item: "one".into(),
            navigation_axis: NavigationAxis::Vertical,
            initial_cooldown_frames: 0,
            move_sound: Some(SoundId(2)),
            back: None,
            start: StartBehavior::Ignore,
        }
    }

    fn pointer_map() -> InteractionMap {
        InteractionMap {
            regions: vec![
                ItemHitRegion {
                    item: "one".into(),
                    bounds: HitRect {
                        min: [0.0, 0.0],
                        max: [640.0, 240.0],
                    },
                },
                ItemHitRegion {
                    item: "two".into(),
                    bounds: HitRect {
                        min: [0.0, 240.0],
                        max: [640.0, 480.0],
                    },
                },
            ],
        }
    }

    #[test]
    fn pointer_maps_high_density_window_coordinates_and_deduplicates_hover() {
        let transform =
            PresentationTransform::new([1280, 720], [2560, 1440], MELEE_AUTHORED_EXTENT);
        let map = pointer_map();
        let mut pointer = PointerInput::default();

        pointer.motion([640.0, 180.0], transform, &map);
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("one".into())]);
        pointer.motion([640.0, 180.0], transform, &map);
        assert!(pointer.sample(true).is_empty());
        pointer.motion([640.0, 540.0], transform, &map);
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("two".into())]);
    }

    #[test]
    fn gutters_and_leave_do_not_change_canonical_focus() {
        let transform = PresentationTransform::new([1280, 720], [1280, 720], MELEE_AUTHORED_EXTENT);
        let map = pointer_map();
        let mut pointer = PointerInput::default();

        pointer.motion([100.0, 360.0], transform, &map);
        assert!(pointer.sample(true).is_empty());
        pointer.motion([640.0, 180.0], transform, &map);
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("one".into())]);
        pointer.clear();
        assert!(pointer.sample(true).is_empty());
        pointer.motion([640.0, 180.0], transform, &map);
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("one".into())]);
    }

    #[test]
    fn click_retests_its_position_and_wins_over_later_motion() {
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let map = pointer_map();
        let mut pointer = PointerInput::default();

        pointer.motion([10.0, 10.0], transform, &map);
        pointer.primary_down([10.0, 300.0], transform, &map);
        pointer.motion([10.0, 10.0], transform, &map);
        assert_eq!(
            pointer.sample(true),
            [MenuCommand::Focus("two".into()), MenuCommand::Confirm]
        );
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("one".into())]);
    }

    #[test]
    fn blocked_tick_drops_click_but_retries_stationary_hover() {
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let map = pointer_map();
        let mut pointer = PointerInput::default();

        pointer.motion([10.0, 300.0], transform, &map);
        pointer.primary_down([10.0, 300.0], transform, &map);
        assert!(pointer.sample(false).is_empty());
        assert_eq!(pointer.sample(true), [MenuCommand::Focus("two".into())]);
        assert!(pointer.sample(true).is_empty());
    }

    #[test]
    fn pointer_click_uses_the_canonical_runtime_action_path() {
        let definition = pointer_definition();
        let map = pointer_map();
        map.validate(&definition).unwrap();
        let mut runtime = MenuRuntime::new(definition, []).unwrap();
        let transform = PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT);
        let mut pointer = PointerInput::default();

        pointer.primary_down([10.0, 300.0], transform, &map);
        let effects = runtime.tick(&InputFrame::new(pointer.sample(true)));
        assert!(matches!(
            effects.as_slice(),
            [
                MenuEffect::SelectionChanged { selected, .. },
                MenuEffect::ActionRequested { item: Some(action_item), action, .. }
            ] if selected.as_str() == "two"
                && action_item == selected
                && action.destination.as_str() == "destination.two"
        ));
    }
}
