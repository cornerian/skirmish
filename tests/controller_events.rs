#![cfg(feature = "controller-input")]

use sdl3::{
    event::Event,
    keyboard::{Keycode, Mod, Scancode},
};
use skirmish::controller::host::ControllerHub;

fn push_application_events(events: &sdl3::EventSubsystem) {
    events
        .push_event(Event::KeyDown {
            timestamp: 1,
            window_id: 0,
            keycode: Some(Keycode::Return),
            scancode: Some(Scancode::Return),
            keymod: Mod::NOMOD,
            repeat: false,
            which: 0,
            raw: 0,
        })
        .expect("push menu key event");
    events
        .push_event(Event::Quit { timestamp: 2 })
        .expect("push quit event");
}

fn assert_application_events(events: &mut sdl3::EventPump) {
    let pending: Vec<_> = events.poll_iter().collect();
    assert!(
        pending.iter().any(|event| matches!(
            event,
            Event::KeyDown {
                keycode: Some(Keycode::Return),
                ..
            }
        )),
        "controller polling consumed the menu key event: {pending:?}"
    );
    assert!(
        pending
            .iter()
            .any(|event| matches!(event, Event::Quit { .. })),
        "controller polling consumed the quit event: {pending:?}"
    );
}

// One SDL test per integration-test process keeps its main-thread and event-pump
// ownership requirements intact. No display or physical controller is needed.
#[test]
fn controller_refresh_preserves_application_events() {
    let sdl = sdl3::init().expect("initialize SDL");
    let events = sdl.event().expect("initialize SDL events");
    {
        let mut pump = sdl.event_pump().expect("acquire application event pump");
        let mut controllers = ControllerHub::with_sdl(&sdl)
            .expect("share SDL with an existing application event pump");
        assert!(controllers.event_pump().is_none());

        push_application_events(&events);
        controllers.refresh().expect("refresh shared controllers");
        assert_application_events(&mut pump);

        push_application_events(&events);
        controllers.poll().expect("sample shared controllers");
        assert_application_events(&mut pump);
    }

    {
        let mut controllers = ControllerHub::new().expect("initialize standalone controllers");
        push_application_events(&events);
        controllers
            .refresh()
            .expect("refresh standalone controllers");
        controllers.poll().expect("sample standalone controllers");
        assert_application_events(
            controllers
                .event_pump()
                .expect("standalone controller pump is available for dispatch"),
        );
    }
    sdl.event_pump().expect("reacquire application event pump");
}
