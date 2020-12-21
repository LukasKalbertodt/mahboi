//! Timing the host loop (usually fixed to the screen's refresh rate) with the
//! Gameboy emulation speed.

use std::time::{Duration, Instant};

use crate::{
    Outcome,
    args::Args,
};


/// How often the FPS are reported. Longer times lead to more delay and more
/// smoothing.
const REPORT_INTERVAL: Duration = Duration::from_millis(250);

/// Check `drive_emulation` for more details.
const SLACK_MULTIPLIER: f32 = 1.3;

pub(crate)  struct LoopTimer {
    /// The time an emulated frame should last. (This stays constant.)
    ideal_frame_time: Duration,

    /// The factor by which the `ideal_frame_time` is divided when the turbo
    /// mode is enabled. (This stays constant.)
    turbo_mode_factor: f64,

    /// The amount the emulation is behind of the ideal time.
    behind: Duration,

    /// The point in time when `should_emulate_frame` was last called. That
    /// method should be called once every frame on the host machine.
    last_host_frame: Option<Instant>,

    /// Whether the turbo mode is enabled.
    turbo: bool,

    // For FPS reporting
    last_report: Instant,
    frames_since_last_report: u32,
    behind_at_last_report: Duration,
}

impl LoopTimer {
    pub(crate) fn new(args: &Args) -> Self {
        let ideal_frame_time = Duration::from_secs(1).div_f64(args.fps);

        // This arbitrary 1.5 factor makes sure that in a typical 59.73
        // emulation FPS and 60 host FPS setting, the first "host frame no
        // emulation frame" doesn't happen at the very start. But it also makes
        // sure that there aren't two emulation frames in one host frame (which
        // would be factor 2).
        let behind = ideal_frame_time.mul_f32(1.5);

        Self {
            ideal_frame_time,
            turbo_mode_factor: args.turbo_mode_factor,
            turbo: false,
            last_host_frame: None,
            behind,
            last_report: Instant::now(),
            frames_since_last_report: 0,
            behind_at_last_report: behind,
        }
    }

    pub(crate) fn set_turbo_mode(&mut self, turbo: bool) {
        self.turbo = turbo;
    }

    // Tells the timer the emulation has just been unpaused. This will reset a
    // few values in the timer so that the timer doesn't think we just
    // experienced a huge lag.
    pub(crate) fn unpause(&mut self) {
        self.behind = self.ideal_frame_time.mul_f32(1.5);
        self.last_host_frame = None;
    }

    /// Call once per host frame and pass a closure that emulates one frame of
    /// the gameboy. This method will make sure that `emulate_frame` is called
    /// an appropriate number of times to keep the target frame rate.
    pub(crate) fn drive_emulation(
        &mut self,
        mut emulate_frame: impl FnMut() -> Outcome,
    ) -> Outcome {
        let now = Instant::now();
        if let Some(last_host_frame) = self.last_host_frame {
            self.behind += now - last_host_frame;
            // println!("{:.1?} -> {:.1?}", now - last_host_frame, self.behind);
        }
        self.last_host_frame = Some(now);

        // Obtain actual target frame time by applying turbo mode.
        let target_frame_time = self.target_frame_time();

        // If one emulation frame fits into the `behind` time, we emulate a
        // frame. After the first iteration we set `slack` to 1.3. This is done
        // to have some transition time/slack. Otherwise the following
        // problematic situation might arise quite often:
        // - The `behind` value gets smaller than `target_frame_time` (which it
        //   does regularly when using the original gameboy speed on a 60hz
        //   monitor).
        // - One host frame, there is no frame emulated to make up for that.
        // - The next host frame takes a bit longer, so that the previous
        //   `behind` value plus this slightly longer host frame result in a
        //   value slightly larger than two times `target_frame_time`.
        // - That results in two frames being emulated. But the next host frame
        //   is likely a bit shorter again, meaning that THEN no gameboy frame
        //   is emulated again.
        //
        // This can destabilize the game loop and lead to some juttery motion.
        let mut slack = 1.0;
        while self.behind > target_frame_time.mul_f32(slack) {
            self.behind -= target_frame_time;
            let outcome = emulate_frame();
            if outcome != Outcome::Continue {
                return outcome;
            }

            slack = SLACK_MULTIPLIER;
            self.frames_since_last_report += 1;
        }

        Outcome::Continue
    }

    /// Returns `Some(fps)` every `REPORT_INTERVAL`.
    pub(crate) fn report_fps(&mut self) -> Option<f64> {
        let elapsed = self.last_report.elapsed();
        if elapsed >= REPORT_INTERVAL {
            // The calculation is a bit more involved to avoid the reported FPS
            // fluctuating all over the place. That's because in the case of
            // original gameboy speed and 60hz monitor, every roughly 2s, a
            // gameboy frame is skipped. With a naive calculation of
            // `frames_since_last_report / relapsed`, the reported FPS would be
            // 60 most of the time and dip down to like 55 or so for a single
            // report. While this is "more correct", technically, I think it's
            // more useful for users to have the saved time in `behind` be
            // included in the report.
            //
            // So we check the difference between `behind` and the `behind`
            // value when the last report was made. That way we know whether we
            // "spent" or gained saved time compared to the last report.
            let saved_time = self.behind.as_secs_f64() - self.behind_at_last_report.as_secs_f64();
            let saved_frames = saved_time / self.target_frame_time().as_secs_f64();
            let fps = (self.frames_since_last_report as f64 + saved_frames)
                / elapsed.as_secs_f64();

            // Reset stuff
            self.behind_at_last_report = self.behind;
            self.last_report = Instant::now();
            self.frames_since_last_report = 0;

            Some(fps)
        } else {
            None
        }
    }

    fn target_frame_time(&self) -> Duration {
        if self.turbo {
            self.ideal_frame_time.div_f64(self.turbo_mode_factor)
        } else {
            self.ideal_frame_time
        }
    }
}
