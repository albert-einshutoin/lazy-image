// src/engine/pipeline/capabilities.rs
//
// Operation precondition tracking and pipeline-level validation. The
// `OperationCapabilities` struct models which pipeline-wide invariants are
// satisfied at a given point (decoded pixels available, color state tracked,
// EXIF orientation accessible); `validate_operation_sequence` walks an
// operation list and ensures every contract's requirements remain met.

use super::PipelineResult;
use crate::error::LazyImageError;
use crate::ops::{Operation, OperationEffect, OperationRequirement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OperationCapabilities {
    pub(super) decoded_pixels: bool,
    pub(super) color_state_tracked: bool,
    pub(super) orientation_available: bool,
}

impl OperationCapabilities {
    pub(super) fn with_defaults() -> Self {
        Self {
            decoded_pixels: true,
            color_state_tracked: true,
            // EXIF Orientation is parsed during decode; assume available unless stripped.
            orientation_available: true,
        }
    }

    fn meets(&self, contract: &crate::ops::OperationContract) -> bool {
        (!contract
            .requires
            .contains(OperationRequirement::DECODED_PIXELS)
            || self.decoded_pixels)
            && (!contract
                .requires
                .contains(OperationRequirement::COLOR_STATE)
                || self.color_state_tracked)
            && (!contract
                .requires
                .contains(OperationRequirement::ORIENTATION)
                || self.orientation_available)
    }

    fn apply(&mut self, contract: &crate::ops::OperationContract) {
        if contract.effects.contains(OperationEffect::NORMALIZES_COLOR) {
            self.color_state_tracked = true;
        }
    }
}

pub(super) fn validate_operation_sequence(ops: &[Operation]) -> PipelineResult<()> {
    let mut caps = OperationCapabilities::with_defaults();
    validate_operation_sequence_with_caps(ops, &mut caps)
}

pub(super) fn validate_operation_sequence_with_caps(
    ops: &[Operation],
    caps: &mut OperationCapabilities,
) -> PipelineResult<()> {
    for op in ops {
        let contract = op.contract();
        if !caps.meets(&contract) {
            return Err(LazyImageError::invalid_argument(
                "operation",
                contract.name,
                "operation prerequisites are not satisfied (missing required state)",
            ));
        }
        caps.apply(&contract);
    }
    Ok(())
}
