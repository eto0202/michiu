use crate::{EffectId, EntityId, SignalId};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum MichiuError {
    #[error(
        "Entity {id:?} not found.\n\
            Possible causes:\n\
            - The entity was already despawned/destroyed (dangling ID).\n\
            - An uninitialized or dummy EntityId was used."
    )]
    EntityNotFound { id: EntityId },

    #[error(
        "Component '{component}' not found for Entity {id:?}.\n\
            Possible causes:\n\
            - The component was not registered during spawn.\n
            - The component was removed/detached from the entity.\n\
            - The entity does not possess this property."
    )]
    ComponentNotFound {
        id: EntityId,
        component: &'static str,
    },

    #[error(
        "Signal {0:?} not found.\n\
            Possible causes: It has already been disposed or never registered."
    )]
    SignalDisposed(SignalId),

    #[error(
        "Effect {0:?} not found.\n\
            Possible causes: It has already been disposed or never registered."
    )]
    EffectDisposed(EffectId),
}
