//! Reusable SDL window orchestration for scene previews and menu presentation.
//!
//! The executable remains the composition root: it chooses resources, opens
//! SDL devices, and injects destination ownership. This module owns the shared
//! event, fixed-tick, and presentation loop so alternative and modded hosts do
//! not need to duplicate input-ordering details.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use sdl3::{
    EventPump,
    event::{Event, WindowEvent},
    keyboard::Scancode,
};

use crate::{
    controller::host::{ControllerHub, ControllerInfo, ControllerState},
    menu::{DestinationId, MenuEffect, MenuEntry, MenuId},
    presentation::AnimationPlayback,
};

use super::{
    audio::AudioOutput,
    clock::FixedStepClock,
    gpu::WindowRenderer,
    menu_host::MenuHost,
    viewport::{MELEE_AUTHORED_EXTENT, PresentationTransform},
};

/// Application-owned lookup for internal menu destinations.
///
/// The emitted action's cooldown is forwarded unchanged so the resolver can
/// preserve a source-global menu timer across screen handoffs.
pub type DestinationResolver = Box<dyn FnMut(&MenuId, &DestinationId, u16) -> Option<MenuEntry>>;

/// State required while the window is hosting a declarative menu.
pub struct MenuWindow {
    controllers: Option<ControllerHub>,
    menu: MenuHost,
    presentation: AnimationPlayback,
    resolve_destination: DestinationResolver,
}

impl MenuWindow {
    pub fn new(
        menu: MenuHost,
        controllers: Option<ControllerHub>,
        resolve_destination: impl FnMut(&MenuId, &DestinationId, u16) -> Option<MenuEntry> + 'static,
    ) -> Result<Self> {
        let presentation =
            AnimationPlayback::new(menu.runtime().selected().presentation.animation.clone())
                .context("initializing menu presentation")?;
        Ok(Self {
            controllers,
            menu,
            presentation,
            resolve_destination: Box::new(resolve_destination),
        })
    }

    fn enter_internal_menu(
        &mut self,
        destination: &DestinationId,
        inherited_cooldown_frames: u16,
        transform: Option<PresentationTransform>,
    ) -> Result<bool> {
        let current = self.menu.runtime().definition().id.clone();
        let Some(entry) =
            (self.resolve_destination)(&current, destination, inherited_cooldown_frames)
        else {
            return Ok(false);
        };

        let cue = entry
            .definition
            .items
            .iter()
            .find(|item| item.id == entry.definition.default_item)
            .context("resolved menu has no default presentation")?
            .presentation
            .animation
            .clone();
        let next_playback =
            AnimationPlayback::new(cue).context("initializing destination menu presentation")?;

        self.menu
            .enter_menu(entry.definition, entry.interaction, [], transform)
            .with_context(|| format!("entering internal menu {}", destination.as_str()))?;
        self.presentation = next_playback;
        println!(
            "Melee menu entered: {} (selected {})",
            self.menu.runtime().definition().id.as_str(),
            self.menu.runtime().selected().id.as_str()
        );
        Ok(true)
    }

    fn apply_effects(
        &mut self,
        effects: Vec<MenuEffect>,
        transform: Option<PresentationTransform>,
    ) -> Result<()> {
        for effect in effects {
            match effect {
                MenuEffect::SelectionChanged {
                    selected,
                    sound,
                    presentation,
                    ..
                } => {
                    self.presentation
                        .restart(presentation.animation)
                        .context("requesting selected menu presentation")?;
                    println!(
                        "Melee menu selection: {}{}",
                        selected.as_str(),
                        sound.map_or_else(String::new, |sound| format!(" (sound {})", sound.0))
                    );
                }
                MenuEffect::ActionRequested {
                    trigger,
                    item,
                    action,
                } => {
                    if let Some(transition) = action.transition.clone() {
                        self.presentation
                            .restart(transition)
                            .context("requesting menu transition presentation")?;
                    }
                    println!(
                        "Melee menu action: {trigger:?}{} -> {}{}",
                        item.map_or_else(String::new, |item| format!(" on {}", item.as_str())),
                        action.destination.as_str(),
                        action
                            .sound
                            .map_or_else(String::new, |sound| format!(" (sound {})", sound.0))
                    );
                    self.enter_internal_menu(
                        &action.destination,
                        action.cooldown_frames,
                        transform,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn poll_controllers(&mut self) -> Result<Vec<(ControllerInfo, ControllerState)>> {
        if let Some(controllers) = &mut self.controllers {
            controllers.poll().context("polling menu controllers")
        } else {
            Ok(Vec::new())
        }
    }
}

/// The behavior mode hosted by an SDL window.
pub enum WindowMode {
    Preview,
    Menu(Box<MenuWindow>),
}

/// Owns the SDL event loop and one renderer window.
pub struct WindowHost {
    renderer: WindowRenderer,
    audio: Option<AudioOutput>,
    mode: WindowMode,
    orbit: [f32; 3],
    focused: bool,
    visible: bool,
    quit: bool,
    dirty: bool,
    frame_clock: FixedStepClock,
    presented_frames: u64,
    frame_limit: Option<u64>,
}

impl WindowHost {
    pub fn new(
        renderer: WindowRenderer,
        audio: Option<AudioOutput>,
        mode: WindowMode,
        frame_limit: Option<u64>,
    ) -> Self {
        Self {
            renderer,
            audio,
            mode,
            orbit: [0.0, 0.0, 1.0],
            focused: true,
            visible: true,
            quit: false,
            dirty: true,
            frame_clock: FixedStepClock::new(Instant::now()),
            presented_frames: 0,
            frame_limit,
        }
    }

    fn menu_mut(&mut self) -> Option<&mut MenuWindow> {
        match &mut self.mode {
            WindowMode::Preview => None,
            WindowMode::Menu(menu) => Some(menu),
        }
    }

    fn has_menu(&self) -> bool {
        matches!(self.mode, WindowMode::Menu(_))
    }

    fn event(&mut self, event: Event) {
        let window_id = self.renderer.window_id();
        let pointer_transform = if event.is_mouse() {
            self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT)
        } else {
            None
        };
        let focused = self.focused;
        if let Some(menu) = self.menu_mut()
            && menu
                .menu
                .handle_sdl_pointer_event(&event, window_id, focused, pointer_transform)
        {
            return;
        }

        match event {
            Event::Quit { .. } | Event::AppTerminating { .. } => self.quit = true,
            Event::Window {
                window_id,
                win_event,
                ..
            } if window_id == self.renderer.window_id() => match win_event {
                WindowEvent::CloseRequested => self.quit = true,
                WindowEvent::FocusLost => {
                    self.focused = false;
                    if let Some(menu) = self.menu_mut() {
                        menu.menu.clear_input();
                    }
                }
                WindowEvent::FocusGained => self.focused = true,
                // Occlusion can follow an Exposed event during a Wayland
                // resize, whose requested frame must still be presented.
                WindowEvent::Hidden | WindowEvent::Minimized => {
                    self.visible = false;
                    if let Some(menu) = self.menu_mut() {
                        menu.menu.clear_input();
                    }
                }
                WindowEvent::Shown
                | WindowEvent::Restored
                | WindowEvent::Maximized
                | WindowEvent::Exposed => {
                    let resumed = !self.visible;
                    self.visible = true;
                    self.dirty = true;
                    if resumed {
                        self.frame_clock.rebase(Instant::now());
                    }
                }
                WindowEvent::Resized(_, _)
                | WindowEvent::PixelSizeChanged(_, _)
                | WindowEvent::DisplayChanged(_) => {
                    let (width, height) = self.renderer.pixel_size();
                    self.renderer.resize(width, height);
                    let transform = self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT);
                    if let Some(menu) = self.menu_mut() {
                        menu.menu.pointer_reproject(transform);
                    }
                    self.dirty = true;
                }
                _ => {}
            },
            Event::KeyDown {
                window_id,
                scancode: Some(code),
                repeat,
                ..
            } if window_id == self.renderer.window_id() && self.focused => {
                if !repeat && code == Scancode::Q {
                    self.quit = true;
                } else if let Some(menu) = self.menu_mut() {
                    menu.menu.key(code, true, repeat);
                } else {
                    self.scene_key(code, repeat);
                }
            }
            Event::KeyUp {
                window_id,
                scancode: Some(code),
                repeat,
                ..
            } if window_id == self.renderer.window_id() => {
                if let Some(menu) = self.menu_mut() {
                    menu.menu.key(code, false, repeat);
                }
            }
            _ => {}
        }
    }

    fn scene_key(&mut self, code: Scancode, repeat: bool) {
        match code {
            Scancode::Escape => self.quit = true,
            Scancode::Left => self.orbit[0] -= 0.1,
            Scancode::Right => self.orbit[0] += 0.1,
            Scancode::Up => self.orbit[1] = (self.orbit[1] + 0.1).min(1.2),
            Scancode::Down => self.orbit[1] = (self.orbit[1] - 0.1).max(-1.2),
            Scancode::Equals | Scancode::KpPlus => self.orbit[2] = (self.orbit[2] * 0.9).max(0.2),
            Scancode::Minus | Scancode::KpMinus => self.orbit[2] = (self.orbit[2] * 1.1).min(5.0),
            Scancode::R => self.orbit = [0.0, 0.0, 1.0],
            Scancode::Space if !repeat => {
                self.cue();
                return;
            }
            _ => return,
        }
        self.dirty = true;
    }

    fn cue(&mut self) {
        if let Some(audio) = &mut self.audio {
            let _ = audio.play_cue();
        }
    }

    fn check_audio(&mut self) {
        if let Some(audio) = &self.audio
            && audio.error_count() > 0
        {
            eprintln!("warning: audio stream failed; disabling audio");
            self.audio = None;
        }
    }

    fn drawable(&self) -> bool {
        let (width, height) = self.renderer.pixel_size();
        self.visible && width > 0 && height > 0
    }

    pub fn run(mut self, mut events: EventPump) -> Result<()> {
        let mut pending = None;
        while !self.quit {
            // This is the sole event consumer. The event returned by waiting is
            // dispatched too, rather than being lost before the next poll.
            if let Some(event) = pending.take() {
                self.event(event);
            }
            for event in events.poll_iter() {
                self.event(event);
            }
            if self.quit {
                break;
            }
            let now = Instant::now();
            self.check_audio();
            let frame_due = self.drawable() && self.frame_clock.is_due(now);
            if frame_due {
                let controller_samples = match &mut self.mode {
                    WindowMode::Preview => Vec::new(),
                    WindowMode::Menu(menu) => menu.poll_controllers()?,
                };
                let due_ticks = self.frame_clock.consume_due(now);
                for tick in 0..due_ticks {
                    let transform = self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT);
                    let focused = self.focused;
                    if let WindowMode::Menu(menu) = &mut self.mode {
                        let effects = if tick + 1 == due_ticks {
                            menu.menu.tick(&controller_samples, focused)
                        } else {
                            menu.menu.tick_previous()
                        };
                        menu.apply_effects(effects, transform)?;
                        menu.presentation.tick();
                        // Every fixed menu tick also advances the authored
                        // presentation clock, including catch-up ticks.
                        self.dirty = true;
                    }
                }
            }
            if frame_due && (self.dirty || self.frame_limit.is_some() || self.has_menu()) {
                let [yaw, pitch, zoom] = self.orbit;
                let presented = self
                    .renderer
                    .render(yaw, pitch, zoom)
                    .context("rendering an SDL frame")?;
                self.dirty = !presented;
                if presented {
                    self.presented_frames += 1;
                    if self
                        .frame_limit
                        .is_some_and(|limit| self.presented_frames >= limit)
                    {
                        println!("Presented {} frames.", self.presented_frames);
                        break;
                    }
                }
            }
            let mut wait = Duration::from_millis(250);
            if self.drawable() && (self.dirty || self.frame_limit.is_some() || self.has_menu()) {
                wait = wait.min(self.frame_clock.time_until_next(Instant::now()));
            }
            // SDL waits in integer milliseconds. Round up to avoid busy polling
            // for the fractional millisecond at the end of a 60 Hz tick.
            pending =
                events.wait_event_timeout_ms(wait.as_nanos().div_ceil(1_000_000).min(250) as u32);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use sdl3::keyboard::Scancode;

    use super::*;
    use crate::menu::melee::{
        MAIN_MENU_ID, MAIN_SELECTION_ANIMATION_ID, MainItem, VERSUS_MENU_ID,
        VERSUS_SELECTION_ANIMATION_ID, main_definition_with_default, main_interaction_map,
        resolve_internal_destination,
    };

    fn identity_transform() -> Option<PresentationTransform> {
        PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT)
    }

    fn exhaust_cooldown(menu: &mut MenuWindow) {
        while menu.menu.runtime().cooldown_frames() > 0 {
            let effects = menu.menu.tick(&[], true);
            menu.apply_effects(effects, identity_transform()).unwrap();
            menu.presentation.tick();
        }
    }

    #[test]
    fn destination_resolver_receives_exact_cooldown_and_enters_menu() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&calls);
        let host = MenuHost::new(
            main_definition_with_default(MainItem::Versus),
            main_interaction_map(),
            [],
        )
        .unwrap();
        let mut menu = MenuWindow::new(host, None, move |current, destination, cooldown| {
            recorded.borrow_mut().push((
                current.as_str().to_owned(),
                destination.as_str().to_owned(),
                cooldown,
            ));
            resolve_internal_destination(current, destination, cooldown)
        })
        .unwrap();
        exhaust_cooldown(&mut menu);

        menu.menu.key(Scancode::Return, true, false);
        let effects = menu.menu.tick(&[], true);
        menu.apply_effects(effects, identity_transform()).unwrap();
        menu.presentation.tick();

        assert_eq!(
            calls.borrow().as_slice(),
            &[(MAIN_MENU_ID.into(), VERSUS_MENU_ID.into(), 5)]
        );
        assert_eq!(menu.menu.runtime().definition().id.as_str(), VERSUS_MENU_ID);
        assert_eq!(
            menu.presentation.animation_id().as_str(),
            VERSUS_SELECTION_ANIMATION_ID
        );
        assert_eq!(
            menu.presentation.frame(),
            menu.menu
                .runtime()
                .selected()
                .presentation
                .animation
                .frames
                .start
        );
    }

    #[test]
    fn unresolved_destination_keeps_the_current_menu() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&calls);
        let host = MenuHost::new(
            main_definition_with_default(MainItem::OnePlayer),
            main_interaction_map(),
            [],
        )
        .unwrap();
        let mut menu = MenuWindow::new(host, None, move |current, destination, cooldown| {
            recorded.borrow_mut().push((
                current.as_str().to_owned(),
                destination.as_str().to_owned(),
                cooldown,
            ));
            None
        })
        .unwrap();
        exhaust_cooldown(&mut menu);

        menu.menu.key(Scancode::Return, true, false);
        let effects = menu.menu.tick(&[], true);
        menu.apply_effects(effects, identity_transform()).unwrap();

        assert_eq!(menu.menu.runtime().definition().id.as_str(), MAIN_MENU_ID);
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(calls.borrow()[0].2, 5);
    }

    #[test]
    fn selection_effect_restarts_presentation_at_selected_frame() {
        let host = MenuHost::new(
            main_definition_with_default(MainItem::OnePlayer),
            main_interaction_map(),
            [],
        )
        .unwrap();
        let mut menu = MenuWindow::new(host, None, |_, _, _| None).unwrap();
        exhaust_cooldown(&mut menu);
        assert_eq!(
            menu.presentation.animation_id().as_str(),
            MAIN_SELECTION_ANIMATION_ID
        );

        menu.menu.key(Scancode::Down, true, false);
        let effects = menu.menu.tick(&[], true);
        menu.apply_effects(effects, identity_transform()).unwrap();

        assert_eq!(
            menu.presentation.animation_id().as_str(),
            MAIN_SELECTION_ANIMATION_ID
        );
        assert_eq!(
            menu.presentation.frame(),
            MainItem::Versus.selection_frames().start
        );
    }
}
