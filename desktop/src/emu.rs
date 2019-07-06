use std::{
    mem,
    sync::{
        MutexGuard,
        atomic::Ordering,
    },
    time::{Duration, Instant},
};

use spin_sleep::LoopHelper;

use mahboi::{
    Emulator, Disruption, SCREEN_WIDTH,
    log::*,
    env::Peripherals,
    primitives::PixelColor,
    machine::input::Keys,
};
use crate::{AtomicKeys, DurationExt, Message, Shared, TARGET_FPS};

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

    // We start with a dummy value, assuming the host CPU is exactly as fast as
    // the Gameboy CPU.
    let mut required_emulation_time = Duration::from_micros((1_000_000.0 / TARGET_FPS) as u64);

    loop {
        let target_rate = if shared.state.turbo_mode.load(Ordering::SeqCst) {
            shared.state.args.turbo_mode_factor * TARGET_FPS
        } else {
            TARGET_FPS
        };
        loop_helper.set_target_rate(target_rate);

        loop_helper.loop_start();

        // We might want to sleep a bit to reduce input lag.
        let emu_delay = {
            use std::cmp::min;

            let render_timing = *shared.state.render_timing.lock().unwrap();
            let target_iteration_time = Duration::from_micros(
                (1_000_000.0 / loop_helper.target_rate()) as u64
            );

            // We have several factors which limit how long we can delay
            // emulation. We start with the user specified value for an upper
            // wait limit.
            let delay = shared.state.args.emu_max_delay;

            // TODO: make this buffer user specified
            let assumed_emulation_time = required_emulation_time + Duration::from_millis(1);

            // Next, we have to finish emulation before the OpenGL thread
            // starts drawing.
            let time_left_before_draw = render_timing.draw_delay
                .saturating_sub(render_timing.last_host_frame_start.elapsed())
                .saturating_sub(required_emulation_time);

            // Finally, if we emulate at a rate that is faster then the host
            // refresh rate, we also have to take care not to sleep when that
            // time is required for emulation. We take the time we have for
            // each emulation loop iteration and subtract the time we expect to
            // need for emulation.
            let time_left_for_iteration = target_iteration_time
                .saturating_sub(assumed_emulation_time);

            // Pick the minimum of all these three limits
            let delay = min(delay, time_left_before_draw);
            let delay = min(delay, time_left_for_iteration);

            delay
        };

        trace!("about to sleep for {:.2?} before emulating", emu_delay);
        spin_sleep::sleep(emu_delay);


        // Lock the buffer for the whole emulation step.
        let back = shared.state.gb_screen.back.lock()
            .expect("[T-emu] failed to lock back buffer");

        // Run the emulator for one frame
        let mut peripherals = DesktopPeripherals {
            back_buffer: back,
            keys: &shared.state.keys,
        };
        let before_emulation = Instant::now();
        let res = emulator.execute_frame(&mut peripherals, |_| false);
        required_emulation_time = {
            let frame_time = before_emulation.elapsed();
            let learn_rate = shared.state.args.emu_delay_learn_rate as f64;

            let new_delay = (1.0 - learn_rate) * required_emulation_time.as_nanos() as f64
                + learn_rate * frame_time.as_nanos() as f64;
            Duration::from_nanos(new_delay as u64)
        };


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
