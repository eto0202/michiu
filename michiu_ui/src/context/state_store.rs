pub mod dnd;
pub mod focus;
pub mod resize;
pub mod scroll;
pub mod edit;

pub use dnd::*;
pub use focus::*;
pub use resize::*;
pub use scroll::*;
pub use edit::*;

use crate::{CapacityConfig, EntityId, LayoutPoint, LayoutRect, LayoutSize};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::ops::Range;

pub struct StateStore {
    pub(crate) dnd: DndStore,
    pub(crate) resize: ResizeStore,
    pub(crate) scroll: ScrollStore,
    pub(crate) edit: TextEditStore,
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            dnd: DndStore::new(),
            resize: ResizeStore::new(),
            scroll: ScrollStore::new(),
            edit: TextEditStore::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            dnd: DndStore::with_capacity(c),
            scroll: ScrollStore::with_capacity(c),
            edit: TextEditStore::with_capacity(c),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.dnd.clear();
        self.resize.clear();
        self.scroll.clear();
        self.edit.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.dnd.despawn(id);
        self.scroll.despawn(id);
        self.edit.despawn(id);
    }
}

#[cfg(test)]
mod tests;
