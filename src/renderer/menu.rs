//! Presentation adapter for the translated menu state machine.
//!
//! The host supplies four HSD button words once per 60 Hz tick. This module
//! preserves the translated cooldowns, repeat behavior, and destination requests.

use menus::controller::Controllers;
use menus::input::{self, BACK};
use menus::{Action, Destination, Menu, MenuState, Scene, Snapshot, Unlocks};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuEvent {
    Navigation(Action),
    Resumed,
    QuitRequested,
    ImportAssetsRequested,
}

#[derive(Clone, Debug)]
pub struct MenuSession {
    state: MenuState,
    controllers: Controllers,
    asset_entry: bool,
    asset_selected: bool,
}

impl Default for MenuSession {
    fn default() -> Self {
        Self::new(Unlocks::default())
    }
}

impl MenuSession {
    pub fn new(unlocks: Unlocks) -> Self {
        Self::from_state(MenuState::new(unlocks))
    }

    pub fn from_state(state: MenuState) -> Self {
        Self {
            state,
            controllers: Controllers::default(),
            asset_entry: false,
            asset_selected: false,
        }
    }

    /// Add a native host utility without changing the translated game menus.
    pub fn enable_asset_import(&mut self) {
        self.asset_entry = true;
    }

    /// Advance one simulation tick, independently of the number of rendered frames.
    /// Confirm and Back remain edge triggered; held directions use native repeats.
    pub fn tick(&mut self, held: [u32; 4]) -> Option<MenuEvent> {
        let frames = self.controllers.poll(held);
        let buttons = input::decode(input::aggregate(&frames));
        let state = self.state.snapshot();
        if self.asset_entry
            && state.menu == Menu::Main
            && state.pending.is_none()
            && state.cooldown == 0
        {
            if self.asset_selected {
                if buttons & input::CONFIRM != 0 {
                    return Some(MenuEvent::ImportAssetsRequested);
                }
                if buttons & (input::UP | input::DOWN | BACK) != 0 {
                    let selection = if buttons & input::UP != 0 { 4 } else { 0 };
                    self.state = MenuState::at(Menu::Main, selection, state.unlocks).ok()?;
                    self.asset_selected = false;
                    return Some(MenuEvent::Navigation(Action::Moved));
                }
                return None;
            }
            if buttons & (input::CONFIRM | BACK) == 0
                && ((state.selection == 4
                    && buttons & input::DOWN != 0
                    && buttons & input::UP == 0)
                    || (state.selection == 0 && buttons & input::UP != 0))
            {
                self.asset_selected = true;
                return Some(MenuEvent::Navigation(Action::Moved));
            }
        }
        if self.state.snapshot().pending.is_some() {
            return (buttons & BACK != 0 && self.resume()).then_some(MenuEvent::Resumed);
        }
        self.state
            .step_for_port(buttons, input::confirming_port(&frames))
            .map(|action| match action {
                Action::Requested(Destination::Scene(Scene::Title)) => MenuEvent::QuitRequested,
                action => MenuEvent::Navigation(action),
            })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.snapshot()
    }

    /// Discard held input after focus loss or a presentation-mode change.
    /// This records releases without advancing cooldowns or consuming actions.
    pub fn release_input(&mut self) {
        self.controllers.poll([0; 4]);
    }

    /// Synchronize input when returning from a host screen without executing
    /// that screen's closing button in the game menu.
    pub fn synchronize_input(&mut self, held: [u32; 4]) {
        self.controllers.poll(held);
    }

    /// Return from a leaf while retaining controller history, preventing held
    /// Confirm or Back from becoming a new press on the originating branch.
    pub fn resume(&mut self) -> bool {
        self.state.resume()
    }

    pub fn view(&self) -> MenuView {
        let mut view = MenuView::from(self.snapshot());
        if self.asset_entry && view.menu == Menu::Main && view.pending.is_none() {
            if self.asset_selected {
                for row in &mut view.rows {
                    row.selected = false;
                }
                view.selected_label = "Import Game Assets";
            }
            view.rows.push(MenuRow {
                index: 5,
                label: "Import Game Assets",
                selected: self.asset_selected,
            });
        }
        view
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuRow {
    pub index: u16,
    pub label: &'static str,
    pub selected: bool,
}

/// Read-only presentation data; no menu selection or unlock can be mutated here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuView {
    pub menu: Menu,
    pub title: &'static str,
    pub breadcrumb: &'static str,
    pub rows: Vec<MenuRow>,
    pub pending: Option<Destination>,
    pub selected_label: &'static str,
    pub status: Option<MenuStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuStatus {
    pub lines: Vec<String>,
    pub percent: Option<u8>,
}

impl From<Snapshot> for MenuView {
    fn from(state: Snapshot) -> Self {
        let rows: Vec<_> = state
            .menu
            .entries()
            .iter()
            .filter(|entry| state.menu.is_available(entry.index, state.unlocks))
            .map(|entry| MenuRow {
                index: entry.index,
                label: entry.label,
                selected: entry.index == state.selection,
            })
            .collect();
        let selected_label = rows
            .iter()
            .find(|row| row.selected)
            .map_or("", |row| row.label);
        let breadcrumb = match state.menu {
            Menu::Main => "HOME",
            Menu::OnePlayer => "HOME / 1-P MODE",
            Menu::Versus => "HOME / VS. MODE",
            Menu::Trophies => "HOME / TROPHIES",
            Menu::Options => "HOME / OPTIONS",
            Menu::Data => "HOME / DATA",
            Menu::RegularMatch => "HOME / 1-P MODE / REGULAR MATCH",
            Menu::Stadium => "HOME / 1-P MODE / STADIUM",
            Menu::SpecialVersus => "HOME / VS. MODE / SPECIAL MELEE",
            Menu::Records => "HOME / DATA / RECORDS",
        };
        Self {
            menu: state.menu,
            title: state.menu.title(),
            breadcrumb,
            rows,
            pending: state.pending,
            selected_label,
            status: None,
        }
    }
}

/// Integer 60 Hz wall-clock accumulator for presentation hosts.
///
/// A stall may catch up at most eight ticks. Excess whole ticks are discarded
/// so a suspended window cannot spend seconds replaying stale held input. The
/// fractional tick is retained; ordinary schedules do not lose time to rounding.
#[derive(Clone, Debug, Default)]
pub struct FixedMenuClock {
    fraction: u128,
}

impl FixedMenuClock {
    pub fn advance(&mut self, elapsed: Duration) -> usize {
        let scaled = self.fraction + elapsed.as_nanos() * 60;
        self.fraction = scaled % 1_000_000_000;
        (scaled / 1_000_000_000).min(8) as usize
    }

    pub fn until_next_tick(&self) -> Duration {
        Duration::from_nanos((1_000_000_000 - self.fraction).div_ceil(60) as u64)
    }

    pub fn reset(&mut self) {
        self.fraction = 0;
    }
}
