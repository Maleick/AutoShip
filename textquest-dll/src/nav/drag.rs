//! DLL-side drag command scaffolding.
//!
//! Navigation can prepare and validate drag requests now; the actual client
//! hook is isolated behind `DragMechanic` so the unsafe implementation can land
//! separately once offsets and mechanics are verified.

use std::{error::Error, fmt};

use textquest_common::nav::Waypoint;

/// Entity selected for a drag operation in the game client.
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

/// Validated request passed to the live drag mechanic.
#[derive(Debug, Clone, PartialEq)]
pub struct DragRequest {
    /// Entity to pick up and move.
    pub subject: DragSubject,
    /// Destination for the dragged entity.
    pub destination: Waypoint,
}

impl DragRequest {
    /// Create a corpse-drag request for the nearest corpse.
    pub fn corpse_to(destination: Waypoint) -> Result<Self, DragError> {
        validate_destination(&destination)?;
        Ok(Self {
            subject: DragSubject::NearestCorpse,
            destination,
        })
    }

    /// Create an object-drag request for a specific object id.
    pub fn object_to(object_id: u32, destination: Waypoint) -> Result<Self, DragError> {
        if object_id == 0 {
            return Err(DragError::InvalidObjectId);
        }
        validate_destination(&destination)?;
        Ok(Self {
            subject: DragSubject::Object { object_id },
            destination,
        })
    }
}

/// Game-client drag hook abstraction.
pub trait DragMechanic {
    /// Begin dragging the requested corpse or object toward its destination.
    fn begin_drag(&mut self, request: &DragRequest) -> Result<(), DragError>;
}

/// Production placeholder for the verified client hook.
#[derive(Debug, Default)]
pub struct HookedDragMechanic;

impl DragMechanic for HookedDragMechanic {
    fn begin_drag(&mut self, _request: &DragRequest) -> Result<(), DragError> {
        Err(DragError::HookUnavailable)
    }
}

/// Build the public `textquest.drag_corpse_to(...)` DLL request.
pub fn drag_corpse_to(destination: Waypoint) -> Result<DragRequest, DragError> {
    DragRequest::corpse_to(destination)
}

/// Build the public `textquest.drag_object(...)` DLL request.
pub fn drag_object(object_id: u32, destination: Waypoint) -> Result<DragRequest, DragError> {
    DragRequest::object_to(object_id, destination)
}

/// Drag request or hook failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragError {
    /// Object id zero is reserved for "no object".
    InvalidObjectId,
    /// Destination contains NaN or infinity and cannot be navigated to.
    InvalidDestination,
    /// The low-level game-client drag hook has not been implemented yet.
    HookUnavailable,
}

impl fmt::Display for DragError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidObjectId => f.write_str("drag object id must be non-zero"),
            Self::InvalidDestination => f.write_str("drag destination must be finite"),
            Self::HookUnavailable => f.write_str("game-client drag hook is unavailable"),
        }
    }
}

impl Error for DragError {}

fn validate_destination(destination: &Waypoint) -> Result<(), DragError> {
    if destination.x.is_finite() && destination.y.is_finite() && destination.z.is_finite() {
        Ok(())
    } else {
        Err(DragError::InvalidDestination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_request_captures_id_and_destination() {
        let destination = Waypoint::new(100.0, 200.0, 3.0);
        let request = drag_object(42, destination).expect("valid object drag request");

        assert_eq!(request.subject, DragSubject::Object { object_id: 42 });
        assert_eq!(request.destination, destination);
    }

    #[test]
    fn request_rejects_non_finite_destination() {
        let err = drag_corpse_to(Waypoint::new(100.0, f32::INFINITY, 3.0))
            .expect_err("infinite destination cannot be navigated to");

        assert_eq!(err, DragError::InvalidDestination);
    }

    #[test]
    fn hooked_mechanic_reports_unavailable_until_client_hook_lands() {
        let request = drag_corpse_to(Waypoint::new(100.0, 200.0, 3.0))
            .expect("valid corpse drag request");
        let mut mechanic = HookedDragMechanic;

        assert_eq!(mechanic.begin_drag(&request), Err(DragError::HookUnavailable));
    }
}
