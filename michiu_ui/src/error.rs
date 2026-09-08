use crate::{EffectId, EntityId, SignalId};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum MichiuError {
    #[error("Component '{component}' not found for Entity {id:?}")]
    ComponentNotFound {
        id: EntityId,
        component: &'static str,
    },

    #[error("Entity {id:?} is already dead/destroyed (slot generation mismatched)")]
    EntityDead { id: EntityId },

    #[error("Signal {0:?} does not exist or has been disposed")]
    SignalDisposed(SignalId),

    #[error("Effect {0:?} does not exist or has been disposed")]
    EffectDisposed(EffectId),
}
