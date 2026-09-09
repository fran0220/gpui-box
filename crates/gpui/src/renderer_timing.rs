//! Opt-in offscreen measurements. None of these clocks measures presentation.

use std::time::Duration;

/// GPU clock evidence, never a CPU completion or pixel-readback duration.
#[derive(Clone, Debug)]
pub enum GpuExecutionTime {
    /// This adapter/API cannot provide usable execution timestamps.
    Unsupported(String),
    /// Elapsed GPU time between commands bracketing the scene's render commands.
    Measured(Duration),
}

impl GpuExecutionTime {
    /// Validate a completed submission's timestamps. Callers must supply fresh,
    /// submission-owned storage, not a cached result from an earlier frame.
    pub fn from_timestamps(start: f64, end: f64, seconds_per_tick: f64) -> anyhow::Result<Self> {
        anyhow::ensure!(
            start.is_finite()
                && end.is_finite()
                && start > 0.0
                && end > start
                && seconds_per_tick.is_finite()
                && seconds_per_tick > 0.0,
            "missing, unordered, or invalid GPU timestamps"
        );
        let duration = Duration::try_from_secs_f64((end - start) * seconds_per_tick)?;
        anyhow::ensure!(
            !duration.is_zero(),
            "GPU timestamp interval rounded to zero"
        );
        Ok(Self::Measured(duration))
    }
}

/// One synchronously measured headless render attempt. IDs increase per renderer;
/// a failed attempt returns an error, never the previous attempt's measurement.
#[derive(Clone, Debug)]
pub struct RendererFrameTiming {
    /// Monotonically increasing measurement attempt ID, local to this renderer.
    pub submission_id: u64,
    /// CPU target preparation, encoding and submission; not GPU time.
    pub cpu_encode_submit: Duration,
    /// CPU wall time from submit returning until completion is observed. Includes
    /// scheduling/poll overhead; not GPU execution and not presentation latency.
    pub submit_to_completion: Duration,
    /// Execution-clock evidence or an explicit unsupported capability reason.
    pub gpu_execution: GpuExecutionTime,
    /// GPU query resolve/copy, mapping and CPU extraction after render completion.
    /// Absent when no timestamp query readback is performed.
    pub timestamp_readback: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_validation_never_turns_missing_or_invalid_evidence_into_zero() {
        for (start, end, period) in [
            (0.0, 0.0, 1.0),
            (4.0, 4.0, 1.0),
            (9.0, 3.0, 1.0),
            (1.0, f64::NAN, 1.0),
            (1.0, 3.0, 0.0),
            (1.0, 3.0, f64::INFINITY),
            (1.0, 3.0, -1.0),
        ] {
            assert!(GpuExecutionTime::from_timestamps(start, end, period).is_err());
        }
        let GpuExecutionTime::Measured(duration) =
            GpuExecutionTime::from_timestamps(11.0, 18.0, 0.000_001).expect("valid timestamps")
        else {
            panic!("valid timestamp pair must be measured")
        };
        assert_eq!(duration, Duration::from_micros(7));
    }
}
