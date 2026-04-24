//! Orchestrator-side drag planning for corpse and object movement.
//!
//! This module intentionally plans the command surface only. The live drag
//! action is performed inside the injected DLL once the game-client hook is
//! available.

use std::{error::Error, fmt};

use textquest_common::nav::Waypoint;

/// Entity selected for a drag operation.
#[derive(Debug, Clone, PartialEq)]
pub enum DragSubject {
    /// The nearest eligible corpse in the current zone.
    NearestCorpse,
    /// A movable world object such as a barrel or crate.
    Object {
        /// Game object identifier supplied by the caller.
        object_id: u32,
    },
}

/// Minimal drag plan that can be translated to DLL navigation commands.
#[derive(Debug, Clone, PartialEq)]
pub struct DragPlan {
    /// Entity to pick up and move.
    pub subject: DragSubject,
    /// Destination for the dragged entity.
    pub destination: Waypoint,
}

impl DragPlan {
    /// Create a corpse-drag plan for the nearest corpse.
    pub fn corpse_to(destination: Waypoint) -> Result<Self, DragPlanError> {
        validate_destination(&destination)?;
        Ok(Self {
            subject: DragSubject::NearestCorpse,
            destination,
        })
    }

    /// Create an object-drag plan for a specific object id.
    pub fn object_to(object_id: u32, destination: Waypoint) -> Result<Self, DragPlanError> {
        if object_id == 0 {
            return Err(DragPlanError::InvalidObjectId);
        }
        validate_destination(&destination)?;
        Ok(Self {
            subject: DragSubject::Object { object_id },
            destination,
        })
    }
}

/// Build the public `textquest.drag_corpse_to(...)` navigation plan.
pub fn drag_corpse_to(destination: Waypoint) -> Result<DragPlan, DragPlanError> {
    DragPlan::corpse_to(destination)
}

/// Build the public `textquest.drag_object(...)` navigation plan.
pub fn drag_object(object_id: u32, destination: Waypoint) -> Result<DragPlan, DragPlanError> {
    DragPlan::object_to(object_id, destination)
}

/// Validation failures for drag plan construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPlanError {
    /// Object id zero is reserved for "no object".
    InvalidObjectId,
    /// Destination contains NaN or infinity and cannot be navigated to.
    InvalidDestination,
}

impl fmt::Display for DragPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidObjectId => f.write_str("drag object id must be non-zero"),
            Self::InvalidDestination => f.write_str("drag destination must be finite"),
        }
    }
}

impl Error for DragPlanError {}

fn validate_destination(destination: &Waypoint) -> Result<(), DragPlanError> {
    if destination.x.is_finite() && destination.y.is_finite() && destination.z.is_finite() {
        Ok(())
    } else {
        Err(DragPlanError::InvalidDestination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpse_plan_targets_nearest_corpse() {
        let destination = Waypoint::new(100.0, 200.0, 3.0);
        let plan = drag_corpse_to(destination).expect("valid corpse drag plan");

        assert_eq!(plan.subject, DragSubject::NearestCorpse);
        assert_eq!(plan.destination, destination);
    }

    #[test]
    fn object_plan_rejects_zero_id() {
        let err = drag_object(0, Waypoint::new(100.0, 200.0, 3.0))
            .expect_err("zero is not a valid object id");

        assert_eq!(err, DragPlanError::InvalidObjectId);
    }

    #[test]
    fn corpse_plan_rejects_non_finite_destination() {
        let err = drag_corpse_to(Waypoint::new(f32::NAN, 200.0, 3.0))
            .expect_err("NaN cannot be navigated to");

        assert_eq!(err, DragPlanError::InvalidDestination);
    }
}
