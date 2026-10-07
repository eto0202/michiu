pub mod dnd;
pub mod edit;
pub mod focus;
pub mod resize;
pub mod scroll;

pub use dnd::*;
pub use edit::*;
pub use focus::*;
pub use resize::*;
pub use scroll::*;

use crate::{CapacityConfig, EntityId};

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
    pub(crate) fn new() -> Self {
        Self {
            dnd: DndStore::new(),
            resize: ResizeStore::new(),
            scroll: ScrollStore::new(),
            edit: TextEditStore::new(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            dnd: DndStore::with_capacity(c),
            scroll: ScrollStore::with_capacity(c),
            edit: TextEditStore::with_capacity(c),
            ..Default::default()
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.dnd.clear();
        self.resize.clear();
        self.scroll.clear();
        self.edit.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.dnd.despawn(id);
        self.scroll.despawn(id);
        self.edit.despawn(id);
    }

    #[inline]
    pub fn active_drag_state_mut(&mut self) -> &mut Option<ActiveDragState> {
        &mut self.dnd.dnd_active_drag_state
    }

    #[inline]
    pub fn drag_properties_mut(&mut self) -> &mut DndDragPropertiesSparse {
        &mut self.dnd.dnd_drag_properties
    }

    #[inline]
    pub fn drop_properties_mut(&mut self) -> &mut DndDropPropertiesSparse {
        &mut self.dnd.dnd_drop_properties
    }

    #[inline]
    pub fn selected_rects_mut(&mut self) -> &mut SelectedRectsSparse {
        &mut self.edit.edit_selected_rects
    }

    #[inline]
    pub fn selection_start_index_mut(&mut self) -> &mut SelectionStartIndexSparse {
        &mut self.edit.edit_selection_start_index
    }

    #[inline]
    pub fn selections_mut(&mut self) -> &mut TextSelectionsSparse {
        &mut self.edit.edit_selections
    }

    #[inline]
    pub fn active_resize_hover_mut(&mut self) -> &mut Option<(EntityId, ResizeDirection)> {
        &mut self.resize.res_active_resize_hover
    }

    #[inline]
    pub fn resizing_state_mut(&mut self) -> &mut Option<ResizingState> {
        &mut self.resize.res_resizing_state
    }

    #[inline]
    pub fn scroll_offsets_mut(&mut self) -> &mut ScrollOffsetsSecondary {
        &mut self.scroll.sc_offsets
    }

    #[inline]
    pub fn scroll_sizes_mut(&mut self) -> &mut ScrollSizesSecondary {
        &mut self.scroll.sc_sizes
    }
}

#[cfg(test)]
mod tests;
