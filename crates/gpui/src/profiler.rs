use itertools::Itertools;
use scheduler::{Instant, SpawnTime};
use std::{
    cell::LazyCell,
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
    time::Duration,
};

mod actions;
pub use actions::{ActionStatistics, ActionTiming, take_action_stats};
pub(crate) use actions::{save_action_timing, update_running_action};

use serde::{Deserialize, Serialize};

use crate::{SharedString, TasksIncluded, WindowId};

#[cfg(feature = "profiler")]
#[doc(hidden)]
pub fn get_all_timings(included: gpui::TasksIncluded) -> Vec<gpui::ThreadTaskTimings> {
    ThreadTaskTimings::collect(upgraded_thread_timings(), included)
}

#[cfg(feature = "profiler")]
#[doc(hidden)]
pub fn get_current_thread_timings(included: TasksIncluded) -> gpui::ThreadTaskTimings {
    gpui::profiler::get_current_thread_task_timings(included)
}

#[cfg(feature = "profiler")]
#[doc(hidden)]
pub fn take_all_stats(included: TasksIncluded) -> Vec<gpui::ThreadTaskStatistics> {
    ThreadTaskStatistics::collect_and_reset(upgraded_thread_timings(), included)
}

#[cfg(not(feature = "profiler"))]
#[doc(hidden)]
pub fn get_all_timings(_included: gpui::TasksIncluded) -> Vec<gpui::ThreadTaskTimings> {
    Vec::new()
}
#[cfg(not(feature = "profiler"))]
#[doc(hidden)]
pub fn get_current_thread_timings(_included: TasksIncluded) -> gpui::ThreadTaskTimings {
    gpui::ThreadTaskTimings {
        thread_name: None,
        thread_id: std::thread::current().id(),
        timings: Vec::new(),
        stats: TaskStatistics::default(),
        total_pushed: 0,
    }
}
#[cfg(not(feature = "profiler"))]
#[doc(hidden)]
pub fn take_all_stats(_included: TasksIncluded) -> Vec<gpui::ThreadTaskStatistics> {
    Vec::new()
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct YieldTime(pub Instant);

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct TaskTiming {
    pub location: &'static core::panic::Location<'static>,
    pub spawned: SpawnTime,
    pub start: Instant,
    pub end: YieldTime,
}

impl std::fmt::Debug for TaskTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskTiming")
            .field("location", &self.location)
            .field("since_spawned", &self.spawned.0.elapsed())
            .field("last_poll_duration", &self.poll_duration())
            .field("total_runtime", &self.since_spawn())
            .finish()
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct ActiveTiming {
    pub location: &'static core::panic::Location<'static>,
    pub spawned: SpawnTime,
    pub start: Instant,
}

impl TaskTiming {
    /// A task timing with a duration of zero. Any task will replace this in history.
    pub fn placeholder() -> Self {
        let now = Instant::now();
        Self {
            location: std::panic::Location::caller(),
            spawned: SpawnTime(now),
            start: now,
            end: YieldTime(now),
        }
    }

    #[inline(always)]
    pub fn poll_duration(&self) -> Duration {
        self.end.0 - self.start
    }

    #[inline(always)]
    fn since_spawn(&self) -> Duration {
        self.end.0 - self.spawned.0
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTaskTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: Vec<TaskTiming>,
    pub stats: TaskStatistics,
    pub total_pushed: u64,
}

impl ThreadTaskTimings {
    /// Convert upgraded per-thread timings into their structured format.
    pub fn collect(
        timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        included: TasksIncluded,
    ) -> Vec<Self> {
        timings
            .into_iter()
            .map(|(thread_id, timings)| {
                let timings = timings.lock();
                let thread_name = timings.thread_name.clone();
                let total_pushed = timings.total_pushed;
                let completed = &timings.timings;

                let mut vec = Vec::with_capacity(completed.len() + 1); // +1 for running task
                let (s1, s2) = completed.as_slices();
                vec.extend_from_slice(s1);
                vec.extend_from_slice(s2);
                if let TasksIncluded::CompletedAndRunning = included
                    && let Some(running) = timings.running
                {
                    vec.push(TaskTiming {
                        location: running.location,
                        spawned: running.spawned,
                        start: running.start,
                        end: YieldTime(Instant::now()),
                    })
                }

                ThreadTaskTimings {
                    thread_name,
                    thread_id,
                    timings: vec,
                    stats: timings.stats.clone(),
                    total_pushed,
                }
            })
            .collect()
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct ThreadTaskStatistics {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub stats: TaskStatistics,
}

impl ThreadTaskStatistics {
    pub fn collect_and_reset(
        timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        include_running: TasksIncluded,
    ) -> Vec<Self> {
        timings
            .into_iter()
            .map(|(thread_id, timings)| {
                let mut timings = timings.lock();
                let thread_name = timings.thread_name.clone();

                let mut stats = std::mem::take(&mut timings.stats);
                if let TasksIncluded::CompletedAndRunning = include_running
                    && let Some(ActiveTiming {
                        location,
                        spawned,
                        start,
                    }) = timings.running
                {
                    let end = YieldTime(Instant::now());
                    let timing = TaskTiming {
                        location,
                        spawned,
                        start,
                        end,
                    };
                    stats.add_runtime(timing);
                    stats.add_yield_timing(timing);
                }

                Self {
                    thread_name,
                    thread_id,
                    stats,
                }
            })
            .collect()
    }
}

/// Serializable variant of [`core::panic::Location`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedLocation {
    /// Name of the source file
    pub file: SharedString,
    /// Line in the source file
    pub line: u32,
    /// Column in the source file
    pub column: u32,
}

impl From<&core::panic::Location<'static>> for SerializedLocation {
    fn from(value: &core::panic::Location<'static>) -> Self {
        SerializedLocation {
            file: value.file().into(),
            line: value.line(),
            column: value.column(),
        }
    }
}

/// Serializable variant of [`TaskTiming`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTaskTiming {
    /// Location of the timing
    pub location: SerializedLocation,
    /// Time at which the measurement was reported in nanoseconds
    pub start: u128,
    /// Duration of the measurement in nanoseconds
    pub duration: u128,
}

impl SerializedTaskTiming {
    /// Convert an array of [`TaskTiming`] into their serializable format
    ///
    /// # Params
    ///
    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn convert(anchor: Instant, timings: &[TaskTiming]) -> Vec<SerializedTaskTiming> {
        timings
            .iter()
            .map(|timing| {
                let start = timing.start.duration_since(anchor).as_nanos();
                let duration = timing.end.0.duration_since(timing.start).as_nanos();
                SerializedTaskTiming {
                    location: timing.location.into(),
                    start,
                    duration,
                }
            })
            .collect::<Vec<_>>()
    }

    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn from(anchor: Instant, timing: TaskTiming) -> SerializedTaskTiming {
        let start = timing.start.duration_since(anchor).as_nanos();
        let duration = timing.end.0.duration_since(timing.start).as_nanos();
        SerializedTaskTiming {
            location: timing.location.into(),
            start,
            duration,
        }
    }
}

/// Serializable variant of [`ThreadTaskTimings`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedThreadTaskTimings {
    /// Thread name
    pub thread_name: Option<String>,
    /// Hash of the thread id
    pub thread_id: u64,
    /// Timing records for this thread
    pub timings: Vec<SerializedTaskTiming>,
}

impl SerializedThreadTaskTimings {
    /// Convert [`ThreadTaskTimings`] into their serializable format
    ///
    /// # Params
    ///
    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn convert(anchor: Instant, timings: ThreadTaskTimings) -> SerializedThreadTaskTimings {
        let serialized_timings = SerializedTaskTiming::convert(anchor, &timings.timings);

        let mut hasher = DefaultHasher::new();
        timings.thread_id.hash(&mut hasher);
        let thread_id = hasher.finish();

        SerializedThreadTaskTimings {
            thread_name: timings.thread_name,
            thread_id,
            timings: serialized_timings,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTimingsDelta {
    /// Hashed thread id
    pub thread_id: u64,
    /// Thread name, if known
    pub thread_name: Option<String>,
    /// New timings since the last call. If the circular buffer wrapped around
    /// since the previous poll, some entries may have been lost.
    pub new_timings: Vec<SerializedTaskTiming>,
}

/// Tracks which timing events have already been seen so that callers can request only unseen events.
#[doc(hidden)]
pub struct ProfilingCollector {
    startup_time: Instant,
    cursors: HashMap<ThreadId, u64>,
}

impl ProfilingCollector {
    pub fn new(startup_time: Instant) -> Self {
        Self {
            startup_time,
            cursors: HashMap::default(),
        }
    }

    pub fn startup_time(&self) -> Instant {
        self.startup_time
    }

    pub fn collect_unseen(
        &mut self,
        all_timings: Vec<ThreadTaskTimings>,
    ) -> Vec<ThreadTimingsDelta> {
        let mut deltas = Vec::with_capacity(all_timings.len());

        for thread in all_timings {
            let mut hasher = DefaultHasher::new();
            thread.thread_id.hash(&mut hasher);
            let hashed_id = hasher.finish();

            let prev_cursor = self.cursors.get(&thread.thread_id).copied().unwrap_or(0);
            let buffer_len = thread.timings.len() as u64;
            let buffer_start = thread.total_pushed.saturating_sub(buffer_len);

            let mut slice = if prev_cursor < buffer_start {
                // Cursor fell behind the buffer — some entries were evicted.
                // Return everything still in the buffer.
                thread.timings.as_slice()
            } else {
                let skip = (prev_cursor - buffer_start) as usize;
                &thread.timings[skip.min(thread.timings.len())..]
            };

            let cursor_advance = thread.total_pushed;
            self.cursors.insert(thread.thread_id, cursor_advance);

            if slice.is_empty() {
                continue;
            }

            let new_timings = SerializedTaskTiming::convert(self.startup_time, slice);

            deltas.push(ThreadTimingsDelta {
                thread_id: hashed_id,
                thread_name: thread.thread_name,
                new_timings,
            });
        }

        deltas
    }

    pub fn reset(&mut self) {
        self.cursors.clear();
    }
}

// Allow 16MiB of task timing entries.
// VecDeque grows by doubling its capacity when full, so keep this a power of 2 to avoid wasting
// memory.
#[cfg(feature = "profiler")]
const MAX_TASK_TIMINGS: usize = (16 * 1024 * 1024) / core::mem::size_of::<TaskTiming>();

#[doc(hidden)]
pub(crate) type TaskTimings = VecDeque<TaskTiming>;

#[doc(hidden)]
pub type GuardedTaskTimings = spin::Mutex<ThreadTimings>;

#[doc(hidden)]
pub struct GlobalThreadTimings {
    pub thread_id: ThreadId,
    pub timings: std::sync::Weak<GuardedTaskTimings>,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct TaskStatistics {
    pub poll_time_to_beat: Duration,
    pub runtime_to_beat: Duration,
    pub longest_poll_times: [TaskTiming; 5],
    pub longest_runtimes: [TaskTiming; 5],
}

impl std::fmt::Display for TaskStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tasks that blocked the longest before yielding\n")?;
        for timing in self.longest_poll_times {
            f.write_fmt(format_args!(
                "{:<20} - {}:{}\n",
                format!("{:?}", timing.poll_duration()),
                timing.location.file(),
                timing.location.column()
            ))?;
        }
        f.write_str("Tasks that ran the longest\n")?;
        for timing in self.longest_runtimes {
            f.write_fmt(format_args!(
                "{:<20} - {}:{}\n",
                format!("{:?}", timing.since_spawn()),
                timing.location.file(),
                timing.location.column()
            ))?;
        }
        Ok(())
    }
}

impl Default for TaskStatistics {
    fn default() -> Self {
        Self {
            // Do not track polls that are not problematic
            // this keeps more calls on the fast path
            poll_time_to_beat: Duration::from_micros(100),
            runtime_to_beat: Duration::from_micros(100),
            longest_poll_times: [TaskTiming::placeholder(); 5],
            longest_runtimes: [TaskTiming::placeholder(); 5],
        }
    }
}

impl TaskStatistics {
    #[inline(always)]
    fn add_yield_timing(&mut self, task: TaskTiming) {
        let yielded_after = task.poll_duration();
        if yielded_after >= self.poll_time_to_beat {
            std::hint::cold_path(); // most tasks are not the worst, optimize for that
            let to_replace = self
                .longest_poll_times
                .iter()
                .position_min_by_key(|task| task.since_spawn())
                .expect("guarded by the comparison with nth_longest_yield_time");
            self.longest_poll_times[to_replace] = task;

            self.poll_time_to_beat = self
                .longest_poll_times
                .iter()
                .map(|task| task.since_spawn())
                .min()
                .expect("never empty");
        }
    }

    #[inline(always)]
    fn add_runtime(&mut self, task: TaskTiming) {
        let runtime = task.since_spawn();
        if runtime >= self.runtime_to_beat {
            std::hint::cold_path(); // most tasks are not the worst, optimize for that
            let to_replace = self
                .longest_runtimes
                .iter()
                .position_min_by_key(|task| task.since_spawn())
                .expect("guarded by the comparison with nth_longest_yield_time");
            self.longest_runtimes[to_replace] = task;

            self.runtime_to_beat = self
                .longest_runtimes
                .iter()
                .map(|task| task.since_spawn())
                .min()
                .expect("never empty");
        }
    }
}

#[doc(hidden)]
pub static GLOBAL_THREAD_TIMINGS: spin::Mutex<Vec<GlobalThreadTimings>> =
    spin::Mutex::new(Vec::new());

/// Upgrades all live per-thread timing handles, holding the global registry
/// lock only for the duration of the upgrades.
///
/// The upgraded `Arc`s must never be dropped while `GLOBAL_THREAD_TIMINGS` is
/// locked: dropping the last strong reference runs [`ThreadTimings::drop`],
/// which locks `GLOBAL_THREAD_TIMINGS` again and would deadlock the
/// non-reentrant spinlock. A thread exiting concurrently can hand off its last
/// reference to us at any time, so callers of this function process (lock,
/// read, drop) the returned handles only after the global lock is released.
fn upgraded_thread_timings() -> Vec<(ThreadId, Arc<GuardedTaskTimings>)> {
    let global_thread_timings = GLOBAL_THREAD_TIMINGS.lock();
    global_thread_timings
        .iter()
        .filter_map(|t| Some((t.thread_id, t.timings.upgrade()?)))
        .collect()
}

thread_local! {
    #[doc(hidden)]
    pub static THREAD_TIMINGS: LazyCell<Arc<GuardedTaskTimings>> = LazyCell::new(|| {
        let current_thread = std::thread::current();
        let thread_name = current_thread.name();
        let thread_id = current_thread.id();
        let timings = ThreadTimings::new(thread_name.map(|e| e.to_string()), thread_id);
        let timings = Arc::new(spin::Mutex::new(timings));

        {
            let timings = Arc::downgrade(&timings);
            let global_timings = GlobalThreadTimings {
                thread_id: std::thread::current().id(),
                timings,
            };
            GLOBAL_THREAD_TIMINGS.lock().push(global_timings);
        }

        timings
    });
}

#[doc(hidden)]
pub struct ThreadTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: TaskTimings,
    pub running: Option<ActiveTiming>,
    pub stats: TaskStatistics,
    pub total_pushed: u64,
}

impl ThreadTimings {
    pub fn new(thread_name: Option<String>, thread_id: ThreadId) -> Self {
        ThreadTimings {
            thread_name,
            thread_id,
            timings: TaskTimings::new(),
            stats: TaskStatistics::default(),
            total_pushed: 0,
            running: None,
        }
    }

    #[cfg(feature = "profiler")]
    pub fn update_running_task(
        &mut self,
        spawned: SpawnTime,
        location: &'static std::panic::Location<'_>,
    ) {
        let start = Instant::now();
        self.running = Some(ActiveTiming {
            spawned,
            location,
            start,
        });
    }
    #[cfg(not(feature = "profiler"))]
    pub fn update_running_task(&mut self, _: SpawnTime, _: &'static std::panic::Location<'_>) {}

    #[cfg(feature = "profiler")]
    pub fn save_task_timing(&mut self, ended: YieldTime) {
        let ActiveTiming {
            location,
            start,
            spawned,
        } = self
            .running
            .take()
            .expect("this function is only ever called after register_task_start");

        let timing = TaskTiming {
            location,
            spawned,
            start,
            end: ended,
        };
        self.stats.add_yield_timing(timing);
        self.stats.add_runtime(timing);

        if trace_enabled() {
            std::hint::cold_path(); // optimize for when the profiling is off
            if self.timings.len() >= MAX_TASK_TIMINGS {
                self.timings.pop_front();
            }
            self.timings.push_back(timing);
            self.total_pushed += 1;
        }
    }
    #[cfg(not(feature = "profiler"))]
    pub fn save_task_timing(&mut self, _: YieldTime) {}

    // Running tasks are included in the reliability trace, which is written
    // whenever the foreground executor makes no progress for > n seconds
    pub fn get_thread_task_timings(&self, includes: TasksIncluded) -> ThreadTaskTimings {
        ThreadTaskTimings {
            thread_name: self.thread_name.clone(),
            thread_id: self.thread_id,
            timings: self
                .timings
                .iter()
                .cloned()
                .chain(
                    self.running
                        .filter(|_| matches!(includes, TasksIncluded::CompletedAndRunning))
                        .map(|running| TaskTiming {
                            spawned: running.spawned,
                            location: running.location,
                            start: running.start,
                            end: YieldTime(Instant::now()),
                        }),
                )
                .collect(),
            stats: self.stats.clone(),
            total_pushed: self.total_pushed,
        }
    }
}

impl Drop for ThreadTimings {
    fn drop(&mut self) {
        let mut thread_timings = GLOBAL_THREAD_TIMINGS.lock();

        let Some((index, _)) = thread_timings
            .iter()
            .enumerate()
            .find(|(_, t)| t.thread_id == self.thread_id)
        else {
            return;
        };
        thread_timings.swap_remove(index);
    }
}

#[doc(hidden)]
pub fn update_running_task(spawned: SpawnTime, location: &'static std::panic::Location<'_>) {
    THREAD_TIMINGS.with(|timings| {
        timings.lock().update_running_task(spawned, location);
    });
}

#[doc(hidden)]
pub fn save_task_timing() {
    let yielded_at = YieldTime(Instant::now());
    THREAD_TIMINGS.with(|timings| {
        timings.lock().save_task_timing(yielded_at);
    });
}

#[doc(hidden)]
pub fn get_current_thread_task_timings(include_running: TasksIncluded) -> ThreadTaskTimings {
    THREAD_TIMINGS.with(|timings| timings.lock().get_thread_task_timings(include_running))
}

static PROFILER_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables or disables task timing trace collection at runtime.
///
/// When transitioning from enabled to disabled, `add_task_timing` becomes a
/// cheaper since only cheap statistics are gathered. The existing per-thread
/// buffers for traces are cleared so stale data isn't reported after a later
/// re-enable. Calls with the current value are a no-op.
pub fn set_trace_enabled(enabled: bool) -> bool {
    if PROFILER_ENABLED.swap(enabled, Ordering::AcqRel) == enabled {
        return false;
    }

    if !enabled {
        for (_, timings) in upgraded_thread_timings() {
            let mut timings = timings.lock();
            timings.timings.clear();
            timings.timings.shrink_to_fit();
            timings.total_pushed = 0;
        }
    }
    true
}

/// Returns whether task timing tracing is enabled.
pub fn trace_enabled() -> bool {
    PROFILER_ENABLED.load(Ordering::Relaxed)
}

/// Timing for a single drawn window frame.
#[derive(Debug, Copy, Clone)]
pub struct FrameTiming {
    /// The window that was drawn.
    pub window_id: WindowId,
    /// When the frame first became dirty (its first invalidation). `None` if
    /// frame tracing was not yet enabled when the invalidation occurred.
    pub dirty_at: Option<Instant>,
    /// Number of invalidations coalesced into this frame.
    pub invalidations: u64,
    /// When `Window::draw` started.
    pub draw_start: Instant,
    /// When `Window::draw` finished.
    pub draw_end: Instant,
}

impl FrameTiming {
    /// Time spent inside `Window::draw`.
    pub fn draw_duration(&self) -> Duration {
        self.draw_end.duration_since(self.draw_start)
    }

    /// Time from the frame's first invalidation to the end of its draw, if the
    /// first invalidation was observed.
    pub fn dirty_to_draw_duration(&self) -> Option<Duration> {
        self.dirty_at
            .map(|dirty_at| self.draw_end.duration_since(dirty_at))
    }
}

/// Timing for one newly drawn frame submitted to the platform.
///
/// `submission_end` is sampled when the synchronous platform draw call
/// returns. It proves that GPUI handed the frame to the platform renderer; it
/// does not claim that a compositor or display has presented the frame.
#[derive(Debug, Copy, Clone)]
pub struct FrameSubmissionTiming {
    /// The draw whose scene was submitted.
    pub frame: FrameTiming,
    /// When platform submission began.
    pub submission_start: Instant,
    /// When the platform draw call returned.
    pub submission_end: Instant,
    /// The first input that invalidated work included in this submission, if
    /// one was observed while tracing was active.
    pub first_input_at: Option<Instant>,
    /// Number of invalidating top-level input events coalesced into this
    /// submission.
    pub input_events: u64,
}

impl FrameSubmissionTiming {
    /// Time spent in the platform submission call.
    pub fn submission_duration(&self) -> Duration {
        self.submission_end.duration_since(self.submission_start)
    }

    /// Time from the draw start until platform submission returned.
    pub fn draw_to_submission_duration(&self) -> Duration {
        self.submission_end.duration_since(self.frame.draw_start)
    }

    /// Time from the first observed invalidation until platform submission
    /// returned.
    pub fn dirty_to_submission_duration(&self) -> Option<Duration> {
        self.frame
            .dirty_at
            .map(|dirty_at| self.submission_end.duration_since(dirty_at))
    }

    /// Time from the first coalesced input until platform submission returned.
    pub fn input_to_submission_duration(&self) -> Option<Duration> {
        self.first_input_at
            .map(|input_at| self.submission_end.duration_since(input_at))
    }
}

/// Deterministic structural work performed while drawing one window frame.
///
/// Unlike [`FrameTiming`], these counters do not depend on machine speed. They
/// are always collected so tests and diagnostics can enforce that work stays
/// proportional to the viewport rather than to the size of a caller's data
/// set. Allocation accounting is optional because it is only available when
/// GPUI is built with test support.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameStats {
    /// Monotonically increasing frame number for this window.
    pub frame_index: u64,
    /// Reactive view entities whose [`crate::Render::render`] method ran.
    pub entity_renders: u64,
    /// Element request-layout calls.
    pub request_layout_calls: u64,
    /// Element prepaint calls.
    pub prepaint_calls: u64,
    /// Element paint calls.
    pub paint_calls: u64,
    /// Invalidations coalesced into this frame.
    pub invalidations: u64,
    /// Product semantic nodes published while this frame was painted.
    pub semantic_nodes: u64,
    /// Native platform-view placements retained by the completed frame.
    pub platform_view_placements: u64,
    /// Growth in the retained element-arena capacity during this frame.
    ///
    /// `None` means the active build does not include allocator accounting.
    pub allocator_delta_bytes: Option<i64>,
}

#[derive(Debug, Copy, Clone)]
enum FrameEvent {
    Draw(FrameTiming),
    Submission(FrameSubmissionTiming),
}

// Allow 16MiB of frame events. Draw and submission records share one ring so
// enabling submission timing does not double the profiler's memory ceiling.
const MAX_FRAME_EVENTS: usize = (16 * 1024 * 1024) / core::mem::size_of::<FrameEvent>();

struct FrameTimings {
    events: VecDeque<FrameEvent>,
    total_pushed: u64,
    manual_enabled: bool,
    lease_count: usize,
    generation: u64,
}

static FRAME_TIMINGS: spin::Mutex<FrameTimings> = spin::Mutex::new(FrameTimings {
    events: VecDeque::new(),
    total_pushed: 0,
    manual_enabled: false,
    lease_count: 0,
    generation: 0,
});

static FRAME_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

impl FrameTimings {
    fn enabled(&self) -> bool {
        self.manual_enabled || self.lease_count > 0
    }

    fn clear(&mut self) {
        self.events.clear();
        self.events.shrink_to_fit();
        self.total_pushed = 0;
    }

    fn push(&mut self, event: FrameEvent) {
        if self.events.len() >= MAX_FRAME_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.total_pushed = self.total_pushed.saturating_add(1);
    }

    fn update_enabled_generation(&mut self, was_enabled: bool) -> bool {
        let enabled = self.enabled();
        if enabled != was_enabled {
            self.generation = self.generation.wrapping_add(1);
        }
        enabled
    }
}

/// Enables or disables the manual owner of frame timing collection at runtime.
///
/// Collection remains enabled while any [`FrameTraceLease`] exists. When the
/// final owner releases tracing, buffered timings are cleared so stale data is
/// not reported after a later re-enable. Returns false if the manual owner's
/// value was unchanged.
pub fn set_frame_trace_enabled(enabled: bool) -> bool {
    let mut frames = FRAME_TIMINGS.lock();
    if frames.manual_enabled == enabled {
        return false;
    }

    let was_enabled = frames.enabled();
    frames.manual_enabled = enabled;
    let is_enabled = frames.update_enabled_generation(was_enabled);
    if was_enabled && !is_enabled {
        frames.clear();
    }
    FRAME_TRACE_ENABLED.store(is_enabled, Ordering::Release);
    true
}

/// Returns whether frame timing collection is enabled.
pub fn frame_trace_enabled() -> bool {
    FRAME_TRACE_ENABLED.load(Ordering::Relaxed)
}

/// Identifies the current uninterrupted frame-tracing session.
///
/// Window integration uses this to prevent a draw captured before tracing was
/// disabled from becoming a submission sample after tracing is enabled again.
pub(crate) fn frame_trace_generation() -> Option<u64> {
    if !frame_trace_enabled() {
        return None;
    }
    let frames = FRAME_TIMINGS.lock();
    frames.enabled().then_some(frames.generation)
}

/// A reference-counted owner of frame timing collection.
///
/// Acquire a lease before creating a [`FrameTimingCollector`]. Dropping one
/// lease cannot disable tracing requested manually or by another consumer.
/// A lease records existing application redraws; it never schedules frames.
#[derive(Debug)]
pub struct FrameTraceLease {
    active: bool,
}

impl FrameTraceLease {
    /// Acquires one independent owner of frame timing collection.
    pub fn new() -> Self {
        let mut frames = FRAME_TIMINGS.lock();
        let was_enabled = frames.enabled();
        frames.lease_count = frames.lease_count.saturating_add(1);
        let enabled = frames.update_enabled_generation(was_enabled);
        FRAME_TRACE_ENABLED.store(enabled, Ordering::Release);
        Self { active: true }
    }
}

impl Default for FrameTraceLease {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FrameTraceLease {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let mut frames = FRAME_TIMINGS.lock();
        let was_enabled = frames.enabled();
        frames.lease_count = frames.lease_count.saturating_sub(1);
        let enabled = frames.update_enabled_generation(was_enabled);
        if !enabled {
            frames.clear();
        }
        FRAME_TRACE_ENABLED.store(enabled, Ordering::Release);
        self.active = false;
    }
}

/// Records the timing of a drawn window frame.
///
/// No-op unless frame tracing is enabled via [`set_frame_trace_enabled`].
pub fn record_frame_timing(timing: FrameTiming) {
    if !frame_trace_enabled() {
        return;
    }
    std::hint::cold_path(); // optimize for when profiling is off

    let mut frames = FRAME_TIMINGS.lock();
    if !frames.enabled() {
        return;
    }
    frames.push(FrameEvent::Draw(timing));
}

/// Records a newly drawn frame's platform submission.
///
/// The generation must be the one captured when that frame was drawn. This is
/// crate-private because only the window owns the platform submission call.
pub(crate) fn record_frame_submission(generation: u64, timing: FrameSubmissionTiming) {
    if !frame_trace_enabled() {
        return;
    }
    std::hint::cold_path();

    let mut frames = FRAME_TIMINGS.lock();
    if !frames.enabled() || frames.generation != generation {
        return;
    }
    frames.push(FrameEvent::Submission(timing));
}

/// Drains frame timings recorded after this collector was created, tracking a
/// cursor so each call to [`Self::collect_unseen`] returns only new entries.
pub struct FrameTimingCollector {
    cursor: u64,
}

impl Default for FrameTimingCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimingCollector {
    /// Creates a collector that only sees frames recorded from this point on.
    pub fn new() -> Self {
        Self {
            cursor: FRAME_TIMINGS.lock().total_pushed,
        }
    }

    /// Returns frame timings recorded since the previous call (or since the
    /// collector was created). If the ring buffer wrapped around since the
    /// previous poll, the evicted entries are lost.
    pub fn collect_unseen(&mut self) -> Vec<FrameTiming> {
        let frames = FRAME_TIMINGS.lock();
        let buffer_len = frames.events.len() as u64;
        let buffer_start = frames.total_pushed.saturating_sub(buffer_len);
        let skip = self.cursor.saturating_sub(buffer_start) as usize;
        let unseen = frames
            .events
            .iter()
            .skip(skip.min(frames.events.len()))
            .filter_map(|event| match event {
                FrameEvent::Draw(timing) => Some(*timing),
                FrameEvent::Submission(_) => None,
            })
            .collect();
        self.cursor = frames.total_pushed;
        unseen
    }
}

/// Drains platform-submission timings observed after this collector was
/// created. Each sample belongs to a newly drawn frame; requests that resubmit
/// unchanged scene contents are not included.
pub struct FrameSubmissionTimingCollector {
    cursor: u64,
}

impl Default for FrameSubmissionTimingCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameSubmissionTimingCollector {
    /// Creates a collector that only sees submissions recorded from this point
    /// on. Hold a [`FrameTraceLease`] for the collector's lifetime.
    pub fn new() -> Self {
        Self {
            cursor: FRAME_TIMINGS.lock().total_pushed,
        }
    }

    /// Returns newly observed frame submissions. If the shared ring wrapped
    /// since the previous poll, evicted entries are lost.
    pub fn collect_unseen(&mut self) -> Vec<FrameSubmissionTiming> {
        let frames = FRAME_TIMINGS.lock();
        let buffer_len = frames.events.len() as u64;
        let buffer_start = frames.total_pushed.saturating_sub(buffer_len);
        let skip = self.cursor.saturating_sub(buffer_start) as usize;
        let unseen = frames
            .events
            .iter()
            .skip(skip.min(frames.events.len()))
            .filter_map(|event| match event {
                FrameEvent::Draw(_) => None,
                FrameEvent::Submission(timing) => Some(*timing),
            })
            .collect();
        self.cursor = frames.total_pushed;
        unseen
    }
}

/// A bounded, per-window summary of observed submitted-frame work.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameTimingSummary {
    /// Number of retained target-window submissions represented by this
    /// summary.
    pub sample_count: usize,
    /// Newly drawn submissions per second, computed from their completion
    /// timestamps rather than from the monitor's polling cadence.
    pub frames_per_second: f64,
    /// Caller-supplied threshold used to classify draw-budget overage.
    pub frame_budget: Duration,
    /// Arithmetic mean of retained `Window::draw` durations.
    pub mean_draw_duration: Duration,
    /// Nearest-rank 95th percentile of retained draw durations.
    pub p95_draw_duration: Duration,
    /// Fraction of retained draws whose own duration exceeded `frame_budget`.
    /// This is draw-budget overage, not a claim about display drops.
    pub over_budget_fraction: f64,
    /// Arithmetic mean of invalidations coalesced into each retained draw.
    pub mean_invalidations: f64,
    /// Mean first-dirty-to-draw-end latency for samples whose first
    /// invalidation was observed after tracing began.
    pub mean_dirty_to_draw_duration: Option<Duration>,
    /// Arithmetic mean of time spent in the synchronous platform submission
    /// call. This is not compositor or display presentation time.
    pub mean_submission_duration: Duration,
    /// Mean first-dirty-to-submission-end latency for samples whose first
    /// invalidation was observed after tracing began.
    pub mean_dirty_to_submission_duration: Option<Duration>,
    /// Mean first-input-to-submission-end latency for submitted frames caused
    /// by input observed after tracing began.
    pub mean_input_to_submission_duration: Option<Duration>,
    /// Arithmetic mean of invalidating top-level input events coalesced into a
    /// submitted frame.
    pub mean_input_events: f64,
    /// Draw durations in retained timestamp order, for caller-owned plots.
    pub draw_durations: Vec<Duration>,
    /// Platform submission-call durations in retained timestamp order, for
    /// caller-owned plots.
    pub submission_durations: Vec<Duration>,
}

/// Observes existing submitted frames for one window and keeps a bounded
/// history.
///
/// The monitor owns a [`FrameTraceLease`] but never refreshes the window. A
/// host chooses when to call [`Self::collect`] and whether its workload should
/// produce another frame, so displaying diagnostics does not silently create
/// a continuous redraw loop.
pub struct FrameTimingMonitor {
    window_id: WindowId,
    capacity: usize,
    frame_budget: Duration,
    samples: VecDeque<FrameSubmissionTiming>,
    collector: FrameSubmissionTimingCollector,
    _lease: FrameTraceLease,
}

impl FrameTimingMonitor {
    /// Creates a monitor for `window_id`. A zero capacity retains one sample,
    /// although a summary is unavailable until two submissions are observed.
    pub fn new(window_id: WindowId, capacity: usize, frame_budget: Duration) -> Self {
        let lease = FrameTraceLease::new();
        let collector = FrameSubmissionTimingCollector::new();
        Self {
            window_id,
            capacity: capacity.max(1),
            frame_budget,
            samples: VecDeque::with_capacity(capacity.max(1)),
            collector,
            _lease: lease,
        }
    }

    /// The only window whose draws this monitor retains.
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Number of currently retained target-window submissions.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Collects newly submitted frames, discarding frames from other windows,
    /// and returns the current summary once two target-window submissions
    /// exist.
    pub fn collect(&mut self) -> Option<FrameTimingSummary> {
        for timing in self.collector.collect_unseen() {
            if timing.frame.window_id != self.window_id {
                continue;
            }
            if self.samples.len() >= self.capacity {
                self.samples.pop_front();
            }
            self.samples.push_back(timing);
        }
        self.summary()
    }

    /// Summarizes the retained samples without consuming them.
    pub fn summary(&self) -> Option<FrameTimingSummary> {
        summarize_frame_timings(&self.samples, self.frame_budget)
    }
}

fn summarize_frame_timings(
    samples: &VecDeque<FrameSubmissionTiming>,
    frame_budget: Duration,
) -> Option<FrameTimingSummary> {
    if samples.len() < 2 {
        return None;
    }

    let sample_count = samples.len();
    let elapsed = samples
        .back()?
        .submission_end
        .saturating_duration_since(samples.front()?.submission_end);
    let frames_per_second = if elapsed.is_zero() {
        0.0
    } else {
        (sample_count - 1) as f64 / elapsed.as_secs_f64()
    };
    let draw_durations = samples
        .iter()
        .map(|sample| sample.frame.draw_duration())
        .collect::<Vec<_>>();
    let submission_durations = samples
        .iter()
        .map(FrameSubmissionTiming::submission_duration)
        .collect::<Vec<_>>();
    let mean_draw_duration = mean_duration(draw_durations.iter().copied());
    let mut sorted_draws = draw_durations.clone();
    sorted_draws.sort_unstable();
    let p95_index = (sample_count * 95).div_ceil(100).saturating_sub(1);
    let p95_draw_duration = sorted_draws[p95_index];
    let over_budget = draw_durations
        .iter()
        .filter(|duration| **duration > frame_budget)
        .count();
    let mean_invalidations = samples
        .iter()
        .map(|sample| sample.frame.invalidations as f64)
        .sum::<f64>()
        / sample_count as f64;
    let dirty_to_draw = samples
        .iter()
        .filter_map(|sample| sample.frame.dirty_to_draw_duration())
        .collect::<Vec<_>>();
    let dirty_to_submission = samples
        .iter()
        .filter_map(FrameSubmissionTiming::dirty_to_submission_duration)
        .collect::<Vec<_>>();
    let input_to_submission = samples
        .iter()
        .filter_map(FrameSubmissionTiming::input_to_submission_duration)
        .collect::<Vec<_>>();

    Some(FrameTimingSummary {
        sample_count,
        frames_per_second,
        frame_budget,
        mean_draw_duration,
        p95_draw_duration,
        over_budget_fraction: over_budget as f64 / sample_count as f64,
        mean_invalidations,
        mean_dirty_to_draw_duration: (!dirty_to_draw.is_empty())
            .then(|| mean_duration(dirty_to_draw.iter().copied())),
        mean_submission_duration: mean_duration(submission_durations.iter().copied()),
        mean_dirty_to_submission_duration: (!dirty_to_submission.is_empty())
            .then(|| mean_duration(dirty_to_submission.iter().copied())),
        mean_input_to_submission_duration: (!input_to_submission.is_empty())
            .then(|| mean_duration(input_to_submission.iter().copied())),
        mean_input_events: samples
            .iter()
            .map(|sample| sample.input_events as f64)
            .sum::<f64>()
            / sample_count as f64,
        draw_durations,
        submission_durations,
    })
}

fn mean_duration(durations: impl Iterator<Item = Duration>) -> Duration {
    let (total, count) = durations.fold((0_u128, 0_u128), |(total, count), duration| {
        (total.saturating_add(duration.as_nanos()), count + 1)
    });
    if count == 0 {
        return Duration::ZERO;
    }
    Duration::from_nanos(u64::try_from(total / count).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod frame_timing_tests {
    use super::*;

    struct TimingArgs {
        window_id: u64,
        anchor: Instant,
        start_ms: u64,
        draw_ms: u64,
        dirty_ms: Option<u64>,
        invalidations: u64,
        submission_ms: u64,
        submission_duration_ms: u64,
        input_ms: Option<u64>,
        input_events: u64,
    }

    fn timing(args: TimingArgs) -> FrameSubmissionTiming {
        let draw_start = args.anchor + Duration::from_millis(args.start_ms);
        let submission_start = args.anchor + Duration::from_millis(args.submission_ms);
        FrameSubmissionTiming {
            frame: FrameTiming {
                window_id: WindowId::from(args.window_id),
                dirty_at: args
                    .dirty_ms
                    .map(|milliseconds| args.anchor + Duration::from_millis(milliseconds)),
                invalidations: args.invalidations,
                draw_start,
                draw_end: draw_start + Duration::from_millis(args.draw_ms),
            },
            submission_start,
            submission_end: submission_start + Duration::from_millis(args.submission_duration_ms),
            first_input_at: args
                .input_ms
                .map(|milliseconds| args.anchor + Duration::from_millis(milliseconds)),
            input_events: args.input_events,
        }
    }

    #[test]
    fn frame_summary_uses_submissions_and_reports_draw_budget_overage() {
        let anchor = Instant::now();
        let samples = VecDeque::from([
            timing(TimingArgs {
                window_id: 1,
                anchor,
                start_ms: 10,
                draw_ms: 4,
                dirty_ms: Some(6),
                invalidations: 1,
                submission_ms: 14,
                submission_duration_ms: 2,
                input_ms: Some(8),
                input_events: 1,
            }),
            timing(TimingArgs {
                window_id: 1,
                anchor,
                start_ms: 30,
                draw_ms: 8,
                dirty_ms: Some(30),
                invalidations: 3,
                submission_ms: 38,
                submission_duration_ms: 4,
                input_ms: Some(28),
                input_events: 3,
            }),
            timing(TimingArgs {
                window_id: 1,
                anchor,
                start_ms: 50,
                draw_ms: 30,
                dirty_ms: None,
                invalidations: 2,
                submission_ms: 80,
                submission_duration_ms: 6,
                input_ms: None,
                input_events: 0,
            }),
            timing(TimingArgs {
                window_id: 1,
                anchor,
                start_ms: 90,
                draw_ms: 12,
                dirty_ms: Some(90),
                invalidations: 2,
                submission_ms: 102,
                submission_duration_ms: 4,
                input_ms: Some(88),
                input_events: 2,
            }),
        ]);

        let summary = summarize_frame_timings(&samples, Duration::from_millis(16))
            .expect("four samples produce a summary");
        assert_eq!(summary.sample_count, 4);
        assert_eq!(summary.frames_per_second, 3.0 / 0.09);
        assert_eq!(summary.mean_draw_duration, Duration::from_micros(13_500));
        assert_eq!(summary.p95_draw_duration, Duration::from_millis(30));
        assert_eq!(summary.over_budget_fraction, 0.25);
        assert_eq!(summary.mean_invalidations, 2.0);
        assert_eq!(
            summary.mean_dirty_to_draw_duration,
            Some(Duration::from_millis(28) / 3)
        );
        assert_eq!(summary.mean_submission_duration, Duration::from_millis(4));
        assert_eq!(
            summary.mean_dirty_to_submission_duration,
            Some(Duration::from_millis(38) / 3)
        );
        assert_eq!(
            summary.mean_input_to_submission_duration,
            Some(Duration::from_millis(40) / 3)
        );
        assert_eq!(summary.mean_input_events, 1.5);
        assert_eq!(summary.draw_durations.len(), 4);
        assert_eq!(summary.submission_durations.len(), 4);
    }

    #[test]
    fn frame_summary_waits_for_two_submissions() {
        let anchor = Instant::now();
        let samples = VecDeque::from([timing(TimingArgs {
            window_id: 1,
            anchor,
            start_ms: 0,
            draw_ms: 4,
            dirty_ms: Some(0),
            invalidations: 1,
            submission_ms: 4,
            submission_duration_ms: 1,
            input_ms: None,
            input_events: 0,
        })]);
        assert!(summarize_frame_timings(&samples, Duration::from_millis(16)).is_none());
    }

    #[test]
    fn frame_trace_ownership_keeps_collection_enabled_until_every_owner_releases() {
        let mut frames = FrameTimings {
            events: VecDeque::new(),
            total_pushed: 0,
            manual_enabled: false,
            lease_count: 0,
            generation: 0,
        };
        assert!(!frames.enabled());
        let was_enabled = frames.enabled();
        frames.lease_count += 1;
        assert!(frames.update_enabled_generation(was_enabled));
        let active_generation = frames.generation;
        let was_enabled = frames.enabled();
        frames.lease_count += 1;
        assert!(frames.update_enabled_generation(was_enabled));
        assert_eq!(frames.generation, active_generation);
        assert!(frames.enabled());
        let was_enabled = frames.enabled();
        frames.manual_enabled = true;
        assert!(frames.update_enabled_generation(was_enabled));
        frames.lease_count -= 2;
        let was_enabled = frames.enabled();
        assert!(frames.update_enabled_generation(was_enabled));
        assert!(frames.enabled());
        let was_enabled = frames.enabled();
        frames.manual_enabled = false;
        assert!(!frames.update_enabled_generation(was_enabled));
        assert!(!frames.enabled());
        assert_ne!(frames.generation, active_generation);
    }
}
