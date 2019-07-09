use std::{
    sync::atomic::Ordering,
};

use winit::{
    ControlFlow, Event, KeyboardInput, WindowEvent,
    ElementState as State,
    VirtualKeyCode as Key,
};

use mahboi::{
    log::*,
    machine::input::JoypadKey,
};
use crate::Shared;



/// Listens for input events and handles them by either updating `keys` or
/// sending messages to the main thread.
pub(crate) fn handle_event(event: &Event, shared: &Shared) -> ControlFlow {
    // First, we extract the inner window event as that's what we are
    // interested in.
    let event = match event {
        // That's what we want!
        Event::WindowEvent { event, .. } => event,

        // When the main thread wakes us up, we just stop this thread.
        Event::Awakened => {
            if shared.should_quit.load(Ordering::SeqCst) {
                return ControlFlow::Break;
            } else {
                warn!("woke up event thread, but `should_quit` not set (that's strange)");
                return ControlFlow::Continue;
            }
        }

        // We ignore all other events (device events).
        _ => return ControlFlow::Continue,
    };

    // Now handle window events.
    match event {
        WindowEvent::CloseRequested | WindowEvent::Destroyed => {
            shared.request_quit();
            return ControlFlow::Break;
        }

        WindowEvent::Resized(new_size) => {
            *shared.window_size.lock().unwrap() = *new_size;
        }
        WindowEvent::HiDpiFactorChanged(new_dpi_factor) => {
            *shared.window_dpi_factor.lock().unwrap() = *new_dpi_factor;
        }


        // A key input that has a virtual keycode attached
        WindowEvent::KeyboardInput {
            input: KeyboardInput { virtual_keycode: Some(key), state, modifiers, .. },
            ..
        } => {
            let keys = &shared.keys;
            let state = *state;

            match key {
                // Button keys
                Key::M if state == State::Pressed => keys.set_key(JoypadKey::Start),
                Key::M if state == State::Released => keys.unset_key(JoypadKey::Start),
                Key::N if state == State::Pressed => keys.set_key(JoypadKey::Select),
                Key::N if state == State::Released => keys.unset_key(JoypadKey::Select),
                Key::J if state == State::Pressed => keys.set_key(JoypadKey::A),
                Key::J if state == State::Released => keys.unset_key(JoypadKey::A),
                Key::K if state == State::Pressed => keys.set_key(JoypadKey::B),
                Key::K if state == State::Released => keys.unset_key(JoypadKey::B),

                // Direction keys
                Key::W if state == State::Pressed => keys.set_key(JoypadKey::Up),
                Key::W if state == State::Released => keys.unset_key(JoypadKey::Up),
                Key::A if state == State::Pressed => keys.set_key(JoypadKey::Left),
                Key::A if state == State::Released => keys.unset_key(JoypadKey::Left),
                Key::S if state == State::Pressed => keys.set_key(JoypadKey::Down),
                Key::S if state == State::Released => keys.unset_key(JoypadKey::Down),
                Key::D if state == State::Pressed => keys.set_key(JoypadKey::Right),
                Key::D if state == State::Released => keys.unset_key(JoypadKey::Right),

                // Other non-Gameboy related functions
                Key::Q if state == State::Pressed && modifiers.ctrl => {
                    shared.request_quit();
                    return ControlFlow::Break;
                }

                Key::LShift => {
                    shared.turbo_mode.store(state == State::Pressed, Ordering::SeqCst);
                }

                _ => {}
            }
        }
        _ => {}
    }

    ControlFlow::Continue
}
