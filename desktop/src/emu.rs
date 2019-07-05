use std::{
    mem,
    sync::{
        MutexGuard,
        atomic::Ordering,
    },
};

use spin_sleep::LoopHelper;

use mahboi::{
    Emulator, Disruption, SCREEN_WIDTH,
    env::Peripherals,
    primitives::PixelColor,
    machine::input::Keys,
};
use crate::{AtomicKeys, Message, Shared, TARGET_FPS};

/// Drives the emulation. The emulator writes into the `gb_buffer` back buffer.
/// Both of those buffers are swapped after each Gameboy frame. The emulator
/// additionally reads from `keys`. Lastly, if the emulator terminates in an
/// unusual fashion, a `Quit` message is send to the main thread.
pub(crate) fn emulator_thread(
    mut emulator: Emulator,
    shared: Shared,
) {
    /// This is what we pass to the emulator.
    struct DesktopPeripherals<'a> {
        back_buffer: MutexGuard<'a, Vec<(u8, u8, u8)>>,
        keys: &'a AtomicKeys,
    }

    impl Peripherals for DesktopPeripherals<'_> {
        fn get_pressed_keys(&self) -> Keys {
            self.keys.as_keys()
        }

        fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
            let start = line_idx as usize * SCREEN_WIDTH;
            let end = start + SCREEN_WIDTH;
            for (src, dst) in pixels.iter().zip(&mut self.back_buffer[start..end]) {
                *dst = (src.r, src.g, src.b);
            }
        }
    }

    // Run forever, until an error occurs.
    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.25)
        .build_with_target_rate(TARGET_FPS);

    loop {
        let target_rate = if shared.state.turbo_mode.load(Ordering::SeqCst) {
            shared.state.args.turbo_mode_factor * TARGET_FPS
        } else {
            TARGET_FPS
        };
        loop_helper.set_target_rate(target_rate);

        loop_helper.loop_start();

        // Lock the buffer for the whole emulation step.
        let back = shared.state.gb_screen.back.lock()
            .expect("[T-emu] failed to lock back buffer");

        // Run the emulator
        let mut peripherals = DesktopPeripherals {
            back_buffer: back,
            keys: &shared.state.keys,
        };
        let res = emulator.execute_frame(&mut peripherals, |_| false);

        // React to abnormal disruptions
        match res {
            Err(Disruption::Terminated) => {
                shared.messages.send(Message::Quit).unwrap();
                break;
            }

            // This means that the emulator reached V-Blank and we want to
            // present the buffer we just wrote to the actual display.
            Ok(true) => {
                // Swap both buffers
                {
                    let mut front = shared.state.gb_screen.front.lock()
                        .expect("[T-emu] failed to lock front buffer");
                    mem::swap(&mut *front, &mut *peripherals.back_buffer);
                }
            }
            _ => {}
        }

        // Release the lock as soon as possible.
        drop(peripherals.back_buffer);

        if let Some(fps) = loop_helper.report_rate() {
            *shared.state.emulation_rate.lock().unwrap() = fps;
        }

        loop_helper.loop_sleep();
    }
}
