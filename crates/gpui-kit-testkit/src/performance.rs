//! Deterministic performance budgets for rendered component trees.
//!
//! These assertions deliberately count structural work instead of elapsed
//! milliseconds. A viewport that mounts 30 rows has the same budget on a
//! laptop, a CI virtual machine, and a browser test runner.

use std::collections::BTreeMap;
use std::fmt;

use gpui::FrameStats;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One deterministic quantity that a [`PerformanceBudget`] can limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceMetric {
    /// Reactive view entities rendered in the measured frame.
    EntityRenders,
    /// Element request-layout calls.
    RequestLayoutCalls,
    /// Element prepaint calls.
    PrepaintCalls,
    /// Element paint calls.
    PaintCalls,
    /// Invalidations coalesced into the measured frame.
    Invalidations,
    /// Product semantic nodes published in the frame.
    SemanticNodes,
    /// Native platform-view placements in the frame.
    PlatformViewPlacements,
    /// Retained allocator growth in bytes.
    AllocatorDeltaBytes,
    /// Heap allocation and reallocation calls made during the measured frame.
    HeapAllocations,
    /// Dataset items mounted by a virtualized builder.
    MountedItems,
    /// Calls to caller-owned item or block builders.
    BuilderCalls,
}

impl fmt::Display for PerformanceMetric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EntityRenders => "entity_renders",
            Self::RequestLayoutCalls => "request_layout_calls",
            Self::PrepaintCalls => "prepaint_calls",
            Self::PaintCalls => "paint_calls",
            Self::Invalidations => "invalidations",
            Self::SemanticNodes => "semantic_nodes",
            Self::PlatformViewPlacements => "platform_view_placements",
            Self::AllocatorDeltaBytes => "allocator_delta_bytes",
            Self::HeapAllocations => "heap_allocations",
            Self::MountedItems => "mounted_items",
            Self::BuilderCalls => "builder_calls",
        })
    }
}

/// Structural measurements from one completed frame and its caller-owned
/// virtualized builders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceSample {
    /// Framework-owned frame counters.
    pub frame: FrameStats,
    /// Heap allocation and reallocation calls during the measured frame.
    pub heap_allocations: u64,
    /// Dataset items mounted by the component during the frame.
    pub mounted_items: u64,
    /// Calls to caller-owned row or block builders during the frame.
    pub builder_calls: u64,
}

impl PerformanceSample {
    /// Starts a sample from GPUI's completed frame counters.
    pub fn new(frame: FrameStats) -> Self {
        Self {
            frame,
            heap_allocations: 0,
            mounted_items: 0,
            builder_calls: 0,
        }
    }

    /// Records the number of dataset items mounted in the frame.
    pub fn mounted_items(mut self, count: u64) -> Self {
        self.mounted_items = count;
        self
    }

    /// Records calls to the caller-owned virtualized builder.
    pub fn builder_calls(mut self, count: u64) -> Self {
        self.builder_calls = count;
        self
    }

    /// Records heap allocation and reallocation calls in the frame.
    pub fn heap_allocations(mut self, count: u64) -> Self {
        self.heap_allocations = count;
        self
    }

    fn value(&self, metric: PerformanceMetric) -> Option<u64> {
        Some(match metric {
            PerformanceMetric::EntityRenders => self.frame.entity_renders,
            PerformanceMetric::RequestLayoutCalls => self.frame.request_layout_calls,
            PerformanceMetric::PrepaintCalls => self.frame.prepaint_calls,
            PerformanceMetric::PaintCalls => self.frame.paint_calls,
            PerformanceMetric::Invalidations => self.frame.invalidations,
            PerformanceMetric::SemanticNodes => self.frame.semantic_nodes,
            PerformanceMetric::PlatformViewPlacements => self.frame.platform_view_placements,
            PerformanceMetric::AllocatorDeltaBytes => {
                return self
                    .frame
                    .allocator_delta_bytes
                    .and_then(|value| u64::try_from(value).ok());
            }
            PerformanceMetric::HeapAllocations => self.heap_allocations,
            PerformanceMetric::MountedItems => self.mounted_items,
            PerformanceMetric::BuilderCalls => self.builder_calls,
        })
    }
}

/// A named upper bound for one structural metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceLimit {
    /// Metric being constrained.
    pub metric: PerformanceMetric,
    /// Inclusive maximum accepted value.
    pub maximum: u64,
}

/// Named deterministic limits for one component scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceBudget {
    /// Stable scenario name used in reports and CI diagnostics.
    pub name: String,
    /// Inclusive maximum values by metric.
    pub limits: BTreeMap<PerformanceMetric, u64>,
}

impl PerformanceBudget {
    /// Creates an empty named budget.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            limits: BTreeMap::new(),
        }
    }

    /// Adds or replaces one named metric limit.
    pub fn limit(mut self, metric: PerformanceMetric, maximum: u64) -> Self {
        self.limits.insert(metric, maximum);
        self
    }

    /// Evaluates `sample`, returning a serializable report on success or
    /// carrying that same report in the error on failure.
    pub fn enforce(
        &self,
        sample: PerformanceSample,
    ) -> Result<PerformanceReport, PerformanceBudgetError> {
        let violations = self
            .limits
            .iter()
            .filter_map(|(&metric, &maximum)| {
                let actual = sample.value(metric);
                if actual.is_some_and(|value| value <= maximum) {
                    None
                } else {
                    Some(PerformanceViolation {
                        metric,
                        maximum,
                        actual,
                    })
                }
            })
            .collect::<Vec<_>>();
        let report = PerformanceReport {
            name: self.name.clone(),
            sample,
            limits: self
                .limits
                .iter()
                .map(|(&metric, &maximum)| PerformanceLimit { metric, maximum })
                .collect(),
            passed: violations.is_empty(),
            violations,
        };
        if report.passed {
            Ok(report)
        } else {
            Err(PerformanceBudgetError {
                report: Box::new(report),
            })
        }
    }
}

/// One exceeded or unavailable metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceViolation {
    /// Metric that failed.
    pub metric: PerformanceMetric,
    /// Inclusive configured maximum.
    pub maximum: u64,
    /// Measured value, or `None` when this build cannot measure the metric.
    pub actual: Option<u64>,
}

/// Machine-readable result for one budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Stable scenario name.
    pub name: String,
    /// Measurements that were checked.
    pub sample: PerformanceSample,
    /// Limits applied to the sample.
    pub limits: Vec<PerformanceLimit>,
    /// Whether every limit passed.
    pub passed: bool,
    /// Every exceeded or unavailable metric.
    pub violations: Vec<PerformanceViolation>,
}

/// A failed deterministic performance budget.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("performance budget `{}` failed: {}", report.name, format_violations(&report.violations))]
pub struct PerformanceBudgetError {
    /// Full serializable result, including every violation. It is boxed so
    /// that the failing arm of every budget call does not widen the result
    /// every caller returns.
    pub report: Box<PerformanceReport>,
}

fn format_violations(violations: &[PerformanceViolation]) -> String {
    violations
        .iter()
        .map(|violation| match violation.actual {
            Some(actual) => format!(
                "{} was {actual}, maximum {}",
                violation.metric, violation.maximum
            ),
            None => format!(
                "{} is unavailable, maximum {}",
                violation.metric, violation.maximum
            ),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_every_exceeded_and_unavailable_limit() {
        let budget = PerformanceBudget::new("virtual-list")
            .limit(PerformanceMetric::MountedItems, 20)
            .limit(PerformanceMetric::SemanticNodes, 24)
            .limit(PerformanceMetric::AllocatorDeltaBytes, 0);
        let sample = PerformanceSample::new(FrameStats {
            semantic_nodes: 30,
            allocator_delta_bytes: None,
            ..Default::default()
        })
        .mounted_items(19);

        let error = budget.enforce(sample).expect_err("budget must fail");
        assert_eq!(error.report.violations.len(), 2);
        assert_eq!(
            serde_json::to_value(&error.report).expect("serializable report")["passed"],
            false
        );
    }

    #[test]
    fn accepts_values_at_the_inclusive_limit() {
        let budget = PerformanceBudget::new("grid")
            .limit(PerformanceMetric::BuilderCalls, 12)
            .limit(PerformanceMetric::AllocatorDeltaBytes, 0)
            .limit(PerformanceMetric::HeapAllocations, 42);
        let sample = PerformanceSample::new(FrameStats {
            allocator_delta_bytes: Some(0),
            ..Default::default()
        })
        .heap_allocations(42)
        .builder_calls(12);

        assert!(budget.enforce(sample).expect("budget passes").passed);
    }
}
