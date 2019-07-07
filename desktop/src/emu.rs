use std::{
    cmp::{max, min},
    mem,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use spin_sleep::LoopHelper;

use mahboi::{
    Emulator, Disruption, SCREEN_WIDTH, SCREEN_HEIGHT,
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
        back_buffer: &'a mut [(u8, u8, u8)],
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
        .report_interval_s(0.5)
        .build_without_target_rate();

    // We start with a dummy value, assuming the host CPU is exactly as fast as
    // the Gameboy CPU.
    let mut required_emulation_time = Duration::from_micros((1_000_000.0 / TARGET_FPS) as u64);

    // The emulator writes into this buffer. After one frame, this is swapped
    // with the front buffer accessible to the render thread.
    let mut back_buffer = vec![(0, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT];

    // This is the time the emulation in this loop should start according to
    // the standard rate.
    let mut regular = Instant::now();

    loop {
        loop_helper.loop_start();


        // ===== Run emulator ====================================================================

        // Run the emulator for one frame
        let mut peripherals = DesktopPeripherals {
            back_buffer: &mut back_buffer,
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
                    let mut frame = shared.state.gb_frame.lock()
                        .expect("failed to lock front buffer");
                    mem::swap(&mut frame.buffer, &mut back_buffer);
                    frame.timestamp = before_emulation;
                }
            }
            _ => {}
        }


        // ===== Sleep ===========================================================================

        let target_rate = if shared.state.turbo_mode.load(Ordering::SeqCst) {
            shared.state.args.turbo_mode_factor * TARGET_FPS
        } else {
            TARGET_FPS
        };
        let target_iteration_time = Duration::from_micros(
            (1_000_000.0 / target_rate) as u64
        );
        let next_regular = regular + target_iteration_time;

        // We might want to sleep a bit to reduce input lag.
        let emu_delay = {
            // The user can control the maximum deviation from the standard
            // tick rate.
            let max_deviation = shared.state.args.max_emu_deviation;

            // Emulation time can vary. Thus we add a margin of 0.5.
            let assumed_emulation_time = required_emulation_time + required_emulation_time / 2;

            // This visualizes the timing situation (not to scale, obviously):
            //
            //    o   <- `now`
            //    |
            //    |
            //    |
            //    |
            //    o   <- `earliest`: we are not allowed to start emulating
            //    |                  before this point
            //    |
            //    o   <- `regular`: following the normal emulation rate, here
            //    |                 is where we should emulate
            //    |
            //    o   <- `latest`: we are not allowed to start emulating after
            //    |                this point
            //    |
            //    |
            //    |
            //    o   <- `draw_time`: at this point, the render thread will
            //    |                   start drawing. If possible, we should
            //    |                   finish emulation before this point
            //    |
            //    o   <- `next_regular`: at this point the next emulation step
            //                           is scheduled. Thus, we need to finish
            //                           before.
            //

            let now = Instant::now();
            let earliest = max(now, regular - max_deviation);
            let latest = regular + max_deviation;
            let earliest_finish = earliest + assumed_emulation_time;

            // Figure out the next draw time that we can manage to finish
            // emulation before. The loop won't iterate very often, as
            // `next_draw_start` is updated by the render thread and is fairly
            // close to the current time.
            let next_draw_time = {
                let render_timing = *shared.state.render_timing.lock().unwrap();
                let mut draw_time = render_timing.next_draw_start;

                while draw_time < earliest_finish {
                    draw_time += render_timing.frame_time;
                }

                draw_time
            };

            // We need to be finished by the time the render thread starts
            // drawing. But we also need to finish before the next `regular`
            // instant of emulation to not waste time sleeping that is required
            // for emulation.
            let need_to_be_finished_by = min(next_regular, next_draw_time);

            // We want to start such that we finish right before the point
            // where we need to be finished. But we must not start after
            // `latest`.
            let sleep_till = min(latest, need_to_be_finished_by - assumed_emulation_time);
            sleep_till.saturating_sub(now)
        };

        trace!("about to sleep for {:.2?} before emulating", emu_delay);
        spin_sleep::sleep(emu_delay);


        if let Some(fps) = loop_helper.report_rate() {
            *shared.state.emulation_rate.lock().unwrap() = fps;
        }

        // If our emulation cannot keep up (with high turbo mode factors), we
        // don't want the `regular` time to fall much behind. Otherwise when
        // disabling turbo mode, we need to catch up, resulting in turbo mode
        // speed for some time.
        regular = max(Instant::now(), next_regular);
    }
}
