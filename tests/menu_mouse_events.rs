use sdl3::{
    event::{Event, WindowEvent},
    hint::Hint,
    mouse::{MouseButton, MouseState},
};
use skirmish::{
    menu::{
        MenuEffect,
        melee::{MainItem, main_definition, main_interaction_map},
    },
    renderer::{
        menu_host::MenuHost,
        viewport::{MELEE_AUTHORED_EXTENT, PresentationTransform},
    },
};

fn motion(window_id: u32, point: [f32; 2]) -> Event {
    Event::MouseMotion {
        timestamp: 1,
        window_id,
        which: 0,
        mousestate: MouseState::from_sdl_state(0),
        x: point[0],
        y: point[1],
        xrel: 0.0,
        yrel: 0.0,
    }
}

fn button_down(window_id: u32, button: MouseButton, point: [f32; 2]) -> Event {
    Event::MouseButtonDown {
        timestamp: 2,
        window_id,
        which: 0,
        mouse_btn: button,
        clicks: 1,
        x: point[0],
        y: point[1],
    }
}

fn mouse_leave(window_id: u32) -> Event {
    Event::Window {
        timestamp: 3,
        window_id,
        win_event: WindowEvent::MouseLeave,
    }
}

fn dispatch_pending(
    events: &mut sdl3::EventPump,
    menu: &mut MenuHost,
    window_id: u32,
    focused: bool,
    transform: Option<PresentationTransform>,
) {
    for event in events.poll_iter() {
        menu.handle_sdl_pointer_event(&event, window_id, focused, transform);
    }
}

fn push_and_dispatch(
    events: &sdl3::EventSubsystem,
    pump: &mut sdl3::EventPump,
    menu: &mut MenuHost,
    window_id: u32,
    focused: bool,
    transform: Option<PresentationTransform>,
    event: Event,
) {
    events.push_event(event).expect("push mouse event");
    dispatch_pending(pump, menu, window_id, focused, transform);
}

// One SDL test in this integration-test process keeps SDL's main-thread and
// event-pump ownership isolated. The dummy driver supplies a real SDL window
// and event queue, but this intentionally does not claim compositor coverage.
#[test]
fn pushed_sdl_mouse_events_reach_the_canonical_menu_runtime() {
    assert!(sdl3::hint::set_with_priority(
        "SDL_VIDEO_DRIVER",
        "dummy",
        &Hint::Override,
    ));
    let sdl = sdl3::init().expect("initialize SDL");
    let video = sdl.video().expect("initialize SDL video");
    assert_eq!(video.current_video_driver(), "dummy");
    let window = video
        .window("Skirmish pointer test", 640, 480)
        .hidden()
        .build()
        .expect("create dummy SDL window");
    let window_id = window.id();
    let (window_width, window_height) = window.size();
    let (pixel_width, pixel_height) = window.size_in_pixels();
    let transform = PresentationTransform::new(
        [window_width, window_height],
        [pixel_width, pixel_height],
        MELEE_AUTHORED_EXTENT,
    );
    let events = sdl.event().expect("initialize SDL events");
    let mut pump = sdl.event_pump().expect("acquire SDL event pump");
    pump.poll_iter().for_each(drop);

    let mut menu =
        MenuHost::new(main_definition(), main_interaction_map(), []).expect("initialize Main menu");
    for _ in 0..20 {
        assert!(menu.tick(&[], true).is_empty());
    }
    let versus_center = [193.227_35, 195.221_6];

    for (focused, event) in [
        (
            false,
            button_down(window_id, MouseButton::Left, versus_center),
        ),
        (
            true,
            button_down(window_id.wrapping_add(1), MouseButton::Left, versus_center),
        ),
        (
            true,
            button_down(window_id, MouseButton::Right, versus_center),
        ),
    ] {
        push_and_dispatch(
            &events, &mut pump, &mut menu, window_id, focused, transform, event,
        );
        assert!(menu.tick(&[], true).is_empty());
        assert_eq!(
            menu.runtime().selected().id.as_str(),
            MainItem::OnePlayer.id()
        );
    }

    for event in [motion(window_id, versus_center), mouse_leave(window_id)] {
        events.push_event(event).expect("push mouse event");
    }
    dispatch_pending(&mut pump, &mut menu, window_id, true, transform);
    assert!(menu.tick(&[], true).is_empty());
    assert_eq!(
        menu.runtime().selected().id.as_str(),
        MainItem::OnePlayer.id()
    );

    for event in [
        motion(window_id, versus_center),
        button_down(window_id, MouseButton::Left, versus_center),
    ] {
        events.push_event(event).expect("push mouse event");
    }
    dispatch_pending(&mut pump, &mut menu, window_id, true, transform);

    assert!(matches!(
        menu.tick(&[], true).as_slice(),
        [
            MenuEffect::SelectionChanged { selected, .. },
            MenuEffect::ActionRequested {
                item: Some(item),
                action,
                ..
            },
        ] if selected.as_str() == MainItem::Versus.id()
            && item == selected
            && action.destination.as_str() == MainItem::Versus.destination()
    ));
}
