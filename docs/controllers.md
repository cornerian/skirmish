# Native controller input

The optional `controller-input` feature provides hot-plug controller discovery
and polling through SDL3:

```rust
use skirmish::controller::host::{Button, ControllerHub};

let mut controllers = ControllerHub::new()?;
for (info, state) in controllers.poll()? {
    println!("{}: {}", info.id.0, info.name);
    if state.buttons.contains(Button::South) {
        println!("primary face button pressed");
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Build it with `cargo build --features controller-input`. Native builds link to
an installed SDL3 library. SDL's gamepad API uses its controller mapping
database to normalize Xbox, PlayStation, Nintendo Switch, and other controllers
to physical button positions and six standard axes.

To print one sample from every attached controller, run:

```sh
cargo run --features controller-input --example controllers
```

The Nintendo GameCube USB adapter HIDAPI driver is enabled before initializing
the gamepad subsystem. SDL exposes an attached adapter's ports as GameCube gamepads;
its physical face-button mapping is South=A, East=X, West=B, and North=Y. The
analog triggers use the standard trigger axes, while GameCube L/R clicks are
available as Misc3/Misc4. Keep action bindings configurable rather than assuming
that every controller has the GameCube layout.

The direct-UI input adapter maps South to A, Start to Start, and either East or
West to B. Accepting both Back positions keeps standard-gamepad East and the
GameCube's physical B usable. The adapter only produces an HSD button word for
the menu runtime; it does not implement menu navigation.

Create and poll `ControllerHub` on the main thread. `ControllerHub::new()` owns
SDL's single process-wide event pump for standalone polling. It updates input
state without removing queued events. In a repeating standalone loop, dispatch
events from `controllers.event_pump().unwrap().poll_iter()` each iteration to
handle quit requests and keep the queue from accumulating. The one-sample
`controllers` example shows this dispatch; `event_pump()` returns `None` for a
hub created with `with_sdl`.

Applications with a window should share their SDL context through
`ControllerHub::with_sdl(&sdl)`. This constructor does not acquire an event pump;
the application owns the central loop and dispatches events before sampling
controllers. In this mode neither `refresh()` nor `poll()` pumps or consumes SDL
events, so keyboard, window, and quit events reach their handlers:

```rust
use sdl3::event::Event;
use skirmish::controller::host::ControllerHub;

let sdl = sdl3::init()?;
let mut events = sdl.event_pump()?;
let mut controllers = ControllerHub::with_sdl(&sdl)?;
'running: loop {
    for event in events.poll_iter() {
        match event {
            Event::Quit { .. } => break 'running,
            _ => { /* Dispatch keyboard, mouse, and window events here. */ }
        }
    }
    for (info, state) in controllers.poll()? {
        // Map this sample into the application's next fixed input tick.
        let _ = (info, state);
    }
    // Advance due simulation/menu ticks and render the next frame.
}
# Ok::<(), Box<dyn std::error::Error>>(())
```
