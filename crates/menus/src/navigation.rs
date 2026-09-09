use crate::input::{BACK, CONFIRM, DOWN, UP};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The ten branch menus whose input callbacks are implemented in `mnmain.c`.
/// Numeric discriminants preserve `melee/mn/forward.h`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Menu {
    Main = 0,
    OnePlayer = 1,
    Versus = 2,
    Trophies = 3,
    Options = 4,
    Data = 5,
    RegularMatch = 6,
    Stadium = 9,
    SpecialVersus = 12,
    Records = 28,
}

/// Save-data predicates supplied by the caller; no unlocks are invented.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Unlocks {
    pub all_star: bool,
    pub sound_test: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    pub index: u16,
    pub label: &'static str,
}

const fn entry(index: u16, label: &'static str) -> Entry {
    Entry { index, label }
}

impl Menu {
    pub const ALL: [Self; 10] = [
        Self::Main,
        Self::OnePlayer,
        Self::Versus,
        Self::Trophies,
        Self::Options,
        Self::Data,
        Self::RegularMatch,
        Self::Stadium,
        Self::SpecialVersus,
        Self::Records,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Main => "Main Menu",
            Self::OnePlayer => "1-P Mode",
            Self::Versus => "VS. Mode",
            Self::Trophies => "Trophies",
            Self::Options => "Options",
            Self::Data => "Data",
            Self::RegularMatch => "Regular Match",
            Self::Stadium => "Stadium",
            Self::SpecialVersus => "Special Melee",
            Self::Records => "Records",
        }
    }

    /// Entries retain their original indices; permanently hidden slots are omitted.
    /// All-Star and Sound Test remain listed so callers can display their lock state.
    pub const fn entries(self) -> &'static [Entry] {
        match self {
            Self::Main => {
                const {
                    &[
                        entry(0, "1-P Mode"),
                        entry(1, "VS. Mode"),
                        entry(2, "Trophies"),
                        entry(3, "Options"),
                        entry(4, "Data"),
                    ]
                }
            }
            Self::OnePlayer => {
                const {
                    &[
                        entry(0, "Regular Match"),
                        entry(1, "Event Match"),
                        entry(3, "Stadium"),
                        entry(4, "Training"),
                    ]
                }
            }
            Self::Versus => {
                const {
                    &[
                        entry(0, "Melee"),
                        entry(1, "Tournament Melee"),
                        entry(2, "Special Melee"),
                        entry(3, "Custom Rules"),
                        entry(4, "Name Entry"),
                    ]
                }
            }
            Self::Trophies => {
                const {
                    &[
                        entry(0, "Gallery"),
                        entry(1, "Lottery"),
                        entry(3, "Collection"),
                    ]
                }
            }
            Self::Options => {
                const {
                    &[
                        entry(0, "Rumble"),
                        entry(1, "Sound"),
                        entry(2, "Screen Display"),
                        entry(4, "Language"),
                        entry(5, "Erase Data"),
                    ]
                }
            }
            Self::Data => {
                const {
                    &[
                        entry(0, "Snapshots"),
                        entry(1, "Archives"),
                        entry(2, "Sound Test"),
                        entry(3, "Records"),
                        entry(4, "Special"),
                    ]
                }
            }
            Self::RegularMatch => {
                const {
                    &[
                        entry(0, "Classic"),
                        entry(1, "Adventure"),
                        entry(2, "All-Star"),
                    ]
                }
            }
            Self::Stadium => {
                const {
                    &[
                        entry(0, "Target Test"),
                        entry(1, "Home-Run Contest"),
                        entry(2, "Multi-Man Melee"),
                    ]
                }
            }
            Self::SpecialVersus => {
                const {
                    &[
                        entry(0, "Camera Mode"),
                        entry(1, "Stamina Mode"),
                        entry(2, "Super Sudden Death"),
                        entry(3, "Giant Melee"),
                        entry(4, "Tiny Melee"),
                        entry(5, "Invisible Melee"),
                        entry(6, "Fixed-Camera Mode"),
                        entry(7, "Single-Button Mode"),
                        entry(8, "Lightning Melee"),
                        entry(9, "Slo-Mo Melee"),
                    ]
                }
            }
            Self::Records => {
                const {
                    &[
                        entry(0, "VS. Records"),
                        entry(1, "Bonus Records"),
                        entry(2, "Misc. Records"),
                    ]
                }
            }
        }
    }

    /// Original table count, including permanently hidden selections.
    pub const fn selection_count(self) -> u16 {
        match self {
            Self::Main | Self::OnePlayer | Self::Versus | Self::Data => 5,
            Self::Trophies => 4,
            Self::Options => 6,
            Self::RegularMatch | Self::Stadium | Self::Records => 3,
            Self::SpecialVersus => 10,
        }
    }

    /// `mn_80229938`: true means available, despite the misleading C comment.
    /// Like that predicate, this does not check whether the index exists.
    pub const fn is_available(self, selection: u16, unlocks: Unlocks) -> bool {
        match (self, selection) {
            (Self::RegularMatch, 2) => unlocks.all_star,
            (Self::Data, 2) => unlocks.sound_test,
            (Self::Options, 3) | (Self::OnePlayer | Self::Trophies, 2) => false,
            _ => true,
        }
    }

    /// `mn_80229A04`: count available indices strictly before `selection`.
    pub fn available_before(self, selection: u16, unlocks: Unlocks) -> u16 {
        (0..selection)
            .filter(|&index| self.is_available(index, unlocks))
            .count() as u16
    }

    fn parent(self) -> Option<(Self, u16)> {
        match self {
            Self::Main => None,
            Self::OnePlayer => Some((Self::Main, 0)),
            Self::Versus => Some((Self::Main, 1)),
            Self::Trophies => Some((Self::Main, 2)),
            Self::Options => Some((Self::Main, 3)),
            Self::Data => Some((Self::Main, 4)),
            Self::RegularMatch => Some((Self::OnePlayer, 0)),
            Self::Stadium => Some((Self::OnePlayer, 3)),
            Self::SpecialVersus => Some((Self::Versus, 2)),
            Self::Records => Some((Self::Data, 3)),
        }
    }

    fn destination(self, selection: u16) -> Target {
        use Destination::{Panel as P, Scene as S};
        use Target::{Branch, Leaf};
        match (self, selection) {
            (Self::Main, 0) => Branch(Self::OnePlayer),
            (Self::Main, 1) => Branch(Self::Versus),
            (Self::Main, 2) => Branch(Self::Trophies),
            (Self::Main, 3) => Branch(Self::Options),
            (Self::Main, 4) => Branch(Self::Data),
            (Self::OnePlayer, 0) => Branch(Self::RegularMatch),
            (Self::OnePlayer, 1) => Leaf(P(Panel::EventMatch)),
            (Self::OnePlayer, 3) => Branch(Self::Stadium),
            (Self::OnePlayer, 4) => Leaf(S(Scene::Training)),
            (Self::Versus, 0) => Leaf(S(Scene::Versus)),
            (Self::Versus, 1) => Leaf(S(Scene::Tournament)),
            (Self::Versus, 2) => Branch(Self::SpecialVersus),
            (Self::Versus, 3) => Leaf(P(Panel::Rules)),
            (Self::Versus, 4) => Leaf(P(Panel::NameEntry)),
            (Self::Trophies, 0) => Leaf(S(Scene::TrophyGallery)),
            (Self::Trophies, 1) => Leaf(S(Scene::TrophyLottery)),
            (Self::Trophies, 3) => Leaf(S(Scene::TrophyCollection)),
            (Self::Options, 0) => Leaf(P(Panel::Rumble)),
            (Self::Options, 1) => Leaf(P(Panel::Sound)),
            (Self::Options, 2) => Leaf(P(Panel::Display)),
            (Self::Options, 4) => Leaf(P(Panel::Language)),
            (Self::Options, 5) => Leaf(P(Panel::EraseData)),
            (Self::Data, 0) => Leaf(P(Panel::Snapshots)),
            (Self::Data, 1) => Leaf(P(Panel::Archives)),
            (Self::Data, 2) => Leaf(P(Panel::SoundTest)),
            (Self::Data, 3) => Branch(Self::Records),
            (Self::Data, 4) => Leaf(P(Panel::Special)),
            (Self::RegularMatch, 0) => Leaf(S(Scene::Classic)),
            (Self::RegularMatch, 1) => Leaf(S(Scene::Adventure)),
            (Self::RegularMatch, 2) => Leaf(S(Scene::AllStar)),
            (Self::Stadium, 0) => Leaf(S(Scene::TargetTest)),
            (Self::Stadium, 1) => Leaf(S(Scene::HomeRunContest)),
            (Self::Stadium, 2) => Leaf(P(Panel::MultiMan)),
            (Self::SpecialVersus, 0) => Leaf(S(Scene::CameraMode)),
            (Self::SpecialVersus, 1) => Leaf(S(Scene::Stamina)),
            (Self::SpecialVersus, 2) => Leaf(S(Scene::SuperSuddenDeath)),
            (Self::SpecialVersus, 3) => Leaf(S(Scene::Giant)),
            (Self::SpecialVersus, 4) => Leaf(S(Scene::Tiny)),
            (Self::SpecialVersus, 5) => Leaf(S(Scene::Invisible)),
            (Self::SpecialVersus, 6) => Leaf(S(Scene::FixedCamera)),
            (Self::SpecialVersus, 7) => Leaf(S(Scene::SingleButton)),
            (Self::SpecialVersus, 8) => Leaf(S(Scene::Lightning)),
            (Self::SpecialVersus, 9) => Leaf(S(Scene::SlowMotion)),
            (Self::Records, 0) => Leaf(P(Panel::VersusRecords)),
            (Self::Records, 1) => Leaf(P(Panel::BonusRecords)),
            (Self::Records, 2) => Leaf(P(Panel::MiscRecords)),
            _ => unreachable!("MenuState maintains an available selection"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Scene {
    Title = 0,
    Versus = 2,
    Classic = 3,
    Adventure = 4,
    AllStar = 5,
    CameraMode = 10,
    TrophyGallery = 11,
    TrophyLottery = 12,
    TrophyCollection = 13,
    TargetTest = 15,
    SuperSuddenDeath = 16,
    Invisible = 17,
    SlowMotion = 18,
    Lightning = 19,
    Tournament = 27,
    Training = 28,
    Tiny = 29,
    Giant = 30,
    Stamina = 31,
    HomeRunContest = 32,
    FixedCamera = 42,
    SingleButton = 44,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Panel {
    EventMatch,
    Rules,
    NameEntry,
    Rumble,
    Sound,
    Display,
    Language,
    EraseData,
    Snapshots,
    Archives,
    SoundTest,
    Special,
    MultiMan,
    VersusRecords,
    BonusRecords,
    MiscRecords,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Scene(Scene),
    Panel(Panel),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Moved,
    Entered(Menu),
    Returned(Menu),
    Requested(Destination),
}

enum Target {
    Branch(Menu),
    Leaf(Destination),
}

/// Serializable observation; deserializing it cannot bypass state validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub menu: Menu,
    pub previous_menu: Menu,
    pub selection: u16,
    pub buttons: u32,
    pub entering_menu: bool,
    pub cooldown: u16,
    pub controller_port: u8,
    pub unlocks: Unlocks,
    pub pending: Option<Destination>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MenuState {
    state: Snapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidSelection {
    pub menu: Menu,
    pub selection: u16,
}

impl fmt::Display for InvalidSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selection {} is unavailable in {}",
            self.selection,
            self.menu.title()
        )
    }
}

impl std::error::Error for InvalidSelection {}

impl MenuState {
    /// Main menu initialization with the original 20 input-poll entry cooldown.
    pub fn new(unlocks: Unlocks) -> Self {
        let mut menu =
            Self::at(Menu::Main, 0, unlocks).expect("main selection is always available");
        menu.state.cooldown = 20;
        menu
    }

    /// Construct a ready menu at an available original selection index.
    /// The caller has completed its entry transition; no cooldown is imposed.
    pub fn at(menu: Menu, selection: u16, unlocks: Unlocks) -> Result<Self, InvalidSelection> {
        if selection >= menu.selection_count() || !menu.is_available(selection, unlocks) {
            return Err(InvalidSelection { menu, selection });
        }
        Ok(Self {
            state: Snapshot {
                menu,
                previous_menu: menu,
                selection,
                buttons: 0,
                entering_menu: false,
                cooldown: 0,
                controller_port: 0,
                unlocks,
                pending: None,
            },
        })
    }

    pub const fn snapshot(&self) -> Snapshot {
        self.state
    }

    /// Process translated menu input, using port zero for single-player selection.
    pub fn step(&mut self, buttons: u32) -> Option<Action> {
        self.step_for_port(buttons, 0)
    }

    /// Process one input poll. Confirm wins over Back, Up, and Down, in that order.
    /// `confirming_port` is the first port that triggered A/Start, or zero if none.
    ///
    /// # Panics
    /// Panics when `confirming_port` is outside the four controller ports.
    pub fn step_for_port(&mut self, buttons: u32, confirming_port: u8) -> Option<Action> {
        assert!(
            confirming_port < 4,
            "controller port must be between 0 and 3"
        );
        if self.state.pending.is_some() {
            return None;
        }
        self.state.buttons = 0;
        if self.state.cooldown != 0 {
            self.state.cooldown -= 1;
            return None;
        }
        self.state.buttons = buttons;
        if buttons & CONFIRM != 0 {
            self.state.entering_menu = true;
            // One-player and Data set cooldown only in their branch cases.
            if !matches!(self.state.menu, Menu::OnePlayer | Menu::Data) {
                self.state.cooldown = 5;
            }
            if matches!(
                self.state.menu,
                Menu::OnePlayer | Menu::RegularMatch | Menu::Trophies
            ) || (self.state.menu == Menu::Stadium && self.state.selection < 2)
            {
                self.state.controller_port = confirming_port;
            }
            match self.state.menu.destination(self.state.selection) {
                Target::Branch(menu) => {
                    self.transition(menu, 0);
                    Some(Action::Entered(menu))
                }
                Target::Leaf(destination) => Some(self.request(destination)),
            }
        } else if buttons & BACK != 0 {
            self.state.entering_menu = false;
            self.state.cooldown = 5;
            match self.state.menu.parent() {
                Some((menu, selection)) => {
                    self.transition(menu, selection);
                    Some(Action::Returned(menu))
                }
                None => Some(self.request(Destination::Scene(Scene::Title))),
            }
        } else if buttons & (UP | DOWN) != 0 {
            let count = self.state.menu.selection_count();
            loop {
                self.state.selection = if buttons & UP != 0 {
                    (self.state.selection + count - 1) % count
                } else {
                    (self.state.selection + 1) % count
                };
                if self
                    .state
                    .menu
                    .is_available(self.state.selection, self.state.unlocks)
                {
                    break;
                }
            }
            Some(Action::Moved)
        } else {
            None
        }
    }

    fn transition(&mut self, menu: Menu, selection: u16) {
        self.state.previous_menu = self.state.menu;
        self.state.menu = menu;
        self.state.selection = selection;
        self.state.cooldown = 5;
    }

    fn request(&mut self, destination: Destination) -> Action {
        self.state.pending = Some(destination);
        Action::Requested(destination)
    }

    /// Native adapter boundary: return from a requested leaf to its originating
    /// branch and selection. This does not simulate or complete that leaf.
    /// The five-poll cooldown follows `mn_80229894`'s branch-return transition.
    pub fn resume(&mut self) -> bool {
        if self.state.pending.take().is_none() {
            return false;
        }
        self.state.cooldown = 5;
        self.state.buttons = 0;
        self.state.entering_menu = false;
        true
    }
}
