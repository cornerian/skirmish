use sdl3::event::Event;
use skirmish::controller::host::ControllerHub;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut controllers = ControllerHub::new()?;
    if let Some(events) = controllers.event_pump() {
        for event in events.poll_iter() {
            if matches!(event, Event::Quit { .. }) {
                return Ok(());
            }
        }
    }
    let samples = controllers.poll()?;

    if samples.is_empty() {
        println!("No controllers detected");
    }
    for (info, state) in samples {
        println!(
            "{}: {} (VID={:04x?}, PID={:04x?}) sticks={:?}/{:?} triggers={:?} buttons={:#010x}",
            info.id.0,
            info.name,
            info.vendor_id,
            info.product_id,
            state.left_stick,
            state.right_stick,
            state.triggers,
            state.buttons.bits(),
        );
    }
    Ok(())
}
