//! Native controller discovery and polling through SDL3.
//!
//! SDL reports controls by physical position. For example, [`Button::South`]
//! is Xbox A, Switch B, PlayStation Cross, and GameCube A. Keeping that
//! convention here lets gameplay bindings decide what an action means.

use sdl3::{
    EventPump, GamepadSubsystem, Sdl,
    gamepad::{Axis as SdlAxis, Button as SdlButton, Gamepad},
};
use std::collections::{BTreeMap, BTreeSet};

/// An SDL joystick instance ID. It remains stable until that device disconnects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControllerId(pub u32);

/// Physical button positions shared by SDL's controller mapping database.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Button {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Misc1,
    RightPaddle1,
    LeftPaddle1,
    RightPaddle2,
    LeftPaddle2,
    Touchpad,
    Misc2,
    Misc3,
    Misc4,
    Misc5,
    Misc6,
}

impl Button {
    pub const ALL: [Self; 26] = [
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::Back,
        Self::Guide,
        Self::Start,
        Self::LeftStick,
        Self::RightStick,
        Self::LeftShoulder,
        Self::RightShoulder,
        Self::DPadUp,
        Self::DPadDown,
        Self::DPadLeft,
        Self::DPadRight,
        Self::Misc1,
        Self::RightPaddle1,
        Self::LeftPaddle1,
        Self::RightPaddle2,
        Self::LeftPaddle2,
        Self::Touchpad,
        Self::Misc2,
        Self::Misc3,
        Self::Misc4,
        Self::Misc5,
        Self::Misc6,
    ];

    const fn sdl(self) -> SdlButton {
        match self {
            Self::South => SdlButton::South,
            Self::East => SdlButton::East,
            Self::West => SdlButton::West,
            Self::North => SdlButton::North,
            Self::Back => SdlButton::Back,
            Self::Guide => SdlButton::Guide,
            Self::Start => SdlButton::Start,
            Self::LeftStick => SdlButton::LeftStick,
            Self::RightStick => SdlButton::RightStick,
            Self::LeftShoulder => SdlButton::LeftShoulder,
            Self::RightShoulder => SdlButton::RightShoulder,
            Self::DPadUp => SdlButton::DPadUp,
            Self::DPadDown => SdlButton::DPadDown,
            Self::DPadLeft => SdlButton::DPadLeft,
            Self::DPadRight => SdlButton::DPadRight,
            Self::Misc1 => SdlButton::Misc1,
            Self::RightPaddle1 => SdlButton::RightPaddle1,
            Self::LeftPaddle1 => SdlButton::LeftPaddle1,
            Self::RightPaddle2 => SdlButton::RightPaddle2,
            Self::LeftPaddle2 => SdlButton::LeftPaddle2,
            Self::Touchpad => SdlButton::Touchpad,
            Self::Misc2 => SdlButton::Misc2,
            Self::Misc3 => SdlButton::Misc3,
            Self::Misc4 => SdlButton::Misc4,
            Self::Misc5 => SdlButton::Misc5,
            Self::Misc6 => SdlButton::Misc6,
        }
    }
}

/// Compact set of pressed [`Button`] values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Buttons(u32);

impl Buttons {
    /// Returns the underlying bit set for serialization or diagnostics.
    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, button: Button) -> bool {
        self.0 & (1 << button as u8) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Button> {
        Button::ALL
            .into_iter()
            .filter(move |&button| self.contains(button))
    }

    fn insert(&mut self, button: Button) {
        self.0 |= 1 << button as u8;
    }
}

/// One polled controller sample, using SDL's unmodified signed 16-bit axes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerState {
    pub buttons: Buttons,
    pub left_stick: [i16; 2],
    pub right_stick: [i16; 2],
    pub triggers: [i16; 2],
}

/// Metadata for an open controller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerInfo {
    pub id: ControllerId,
    pub name: String,
    pub path: Option<String>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub player_index: Option<u16>,
}

/// Error returned while initializing, discovering, or opening SDL controllers.
#[derive(Debug, thiserror::Error)]
#[error("SDL controller input failed: {0}")]
pub struct ControllerError(String);

/// Owns SDL's gamepad subsystem and keeps all currently attached devices open.
///
/// Create and poll this value on the application's main thread. SDL permits only
/// one event pump per process. Use [`Self::with_sdl`] when the application already
/// dispatches SDL events, or [`Self::new`] for standalone controller polling.
pub struct ControllerHub {
    _sdl: Sdl,
    gamepads: GamepadSubsystem,
    events: Option<EventPump>,
    open: BTreeMap<ControllerId, Gamepad>,
}

impl ControllerHub {
    /// Initializes SDL gamepad input and enables its Nintendo GameCube adapter
    /// HIDAPI driver unless the user has overridden that SDL hint.
    ///
    /// Owns SDL's event pump and updates input state when refreshed. Events stay
    /// queued; dispatch them through [`Self::event_pump`] each loop iteration.
    pub fn new() -> Result<Self, ControllerError> {
        let sdl = sdl3::init().map_err(error)?;
        let mut hub = Self::with_sdl(&sdl)?;
        hub.events = Some(sdl.event_pump().map_err(error)?);
        hub.refresh()?;
        Ok(hub)
    }

    /// Shares an application's SDL context without acquiring its event pump.
    ///
    /// Enables the GameCube adapter hint before initializing the gamepad
    /// subsystem. The caller must pump and dispatch SDL events on the main
    /// thread before calling [`Self::refresh`] or [`Self::poll`]. Neither method
    /// pumps or consumes events in this mode, preserving window and menu events
    /// for the application's central event loop.
    pub fn with_sdl(sdl: &Sdl) -> Result<Self, ControllerError> {
        let _ = sdl3::hint::set("SDL_JOYSTICK_HIDAPI_GAMECUBE", "1");
        let gamepads = sdl.gamepad().map_err(error)?;
        let mut hub = Self {
            _sdl: sdl.clone(),
            gamepads,
            events: None,
            open: BTreeMap::new(),
        };
        hub.refresh()?;
        Ok(hub)
    }

    /// Returns the event pump owned by a standalone hub, or `None` when SDL is
    /// shared through [`Self::with_sdl`].
    ///
    /// Standalone loops should dispatch this pump's events each iteration so
    /// the queue does not accumulate keyboard, window, or controller events.
    pub fn event_pump(&mut self) -> Option<&mut EventPump> {
        self.events.as_mut()
    }

    /// Closes removed controllers and opens new ones without consuming events.
    ///
    /// A standalone hub also pumps SDL input state. For a hub created with
    /// [`Self::with_sdl`], pump and dispatch events in the application's event
    /// loop before refreshing.
    pub fn refresh(&mut self) -> Result<(), ControllerError> {
        if let Some(events) = &mut self.events {
            events.pump_events();
        }
        let attached: BTreeSet<_> = self
            .gamepads
            .gamepads()
            .map_err(error)?
            .into_iter()
            .map(|id| ControllerId(id.raw()))
            .collect();

        self.open
            .retain(|id, gamepad| attached.contains(id) && gamepad.connected());
        for id in attached {
            if !self.open.contains_key(&id) {
                let gamepad = self.gamepads.open(id.0.into()).map_err(error)?;
                self.open.insert(id, gamepad);
            }
        }
        Ok(())
    }

    /// Adds mappings in SDL's standard controller-database text format.
    pub fn load_mappings(&self, source: &mut impl std::io::Read) -> Result<usize, ControllerError> {
        let count = self
            .gamepads
            .load_mappings_from_read(source)
            .map_err(error)?;
        usize::try_from(count).map_err(error)
    }

    /// Returns metadata for every currently open controller, ordered by ID.
    pub fn controllers(&self) -> Vec<ControllerInfo> {
        self.open
            .iter()
            .map(|(&id, gamepad)| ControllerInfo {
                id,
                name: gamepad
                    .name()
                    .unwrap_or_else(|| "Unknown controller".into()),
                path: gamepad.path(),
                vendor_id: gamepad.vendor_id(),
                product_id: gamepad.product_id(),
                player_index: gamepad.player_index(),
            })
            .collect()
    }

    /// Polls the latest state for one open controller.
    pub fn state(&self, id: ControllerId) -> Option<ControllerState> {
        self.open.get(&id).map(sample)
    }

    /// Refreshes hot-plug state and samples every open controller.
    ///
    /// When using [`Self::with_sdl`], dispatch SDL events before sampling.
    pub fn poll(&mut self) -> Result<Vec<(ControllerInfo, ControllerState)>, ControllerError> {
        self.refresh()?;
        Ok(self
            .controllers()
            .into_iter()
            .filter_map(|info| self.state(info.id).map(|state| (info, state)))
            .collect())
    }
}

fn sample(gamepad: &Gamepad) -> ControllerState {
    let mut buttons = Buttons::default();
    for button in Button::ALL {
        if gamepad.button(button.sdl()) {
            buttons.insert(button);
        }
    }
    ControllerState {
        buttons,
        left_stick: [gamepad.axis(SdlAxis::LeftX), gamepad.axis(SdlAxis::LeftY)],
        right_stick: [gamepad.axis(SdlAxis::RightX), gamepad.axis(SdlAxis::RightY)],
        triggers: [
            gamepad.axis(SdlAxis::TriggerLeft),
            gamepad.axis(SdlAxis::TriggerRight),
        ],
    }
}

fn error(error: impl std::fmt::Display) -> ControllerError {
    ControllerError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_set_preserves_distinct_sdl_positions() {
        let mut buttons = Buttons::default();
        buttons.insert(Button::South);
        buttons.insert(Button::Misc4);

        assert!(buttons.contains(Button::South));
        assert!(buttons.contains(Button::Misc4));
        assert!(!buttons.contains(Button::East));
        assert_eq!(
            buttons.iter().collect::<Vec<_>>(),
            [Button::South, Button::Misc4]
        );
    }
}
