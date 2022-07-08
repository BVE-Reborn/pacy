use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use spin_sleep::SpinSleeper;

pub struct FramePacer {
    pub options: Options,
    internals: Internals,
    sleeper: SpinSleeper,
}
impl FramePacer {
    pub fn new() -> Self {
        Self {
            options: Options::default(),
            internals: Internals::default(),
            sleeper: SpinSleeper::default(),
        }
    }

    pub fn internals(&self) -> &Internals {
        &self.internals
    }

    pub fn start_frame(&mut self, vblank_interval: f32) {
        self.internals.cpu_input_time_history.reserve(1);

        let input_end = Instant::now();
        if let Some(current_cpu_input_start) = self.internals.current_cpu_input_start {
            let input_time = input_end - current_cpu_input_start;
            self.internals.cpu_input_time_history.push_back(input_time);
        }

        self.internals.monitor.vblank_interval = Duration::from_secs_f32(vblank_interval);
        self.internals.current_cpu_frame_start = Some(input_end);
    }

    pub fn end_frame(&mut self) -> Duration {
        let start = self.internals.current_cpu_frame_start.unwrap();
        let end = Instant::now();
        self.internals.current_cpu_frame_end = Some(end);

        let duration = end - start;
        self.internals.cpu_time_history.push_back(duration);

        let sleep_time = self
            .internals
            .monitor
            .vblank_interval
            .saturating_sub(duration);

        sleep_time
    }

    pub fn wait_for_frame(&mut self) {
        // Make sure we've allocated space _before_ we take the current measurement.
        self.internals.cpu_post_frame_time_history.reserve(1);
        self.internals.cpu_sleep_time_history.reserve(1);

        let start = self.internals.current_cpu_frame_start.take().unwrap();
        let wait_start = Instant::now();

        let after_frame_duration =
            wait_start - self.internals.current_cpu_frame_end.take().unwrap();
        self.internals
            .cpu_post_frame_time_history
            .push_back(after_frame_duration);

        let used_duration = wait_start - start;
        let mut sleep_time = self
            .internals
            .monitor
            .vblank_interval
            .saturating_sub(used_duration);

        // TODO this is carp
        if let Some(last_input_time) = self.internals.cpu_input_time_history.pop_back() {
            sleep_time = sleep_time.saturating_sub(last_input_time);
        }

        self.internals.cpu_sleep_time_history.push_back(sleep_time);
        if self.options.enabled {
            profiling::scope!("FramePacer::wait_for_frame");
            self.sleeper.sleep(sleep_time);
        }

        let input_start_time = Instant::now();
        self.internals.current_cpu_input_start = Some(input_start_time);
    }
}

pub struct Options {
    pub enabled: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Default)]
pub struct Internals {
    pub current_cpu_frame_start: Option<Instant>,
    pub current_cpu_sleep_start_time: Option<Duration>,
    pub current_cpu_frame_end: Option<Instant>,
    pub current_cpu_input_start: Option<Instant>,

    pub cpu_time_history: VecDeque<Duration>,
    pub cpu_post_frame_time_history: VecDeque<Duration>,
    pub cpu_sleep_time_history: VecDeque<Duration>,
    pub cpu_input_time_history: VecDeque<Duration>,

    pub monitor: Monitor,
}

pub struct Monitor {
    pub vblank_interval: Duration,
}
impl Default for Monitor {
    fn default() -> Self {
        Self {
            vblank_interval: Duration::from_secs_f32(1.0 / 60.0),
        }
    }
}
