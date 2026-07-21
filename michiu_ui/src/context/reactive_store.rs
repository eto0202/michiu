use crate::*;
use slotmap::{SecondaryMap, SlotMap, SparseSecondaryMap};
use smallvec::SmallVec;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectCategory {
    None,
    Style,
    Text,
    Input,
    Image,
    Movie,
    WebView2,
    Contents,
    UiaName,
    UiaAutomationId,
    ActiveState,
    SelectState,
    DisableState,
    FocusState,
    FocusableState,
}

pub struct ReactiveStore {
    pub(crate) signals: SlotMap<SignalId, Box<dyn std::any::Any>>,
    pub(crate) effects: SlotMap<EffectId, Effects>,
    pub(crate) subscribers: SecondaryMap<SignalId, SmallVec<[EffectId; 4]>>,
    pub(crate) element_effects: SecondaryMap<EntityId, SmallVec<[(EffectCategory, EffectId); 4]>>,
    pub(crate) effect_to_element: SecondaryMap<EffectId, EntityId>,
    pub(crate) pending_element_effects: Vec<EffectId>,
    pub(crate) providers: SparseSecondaryMap<EntityId, HashMap<std::any::TypeId, SignalId>>,
}

pub(crate) type Effects = Box<dyn FnMut(&mut Context)>;

impl Default for ReactiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactiveStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            signals: SlotMap::with_key(),
            effects: SlotMap::with_key(),
            subscribers: SecondaryMap::new(),
            element_effects: SecondaryMap::new(),
            effect_to_element: SecondaryMap::new(),
            pending_element_effects: Vec::new(),
            providers: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.signals.clear();
        self.effects.clear();
        self.subscribers.clear();
        self.element_effects.clear();
        self.effect_to_element.clear();
        self.pending_element_effects.clear();
        self.providers.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        if let Some(effects) = self.element_effects.remove(id) {
            for (_, effect_id) in effects {
                self.effects.remove(effect_id);
                self.effect_to_element.remove(effect_id);
                self.pending_element_effects.retain(|&x| x != effect_id);
            }
        }
        self.providers.remove(id);
    }
}
