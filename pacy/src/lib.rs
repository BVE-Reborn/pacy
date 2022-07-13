use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use spin_sleep::SpinSleeper;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FrameStage(pub usize);

pub struct FramePacer {
    pub options: Options,
    internals: Internals,
    sleeper: SpinSleeper,
}
impl FramePacer {
    pub fn new(reported_frequency: f32) -> Self {
        Self {
            options: Options::default(),
            internals: Internals::new(Monitor::new(reported_frequency)),
            sleeper: SpinSleeper::default(),
        }
    }

    pub fn create_frame_stage(&mut self) -> FrameStage {
        let id = self.internals.frame_stages.len();
        self.internals.frame_stages.push(FrameStageStats::default());
        FrameStage(id)
    }

    pub fn set_monitor_frequency(&mut self, frequency: f32) {
        self.internals.monitor.reported_frequency = frequency;
    }

    pub fn internals(&self) -> &Internals {
        &self.internals
    }

    pub fn begin_frame_stage(&mut self, stage_id: FrameStage) {
        self.internals.frame_stages[stage_id.0].begin();
    }

    pub fn end_frame_stage(&mut self, stage_id: FrameStage) {
        self.internals.frame_stages[stage_id.0].end();
    }

    pub fn wait_for_frame(&mut self) {
        let next_frame_pipeline_duration: Duration = self
            .internals
            .frame_stages
            .iter()
            .map(FrameStageStats::estimate_time_for_completion)
            .sum(); // TODO

        let sleep_duration = self
            .internals
            .monitor
            .duration_until_next_hittable_timestamp(next_frame_pipeline_duration);

        self.internals.sleep_history.push_back(sleep_duration);

        self.sleeper.sleep(sleep_duration);
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

pub struct Internals {
    pub frame_stages: Vec<FrameStageStats>,

    pub sleep_history: VecDeque<Duration>,

    pub monitor: Monitor,
}
impl Internals {
    pub fn new(monitor: Monitor) -> Self {
        Self {
            frame_stages: Vec::new(),
            sleep_history: VecDeque::new(),
            monitor,
        }
    }
}

#[derive(Default)]
pub struct FrameStageStats {
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,

    pub duration_history: VecDeque<Duration>,
}
impl FrameStageStats {
    fn begin(&mut self) {
        self.start_time = Some(Instant::now());
    }

    fn end(&mut self) {
        self.duration_history.reserve(1);
        self.end_time = Some(Instant::now());
        self.duration_history
            .push_back(self.end_time.unwrap() - self.start_time.unwrap());
    }

    pub fn estimate_time_for_completion(&self) -> Duration {
        *self
            .duration_history
            .iter()
            .rev()
            .take(10)
            .max()
            .unwrap_or(&Duration::from_secs(0))
    }
}

pub struct Monitor {
    pub reported_frequency: f32,

    pub last_reported_timestamp: Instant,
}
impl Monitor {
    fn new(reported_frequency: f32) -> Self {
        Self {
            reported_frequency,
            last_reported_timestamp: Instant::now(),
        }
    }

    pub fn duration_until_next_hittable_timestamp(&self, compute_time: Duration) -> Duration {
        // TODO: improve the precision of this calculation
        let actual_vblank_secs = ((self.reported_frequency + 0.5).floor() - 0.5).recip();
        let actual_vblank_nanos = (1_000_000_000.0 * actual_vblank_secs) as u128;

        let now = Instant::now();

        let compute_finished = now + compute_time;
        let dur_since_timestamp = compute_finished - self.last_reported_timestamp;

        let nanos_into_frame = dur_since_timestamp.as_nanos() % actual_vblank_nanos;
        let nanos_remaining = actual_vblank_nanos - nanos_into_frame;

        Duration::from_nanos(nanos_remaining as u64)
    }
}
