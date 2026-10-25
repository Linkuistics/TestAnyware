//! The seam where the TestAnyware app gives a macOS start its admission gate.
//!
//! The app keeps one ordered queue for the macOS guest slots of its user (spec
//! "admission-queue": "The order"). A start asks the gate for a slot after its clone is ready and
//! before its first `tart run`. The Host CLI on a host without the app has no gate, and a start
//! there tries `tart run` and meets the framework's refusal, as it always has.
//!
//! The gate lives in this crate as a trait, and the app implements it. The lifecycle code then
//! needs nothing from the app, and the app needs nothing from the lifecycle beyond the trait.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::error::VmError;
use crate::start_context::Progress;

/// What a start asks of the gate.
#[derive(Debug, Clone)]
pub struct SlotAsk<'a> {
    /// The id of the VM that the start clones. The count names a start by it.
    pub vm_id: &'a str,
    /// How long the start may wait in the queue. `None` and zero both mean that the start never
    /// waits and never passes a waiter.
    pub wait: Option<Duration>,
}

/// A slot held for one start, from its admission until the value is dropped.
///
/// The start drops it after its `tart run` has exited, or after its guest has a meta: from then on
/// the meta holds the slot in the count.
pub trait SlotHold: Send {
    /// The place the start would take in the queue if the framework refused it now, from 1.
    fn place_if_requeued(&self) -> u32;
    /// The request id of the start.
    fn request_id(&self) -> &str;
    /// The seconds the start waited in the queue before its admission.
    fn waited(&self) -> Duration;
}

/// The queue of the app, as a start sees it.
pub trait SlotGate: Send + Sync + std::fmt::Debug {
    /// Join the queue, and block until the app admits the start. `progress` receives the lines
    /// of the wait. Returns `Err(VmError::StartCancelled)` when `cancel` fires first, and
    /// `Err(VmError::VmLimitReached)` when the wait runs out or when a start that never waits
    /// is not admitted at once. The gate holds no slot for a start that it fails.
    fn admit(
        &self,
        ask: &SlotAsk<'_>,
        cancel: &CancellationToken,
        progress: &Progress,
    ) -> Result<Box<dyn SlotHold>, VmError>;
}

/// The gate of a start, or none.
pub type SharedGate = Option<Arc<dyn SlotGate>>;
